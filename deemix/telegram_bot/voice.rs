//! Voice feature handlers.
//!
//! Supports three Whisper backends (priority order):
//!   1. OpenAI remote API  — set OPENAI_API_KEY
//!   2. Local compatible   — set WHISPER_URL (e.g. faster-whisper-server)
//!   3. Disabled           — leave both empty
//!
//! AudD song recognition  — set AUDD_API_KEY

use reqwest::multipart;
use regex::Regex;

use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup};

use crate::{BotState, MyDialogue, build_search_results, deemix, spotify, user_id_from_msg, users};

/// Transcribe an OGG audio file using the configured Whisper backend.
/// Returns the transcribed text or an error string.
pub async fn transcribe(
    http: &reqwest::Client,
    audio_bytes: Vec<u8>,
    openai_key: &str,
    whisper_url: &str,
) -> Result<String, String> {
    // Determine endpoint and auth header
    let (endpoint, auth) = if !openai_key.is_empty() {
        (
            "https://api.openai.com/v1/audio/transcriptions".to_string(),
            format!("Bearer {}", openai_key),
        )
    } else if !whisper_url.is_empty() {
        (whisper_url.to_string(), String::new())
    } else {
        return Err("Voice search is not configured.".to_string());
    };

    let part = multipart::Part::bytes(audio_bytes)
        .file_name("audio.ogg")
        .mime_str("audio/ogg")
        .map_err(|e| e.to_string())?;

    let form = multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1")
        .text("response_format", "text");

    let mut req = http.post(&endpoint).multipart(form);
    if !auth.is_empty() {
        req = req.header("Authorization", auth);
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Whisper API error: {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;
    Ok(text.trim().to_string())
}

/// Recognition result from AudD.
/// Prefer deezer_url → song_link (Odesli) → spotify_url → text search, in that order.
pub struct RecognitionResult {
    pub title: String,
    pub artist: String,
    pub deezer_url: Option<String>,
    pub spotify_url: Option<String>,
    pub song_link: Option<String>,
}

/// Identify a song from audio bytes using the AudD API.
/// Returns RecognitionResult or an error string.
pub async fn recognize(
    http: &reqwest::Client,
    audio_bytes: Vec<u8>,
    audd_key: &str,
) -> Result<RecognitionResult, String> {
    if audd_key.is_empty() {
        return Err("Song recognition is not configured.".to_string());
    }

    let part = multipart::Part::bytes(audio_bytes)
        .file_name("audio.ogg")
        .mime_str("audio/ogg")
        .map_err(|e| e.to_string())?;

    let form = multipart::Form::new()
        .part("file", part)
        .text("api_token", audd_key.to_string())
        .text("return", "deezer,spotify");

    let resp = http
        .post("https://api.audd.io/")
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    if data["status"] != "success" {
        return Err("AudD could not identify the song.".to_string());
    }

    let result = &data["result"];
    if result.is_null() {
        return Err("Song not recognized. Try a longer clip.".to_string());
    }

    let title = result["title"].as_str().unwrap_or("").to_string();
    let artist = result["artist"].as_str().unwrap_or("").to_string();

    if title.is_empty() {
        return Err("Song not recognized.".to_string());
    }

    let deezer_url = result["deezer"]["link"]
        .as_str()
        .map(|s| s.to_string());

    let spotify_url = result["spotify"]["external_urls"]["spotify"]
        .as_str()
        .map(|s| s.to_string());

    let song_link = result["song_link"].as_str().map(|s| s.to_string());

    log::info!(
        "AudD recognized: title={:?} artist={:?} deezer_url={:?} spotify_url={:?} song_link={:?}",
        title, artist, deezer_url, spotify_url, song_link
    );
    log::info!("AudD raw result keys: {:?}", result.as_object().map(|o| o.keys().collect::<Vec<_>>()));
    log::info!("AudD deezer field: {}", result["deezer"]);
    log::info!("AudD spotify field: {}", result["spotify"]);

    Ok(RecognitionResult { title, artist, deezer_url, spotify_url, song_link })
}

/// Scrape a song.link/lis.tn page for a direct Deezer URL via __NEXT_DATA__ JSON.
/// The Odesli API rejects lis.tn vanity URLs, so we parse the page HTML instead.
pub async fn lookup_deezer_via_odesli(http: &reqwest::Client, song_link: &str) -> Option<String> {
    log::info!("Scraping song.link page: {}", song_link);

    let resp = http.get(song_link).send().await.ok()?;
    let status = resp.status();
    if !status.is_success() {
        log::info!("song.link page fetch failed: {}", status);
        return None;
    }
    let html = resp.text().await.ok()?;

    let re = Regex::new(r#"<script id="__NEXT_DATA__" type="application/json">(.*?)</script>"#).ok()?;
    let caps = re.captures(&html);
    log::info!("song.link __NEXT_DATA__ regex matched: {}", caps.is_some());
    let json_str = caps?.get(1)?.as_str().to_string();

    let data: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    let page_props = &data["props"]["pageProps"];

    // Try the two known shapes of song.link __NEXT_DATA__
    let deezer_url =
        page_props["data"]["linksByPlatform"]["deezer"]["url"].as_str()
        .or_else(|| page_props["pageData"]["linksByPlatform"]["deezer"]["url"].as_str())
        .map(|s| s.to_string());

    if deezer_url.is_none() {
        let keys: Vec<&str> = page_props.as_object()
            .map(|o| o.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();
        log::info!("song.link __NEXT_DATA__ pageProps keys: {:?}", keys);
    }

    log::info!("song.link scraped Deezer URL: {:?}", deezer_url);
    deezer_url
}

/// Look up a Deezer URL via the Odesli API.
/// Accepts any streaming URL Odesli understands (Spotify, YouTube, Apple Music)
/// and returns the matching Deezer link if one exists.
pub async fn lookup_deezer_via_spotify(http: &reqwest::Client, spotify_url: &str) -> Option<String> {
    log::info!("Odesli lookup via Spotify URL: {}", spotify_url);

    let resp = http
        .get("https://api.song.link/v1-alpha.1/links")
        .query(&[("url", spotify_url)])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        log::info!("Odesli via Spotify failed: {}", resp.status());
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;
    let deezer_url = data["linksByPlatform"]["deezer"]["url"].as_str().map(|s| s.to_string());
    log::info!("Odesli via Spotify Deezer URL: {:?}", deezer_url);
    deezer_url
}

/// Search iTunes for title+artist, then pass the Apple Music URL to Odesli to get a Deezer link.
/// iTunes is free/no-auth and globally indexed; Odesli accepts Apple Music URLs.
pub async fn lookup_deezer_via_itunes(http: &reqwest::Client, title: &str, artist: &str) -> Option<String> {
    let term = format!("{} {}", title, artist);
    log::info!("iTunes search: {:?}", term);

    let resp = match http
        .get("https://itunes.apple.com/search")
        .query(&[("term", term.as_str()), ("media", "music"), ("entity", "song"), ("limit", "1")])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => { log::info!("iTunes request failed: {}", e); return None; }
    };

    log::info!("iTunes response status: {}", resp.status());
    let data: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => { log::info!("iTunes JSON parse failed: {}", e); return None; }
    };
    log::info!("iTunes resultCount: {}", data["resultCount"]);

    let track_url = match data["results"][0]["trackViewUrl"].as_str() {
        Some(u) => u.to_string(),
        None => { log::info!("iTunes: no trackViewUrl in first result"); return None; }
    };
    log::info!("iTunes track URL: {}", track_url);

    let resp = http
        .get("https://api.song.link/v1-alpha.1/links")
        .query(&[("url", track_url.as_str())])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        log::info!("Odesli via iTunes failed: {}", resp.status());
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;
    let deezer_url = data["linksByPlatform"]["deezer"]["url"].as_str().map(|s| s.to_string());
    log::info!("iTunes→Odesli Deezer URL: {:?}", deezer_url);
    deezer_url
}

// ── Voice Message Entry ─────────────────────
pub(crate) async fn handle_voice_message(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    dialogue: MyDialogue,
) -> ResponseResult<()> {
    dialogue.exit().await.ok(); // exit any active dialogue state
    let voice = match msg.voice() {
        Some(v) => v,
        None => return Ok(()),
    };
    let user_settings = users::get_or_create(&state.users, user_id_from_msg(&msg));
    let audd_on = state.config.audd_enabled() && user_settings.song_recognition;
    let whisper_on = state.config.whisper_enabled() && user_settings.voice_search;

    if !audd_on && !whisper_on {
        bot.send_message(msg.chat.id, "⚠️ Voice features are not configured or disabled. Use /settings to manage them.").await?;
        return Ok(());
    }

    // Store file_id with a short key to stay under Telegram's 64 byte callback limit
    let short_id = format!("{}", msg.id.0);
    {
        let mut map = state.pending_voices.lock().await;
        map.insert(short_id.clone(), voice.file.id.to_string());
    }

    // Build choice buttons based on what's enabled
    let mut buttons: Vec<Vec<teloxide::types::InlineKeyboardButton>> = vec![];
    if whisper_on {
        buttons.push(vec![teloxide::types::InlineKeyboardButton::callback(
            "🎤 Transcribe what I said",
            format!("vt:{}", short_id),
        )]);
    }
    if audd_on {
        buttons.push(vec![teloxide::types::InlineKeyboardButton::callback(
            "🎵 Recognize the song",
            format!("vr:{}", short_id),
        )]);
    }
    buttons.push(vec![teloxide::types::InlineKeyboardButton::callback("❌ Cancel", "cancel")]);

    bot.send_message(msg.chat.id, "🎙️ What should I do with this voice note?")
        .reply_markup(teloxide::types::InlineKeyboardMarkup::new(buttons))
        .await?;
    return Ok(());
}

pub(crate) async fn handle_voice_callback(
    bot: &Bot,
    q: &CallbackQuery,
    data: &str,
    state: &Arc<BotState>,
) -> ResponseResult<()> {
    let is_transcribe = data.starts_with("vt:");
    let is_recognize = data.starts_with("vr:");
    if is_transcribe || is_recognize {
        if let Some(msg) = &q.message {
            let short_id = &data[3..];
            let action = if is_transcribe { "transcribe" } else { "recognize" };

            // Retrieve file_id from pending_voices map
            let file_id = {
                let map = state.pending_voices.lock().await;
                map.get(short_id).cloned()
            };
            let file_id = match file_id {
                Some(f) => f,
                None => {
                    bot.edit_message_text(msg.chat.id, msg.id, "❌ Voice note expired. Please send it again.").await?;
                    return Ok(());
                }
            };

            bot.edit_message_text(msg.chat.id, msg.id, "⏳ Processing voice note...").await?;

            // Download audio from Telegram
            let file = bot.get_file(&file_id).await
                .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
            let url = format!("https://api.telegram.org/file/bot{}/{}", bot.token(), file.path);
            let audio_bytes = match state.http.get(&url).send().await {
                Ok(r) => match r.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => { bot.edit_message_text(msg.chat.id, msg.id, format!("❌ Failed to read audio: {}", e)).await?; return Ok(()); }
                },
                Err(e) => { bot.edit_message_text(msg.chat.id, msg.id, format!("❌ Failed to download audio: {}", e)).await?; return Ok(()); }
            };

            match action {
                "transcribe" => {
                    process_voice_transcribe(&bot, msg.chat.id, msg.id, audio_bytes, &state).await?;
                }
                "recognize" => {
                    process_voice_recognize(&bot, msg.chat.id, msg.id, audio_bytes, &state).await?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Core transcription logic shared by dialogue receiver and callback handler.
/// `status_msg_id` is the message being edited for progress updates.
pub(crate) async fn process_voice_transcribe(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    status_msg_id: teloxide::types::MessageId,
    audio_bytes: Vec<u8>,
    state: &Arc<BotState>,
) -> ResponseResult<()> {
    match transcribe(&state.http, audio_bytes, &state.config.openai_api_key, &state.config.whisper_url).await {
        Ok(text) if text.is_empty() => {
            bot.edit_message_text(chat_id, status_msg_id, "😕 Could not transcribe anything. Try speaking more clearly.").await?;
        }
        Ok(text) => {
            bot.edit_message_text(chat_id, status_msg_id, format!("🔍 I heard: {}\nSearching...", text)).await?;
            let sent = bot.send_message(chat_id, format!("Results for: {}", text)).await?;
            match deemix::search(&state, &text, "track").await {
                Ok(results) if results.is_empty() => {
                    bot.edit_message_text(chat_id, sent.id, format!("😕 No results for: {}", text)).await?;
                }
                Ok(results) => {
                    let (listing, mut buttons) = build_search_results(&results, "🎵");
                    buttons.push(vec![InlineKeyboardButton::callback("❌ Cancel", "cancel")]);
                    bot.edit_message_text(chat_id, sent.id, format!("Results for {}:\n\n{}\nTap a button to download.", text, listing))
                        .reply_markup(InlineKeyboardMarkup::new(buttons))
                        .await?;
                }
                Err(e) => {
                    bot.edit_message_text(chat_id, sent.id, format!("❌ Search failed: {}", e)).await?;
                }
            }
        }
        Err(e) => {
            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Transcription failed: {}", e)).await?;
        }
    }
    Ok(())
}

/// Core song recognition logic shared by dialogue receiver and callback handler.
/// Implements the full cascade: AudD Deezer URL → Odesli → iTunes → Spotify → text search.
pub(crate) async fn process_voice_recognize(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    status_msg_id: teloxide::types::MessageId,
    audio_bytes: Vec<u8>,
    state: &Arc<BotState>,
) -> ResponseResult<()> {
    match recognize(&state.http, audio_bytes, &state.config.audd_api_key).await {
        Ok(rec) => {
            let query = format!("{} {}", rec.title, rec.artist).replace('&', " ").split_whitespace().collect::<Vec<&str>>().join(" ");
            bot.edit_message_text(chat_id, status_msg_id, format!("🎵 Found: {} — {}\nQueuing...", rec.title, rec.artist)).await?;
            // Step 1: Use Deezer URL directly from AudD if available
            if let Some(ref deezer_url) = rec.deezer_url {
                log::info!("[recognize] Step 1: using AudD Deezer URL: {}", deezer_url);
                match deemix::add_to_queue(&state, deezer_url).await {
                    Ok(_) => { bot.edit_message_text(chat_id, status_msg_id, format!("✅ {} — {} added to queue!", rec.title, rec.artist)).await?; }
                    Err(e) => {
                        log::info!("[recognize] Step 1 FAILED: add_to_queue error: {}", e);
                        bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to queue: {}", e)).await?;
                    }
                }
            } else {
                // Step 2: Try Odesli (song.link) for a direct Deezer URL
                log::info!("[recognize] Step 1: no AudD Deezer URL, trying Odesli");
                let odesli_deezer = if let Some(ref sl) = rec.song_link {
                    log::info!("[recognize] Step 2: song_link present: {}", sl);
                    lookup_deezer_via_odesli(&state.http, sl).await
                } else {
                    log::info!("[recognize] Step 2: no song_link, skipping Odesli");
                    None
                };
                if let Some(ref deezer_url) = odesli_deezer {
                    log::info!("[recognize] Step 2: using Odesli Deezer URL: {}", deezer_url);
                    match deemix::add_to_queue(&state, deezer_url).await {
                        Ok(_) => { bot.edit_message_text(chat_id, status_msg_id, format!("✅ {} — {} added to queue!", rec.title, rec.artist)).await?; }
                        Err(e) => {
                            log::info!("[recognize] Step 2 FAILED: add_to_queue error: {}", e);
                            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to queue: {}", e)).await?;
                        }
                    }
                } else {
                    // Step 2b: Try iTunes → Odesli
                    log::info!("[recognize] Step 2: song.link gave no Deezer URL, trying iTunes→Odesli");
                    let itunes_deezer = lookup_deezer_via_itunes(&state.http, &rec.title, &rec.artist).await;
                    if let Some(ref deezer_url) = itunes_deezer {
                        log::info!("[recognize] Step 2b: using iTunes→Odesli Deezer URL: {}", deezer_url);
                        match deemix::add_to_queue(&state, deezer_url).await {
                            Ok(_) => { bot.edit_message_text(chat_id, status_msg_id, format!("✅ {} — {} added to queue!", rec.title, rec.artist)).await?; }
                            Err(e) => {
                                log::info!("[recognize] Step 2b FAILED: add_to_queue error: {}", e);
                                bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to queue: {}", e)).await?;
                            }
                        }
                    } else {
                        // Step 2c: Try Odesli API directly with Spotify URL
                        let spotify_deezer = if let Some(ref sp_url) = rec.spotify_url {
                            log::info!("[recognize] Step 2c: trying Odesli via Spotify URL: {}", sp_url);
                            lookup_deezer_via_spotify(&state.http, sp_url).await
                        } else {
                            log::info!("[recognize] Step 2c: no Spotify URL, skipping");
                            None
                        };
                        if let Some(ref deezer_url) = spotify_deezer {
                            log::info!("[recognize] Step 2c: using Spotify→Odesli Deezer URL: {}", deezer_url);
                            match deemix::add_to_queue(&state, deezer_url).await {
                                Ok(_) => { bot.edit_message_text(chat_id, status_msg_id, format!("✅ {} — {} added to queue!", rec.title, rec.artist)).await?; }
                                Err(e) => {
                                    log::info!("[recognize] Step 2c FAILED: {}", e);
                                    bot.edit_message_text(chat_id, status_msg_id, format!("❌ Failed to queue: {}", e)).await?;
                                }
                            }
                        } else {
                            // Step 3: Try Spotify metadata for proper Unicode title; fall back to arabizi
                            let search_query = if let Some(ref sp_url) = rec.spotify_url {
                                log::info!("[recognize] Step 3: resolving Spotify URL: {}", sp_url);
                                match spotify::resolve(sp_url).await {
                                    Some(meta) => { log::info!("[recognize] Step 3: Spotify query: {:?}", meta.query); meta.query }
                                    None => { log::info!("[recognize] Step 3: Spotify resolve failed, falling back to AudD text: {:?}", query); query.clone() }
                                }
                            } else {
                                log::info!("[recognize] Step 3: no Spotify URL, using AudD text: {:?}", query);
                                query.clone()
                            };
                            log::info!("[recognize] Step 3: Deezer text search query: {:?}", search_query);
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
                                    log::info!("[recognize] Step 3: Deezer search error: {}", e);
                                    bot.edit_message_text(chat_id, sent.id, format!("❌ Search failed: {}", e)).await?;
                                    return Ok(());
                                }
                                Ok(r) if r.is_empty() => {
                                    log::info!("[recognize] Step 3: full query 0 results, retrying title-only: {:?}", rec.title);
                                    deemix::search(&state, &rec.title, "track").await.unwrap_or_default()
                                }
                                Ok(r) => r,
                            };
                            if results.is_empty() {
                                log::info!("[recognize] Step 3: title-only search also 0 results");
                                if let Ok(url) = reqwest::Url::parse(&deezer_search_url) {
                                    bot.edit_message_text(chat_id, sent.id, format!("😕 No results for: {} — {}\n\nSearch on Deezer and paste the link back here.", rec.title, rec.artist))
                                        .reply_markup(InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::url("🔍 Search on Deezer", url)]]))
                                        .await?;
                                } else {
                                    bot.edit_message_text(chat_id, sent.id, format!("😕 No results for: {} — {}", rec.title, rec.artist)).await?;
                                }
                            } else {
                                log::info!("[recognize] Step 3: got {} Deezer results", results.len());
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
                }
            }
        }
        Err(e) => {
            bot.edit_message_text(chat_id, status_msg_id, format!("❌ Recognition failed: {}", e)).await?;
        }
    }
    Ok(())
}

// ── Dialogue Voice Receivers ───────────────────────────────────────────────────────────
pub(crate) async fn receive_voice_transcribe(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    dialogue.exit().await.ok();
    let voice = match msg.voice() {
        Some(v) => v,
        None => {
            bot.send_message(msg.chat.id, "⚠️ I expected a voice note. Use /menu to try again.").await?;
            return Ok(());
        }
    };
    let sent = bot.send_message(msg.chat.id, "🎤 Transcribing...").await?;
    // Download audio from Telegram
    let file = bot.get_file(&voice.file.id).await
        .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
    let url = format!("https://api.telegram.org/file/bot{}/{}", bot.token(), file.path);
    let audio_bytes = match state.http.get(&url).send().await {
        Ok(r) => r.bytes().await.map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?.to_vec(),
        Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Failed to download audio: {}", e)).await?; return Ok(()); }
    };
    process_voice_transcribe(&bot, msg.chat.id, sent.id, audio_bytes, &state).await
}

pub(crate) async fn receive_voice_recognize(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    dialogue.exit().await.ok();
    let voice = match msg.voice() {
        Some(v) => v,
        None => {
            bot.send_message(msg.chat.id, "⚠️ I expected a voice note. Use /menu to try again.").await?;
            return Ok(());
        }
    };
    let sent = bot.send_message(msg.chat.id, "🎵 Recognizing song...").await?;
    // Download audio from Telegram
    let file = bot.get_file(&voice.file.id).await
        .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
    let url = format!("https://api.telegram.org/file/bot{}/{}", bot.token(), file.path);
    let audio_bytes = match state.http.get(&url).send().await {
        Ok(r) => r.bytes().await.map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?.to_vec(),
        Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Failed to download audio: {}", e)).await?; return Ok(()); }
    };
    process_voice_recognize(&bot, msg.chat.id, sent.id, audio_bytes, &state).await
}
