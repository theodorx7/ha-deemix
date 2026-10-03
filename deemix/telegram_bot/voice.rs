//! ACRCloud song recognition from Telegram voice notes.
//!
//! A received voice note is sent straight to ACRCloud — no choice dialogs.
//! Enable by setting ACRCLOUD_HOST/ACCESS_KEY/SECRET in the add-on options.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, KeyInit, Mac};
use reqwest::multipart;
use sha1::Sha1;

use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{FileId, InlineKeyboardButton, InlineKeyboardMarkup};

use crate::{BotState, build_search_results, deemix, spotify, user_id_from_msg, users};

/// Recognition result from ACRCloud.
/// Prefer deezer_url → spotify_url (metadata refinement) → text search, in that order.
pub struct RecognitionResult {
    pub title: String,
    pub artist: String,
    pub deezer_url: Option<String>,
    pub spotify_url: Option<String>,
}

/// Identify a song from audio bytes using the ACRCloud Identification API.
/// Returns RecognitionResult or an error string.
async fn recognize(
    http: &reqwest::Client,
    audio_bytes: Vec<u8>,
    cfg: &crate::config::Config,
) -> Result<RecognitionResult, String> {
    if !cfg.acrcloud_enabled() {
        return Err("Song recognition is not configured.".to_string());
    }
    if cfg.acrcloud_host.is_empty() {
        return Err("ACRCloud Host is not configured. Set it in the add-on options.".to_string());
    }
    if audio_bytes.len() >= 5 * 1024 * 1024 {
        return Err("Voice note too long for song recognition.".to_string());
    }

    // signature = base64(HMAC-SHA1(string_to_sign, access_secret))
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let string_to_sign = format!("POST\n/v1/identify\n{}\naudio\n1\n{}", cfg.acrcloud_access_key, ts);
    let mut mac = Hmac::<Sha1>::new_from_slice(cfg.acrcloud_secret_key.as_bytes())
        .map_err(|e| e.to_string())?;
    mac.update(string_to_sign.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());

    let sample_bytes = audio_bytes.len();
    let part = multipart::Part::bytes(audio_bytes)
        .file_name("audio.ogg")
        .mime_str("audio/ogg")
        .map_err(|e| e.to_string())?;

    let form = multipart::Form::new()
        .part("sample", part)
        .text("access_key", cfg.acrcloud_access_key.clone())
        .text("sample_bytes", sample_bytes.to_string())
        .text("timestamp", ts.to_string())
        .text("signature", signature)
        .text("data_type", "audio")
        .text("signature_version", "1");

    let resp = http
        .post(format!("https://{}/v1/identify", cfg.acrcloud_host))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("ACRCloud API error: HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let code = data["status"]["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(if code == 1001 {
            "Song not recognized. Try a longer clip.".to_string()
        } else {
            format!(
                "ACRCloud API error ({}): {}",
                code,
                data["status"]["msg"].as_str().unwrap_or("unknown")
            )
        });
    }

    let music = &data["metadata"]["music"][0];
    let title = music["title"].as_str().unwrap_or("").to_string();
    let artist = music["artists"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();

    if title.is_empty() {
        return Err("Song not recognized. Try a longer clip.".to_string());
    }

    let deezer_url = music["external_metadata"]["deezer"]["track"]["id"]
        .as_str()
        .map(|id| format!("https://www.deezer.com/track/{}", id));
    let spotify_url = music["external_metadata"]["spotify"]["track"]["id"]
        .as_str()
        .map(|id| format!("https://open.spotify.com/track/{}", id));

    log::info!(
        "ACRCloud recognized: title={:?} artist={:?} deezer_url={:?} spotify_url={:?}",
        title, artist, deezer_url, spotify_url
    );

    Ok(RecognitionResult { title, artist, deezer_url, spotify_url })
}

// ── Voice Message Entry ─────────────────────
pub(crate) async fn handle_voice_message(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> ResponseResult<()> {
    let voice = match msg.voice() {
        Some(v) => v,
        None => return Ok(()),
    };
    let user_settings = users::get_or_create(&state.users, &state.config.users_file, user_id_from_msg(&msg));
    if !state.config.acrcloud_enabled() || !user_settings.song_recognition {
        bot.send_message(msg.chat.id, "⚠️ Song recognition is not configured or disabled. Use /settings to manage it.").await?;
        return Ok(());
    }

    let sent = bot.send_message(msg.chat.id, "🎵 Recognizing song...").await?;
    // Download audio from Telegram
    let Some(audio_bytes) =
        download_voice_audio(&bot, &state, voice.file.id.clone(), msg.chat.id, sent.id).await?
    else {
        return Ok(());
    };
    process_voice_recognize(&bot, msg.chat.id, sent.id, audio_bytes, &state).await
}

/// Download a Telegram voice/audio file's bytes, reporting any failure by editing the status message. `None` = the user has already been notified,
/// the caller must stop. Large audio files (Telegram caps bot downloads at 20 MB) need the per-request 120 s timeout overriding the client-wide 30 s.
async fn download_voice_audio(
    bot: &Bot,
    state: &Arc<BotState>,
    file_id: FileId,
    chat_id: teloxide::types::ChatId,
    status_msg_id: teloxide::types::MessageId,
) -> ResponseResult<Option<Vec<u8>>> {
    let file = match bot.get_file(file_id).await {
        Ok(f) => f,
        Err(e) => {
            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to fetch file: {}", e)).await?;
            return Ok(None);
        }
    };
    let url = format!("https://api.telegram.org/file/bot{}/{}", bot.token(), file.path);
    let resp = match state.http.get(&url).timeout(std::time::Duration::from_secs(120)).send().await {
        Ok(r) => r,
        Err(e) => {
            let e = e.without_url();
            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to download audio: {}", e)).await?;
            return Ok(None);
        }
    };
    match resp.bytes().await {
        Ok(b) => Ok(Some(b.to_vec())),
        Err(e) => {
            let e = e.without_url();
            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to read audio: {}", e)).await?;
            Ok(None)
        }
    }
}

/// Core song recognition logic.
/// Implements the cascade: user confirmation of the recognized Deezer track → Spotify metadata → text search.
pub(crate) async fn process_voice_recognize(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    status_msg_id: teloxide::types::MessageId,
    audio_bytes: Vec<u8>,
    state: &Arc<BotState>,
) -> ResponseResult<()> {
    match recognize(&state.http, audio_bytes, &state.config).await {
        Ok(rec) => {
            let query = format!("{} {}", rec.title, rec.artist).replace('&', " ").split_whitespace().collect::<Vec<&str>>().join(" ");
            let text = format!("🎵 Recognized track: {} — {}", rec.title, rec.artist);
            // Step 1: exact Deezer track found — let the user confirm before queueing
            if let Some(ref deezer_url) = rec.deezer_url {
                log::info!("[recognize] Step 1: offering Deezer track: {}", deezer_url);
                let buttons = vec![
                    vec![InlineKeyboardButton::callback("⬇️ Download", format!("dl:{}", deezer_url))],
                    vec![InlineKeyboardButton::callback("❌ Cancel", "cancel")],
                ];
                bot.edit_message_text(chat_id, status_msg_id, text.clone())
                    .reply_markup(InlineKeyboardMarkup::new(buttons))
                    .await?;
            } else {
                bot.edit_message_text(chat_id, status_msg_id, text).await?;
                // Step 2: Try Spotify metadata for proper Unicode title; fall back to arabizi
                let search_query = if let Some(ref sp_url) = rec.spotify_url {
                    log::info!("[recognize] Step 2: resolving Spotify URL: {}", sp_url);
                    match spotify::resolve(sp_url).await {
                        Some(meta) => { log::info!("[recognize] Step 2: Spotify query: {:?}", meta.query); meta.query }
                        None => { log::info!("[recognize] Step 2: Spotify resolve failed, falling back to recognition text: {:?}", query); query.clone() }
                    }
                } else {
                    log::info!("[recognize] Step 2: no Spotify URL, using recognition text: {:?}", query);
                    query.clone()
                };
                log::info!("[recognize] Step 2: Deezer text search query: {:?}", search_query);
                let deezer_search_url: String = {
                    let encoded: String = search_query.bytes().map(|b| match b {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => char::from(b).to_string(),
                        b' ' => "%20".to_string(),
                        _ => format!("%{:02X}", b),
                    }).collect();
                    format!("https://www.deezer.com/search/{}", encoded)
                };
                let sent = bot.send_message(chat_id, "Searching on Deezer...").await?;
                let results = match deemix::search(&state, &search_query, "track").await {
                    Err(e) => {
                        log::info!("[recognize] Step 2: Deezer search error: {}", e);
                        bot.edit_message_text(chat_id, sent.id, format!("❌ Search failed: {}", e)).await?;
                        return Ok(());
                    }
                    Ok(r) if r.is_empty() => {
                        log::info!("[recognize] Step 2: full query 0 results, retrying title-only: {:?}", rec.title);
                        deemix::search(&state, &rec.title, "track").await.unwrap_or_default()
                    }
                    Ok(r) => r,
                };
                if results.is_empty() {
                    log::info!("[recognize] Step 2: title-only search also 0 results");
                    if let Ok(url) = reqwest::Url::parse(&deezer_search_url) {
                        bot.edit_message_text(chat_id, sent.id, format!("😕 No results for: {} — {}\n\nSearch on Deezer and paste the link back here.", rec.title, rec.artist))
                            .reply_markup(InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::url("🔍 Search on Deezer", url)]]))
                            .await?;
                    } else {
                        bot.edit_message_text(chat_id, sent.id, format!("😕 No results for: {} — {}", rec.title, rec.artist)).await?;
                    }
                } else {
                    log::info!("[recognize] Step 2: got {} Deezer results", results.len());
                    let (listing, mut buttons) = build_search_results(&results, "🎵");
                    if let Ok(url) = reqwest::Url::parse(&deezer_search_url) {
                        buttons.push(vec![InlineKeyboardButton::url("🔍 None of these — search on Deezer", url)]);
                    }
                    buttons.push(vec![InlineKeyboardButton::callback("❌ Cancel", "cancel")]);
                    bot.edit_message_text(chat_id, sent.id, format!("Results for {} — {}:\n\n{}\nIf none match, search on Deezer and paste the link back here.", rec.title, rec.artist, listing))
                        .reply_markup(InlineKeyboardMarkup::new(buttons))
                        .await?;
                }
            }
        }
        Err(e) => {
            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Recognition failed: {}", e)).await?;
        }
    }
    Ok(())
}
