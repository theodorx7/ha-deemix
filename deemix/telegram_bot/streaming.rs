//! Streaming-link handling: Spotify, YouTube, YouTube Music and Apple Music.
//!
//! Converts a streaming URL to Deezer: single tracks via metadata search,
//! playlists are scanned per-track, everything else goes through Odesli.

use std::sync::{Arc, LazyLock};

use regex::Regex;

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::{BotState, build_search_results, capitalize, deemix, spotify, voice, youtube};

// ── URL Patterns ──────────────────────────────────────────────────────────────
pub(crate) static SPOTIFY_TRACK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"https?://open\.spotify\.com/track/([A-Za-z0-9]+)"
).unwrap());
pub(crate) static SPOTIFY_ALBUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"https?://open\.spotify\.com/album/([A-Za-z0-9]+)"
).unwrap());
pub(crate) static SPOTIFY_PLAYLIST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"https?://open\.spotify\.com/playlist/[A-Za-z0-9]+"
).unwrap());
pub(crate) static YOUTUBE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"https?://(?:(?:www\.)?youtube\.com/|youtu\.be/|music\.youtube\.com/)"
).unwrap());
pub(crate) static APPLE_MUSIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"https?://music\.apple\.com/"
).unwrap());

pub(crate) async fn handle_streaming_link(bot: &Bot, msg: &Message, state: &Arc<BotState>, url: &str) -> ResponseResult<()> {
    let sent = bot.send_message(msg.chat.id, "🎵 Looking up link...").await?;

    // Spotify track or album: resolve metadata then show Deezer search results
    if SPOTIFY_TRACK_RE.is_match(url) || SPOTIFY_ALBUM_RE.is_match(url) {
        let search_type = if SPOTIFY_TRACK_RE.is_match(url) { "track" } else { "album" };
        match spotify::resolve(url).await {
            Some(meta) => {
                bot.edit_message_text(msg.chat.id, sent.id, format!("🔍 Found: {}\nSearching on Deezer...", meta.label)).await?;
                match deemix::search(state, &meta.query, search_type).await {
                    Ok(results) if results.is_empty() => {
                        // Fallback: try Odesli directly before giving up
                        match voice::lookup_deezer_via_spotify(&state.http, url).await {
                            Some(deezer_url) => {
                                match deemix::add_to_queue(state, &deezer_url).await {
                                    Ok(_) => { bot.edit_message_text(msg.chat.id, sent.id, format!("✅ {} added to queue!", meta.label)).await?; }
                                    Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Failed to queue: {}", e)).await?; }
                                }
                            }
                            None => { bot.edit_message_text(msg.chat.id, sent.id, format!("😕 No results found on Deezer for: {}", meta.query)).await?; }
                        }
                    }
                    Ok(results) => {
                        let icon = if search_type == "track" { "🎵" } else { "💿" };
                        let (listing, mut buttons) = build_search_results(&results, icon);
                        buttons.push(vec![InlineKeyboardButton::callback("❌ Cancel", "cancel")]);
                        bot.edit_message_text(msg.chat.id, sent.id, format!("Results for {}:\n\n{}\nTap a button to download.", meta.query, listing))
                            .reply_markup(InlineKeyboardMarkup::new(buttons))
                            .await?;
                    }
                    Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Search failed: {}", e)).await?; }
                }
            }
            None => { bot.edit_message_text(msg.chat.id, sent.id, "❌ Could not resolve Spotify link. Try /search instead.").await?; }
        }
        return Ok(());
    }

    // Spotify playlist: scan the track list and queue each song individually,
    // so user-generated playlists work (Deezer rarely has a same-named playlist)
    if SPOTIFY_PLAYLIST_RE.is_match(url) {
        bot.edit_message_text(msg.chat.id, sent.id, "🔍 Reading Spotify playlist...").await?;
        match spotify::resolve_playlist(url).await {
            Some(pl) => { queue_playlist(bot, msg, state, sent.id, &pl).await?; }
            None => {
                bot.edit_message_text(msg.chat.id, sent.id,
                    "😕 Couldn't read that Spotify playlist. Make sure it's public and try again.").await?;
            }
        }
        return Ok(());
    }

    // YouTube playlist: same per-track treatment
    let mut url = url.to_string();
    if YOUTUBE_RE.is_match(&url) && youtube::playlist_id(&url).is_some() {
        bot.edit_message_text(msg.chat.id, sent.id, "🔍 Reading YouTube playlist...").await?;
        match youtube::resolve_playlist(&state.http, &url).await {
            Some(pl) => {
                queue_playlist(bot, msg, state, sent.id, &pl).await?;
                return Ok(());
            }
            None => {
                if url.contains("watch?v=") || url.contains("youtu.be/") {
                    // Mixes and private lists can't be scanned — fall back to the single video
                    url = youtube::strip_playlist_params(&url);
                } else {
                    bot.edit_message_text(msg.chat.id, sent.id,
                        "😕 Couldn't read that YouTube playlist. Make sure it's public and try again.").await?;
                    return Ok(());
                }
            }
        }
    }
    let url = url.as_str();

    // Single YouTube video / Apple Music: convert via Odesli to a Deezer URL and queue directly
    let service = if YOUTUBE_RE.is_match(url) {
        "YouTube link".to_string()
    } else {
        "Apple Music link".to_string()
    };

    bot.edit_message_text(msg.chat.id, sent.id, format!("🔍 Looking up {} on Deezer...", service)).await?;

    match voice::lookup_deezer_via_spotify(&state.http, url).await {
        Some(deezer_url) => {
            match deemix::add_to_queue(state, &deezer_url).await {
                Ok(_) => { bot.edit_message_text(msg.chat.id, sent.id, format!("✅ {} added to queue!", capitalize(&service))).await?; }
                Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Failed to queue: {}", e)).await?; }
            }
        }
        None => {
            bot.edit_message_text(msg.chat.id, sent.id,
                format!("😕 Couldn't find this {} on Deezer. Try searching by name with /search.", service)).await?;
        }
    }

    Ok(())
}

fn first_link(results: &[serde_json::Value]) -> Option<String> {
    results.first().and_then(|x| x["link"].as_str()).map(|s| s.to_string())
}

/// Queue every track of a scanned playlist by searching it on Deezer and
/// queuing the first match. Edits the status message with progress and a
/// final summary of anything that couldn't be found.
async fn queue_playlist(
    bot: &Bot,
    msg: &Message,
    state: &Arc<BotState>,
    status_id: teloxide::types::MessageId,
    pl: &spotify::Playlist,
) -> ResponseResult<()> {
    let total = pl.tracks.len();
    let mut queued = 0usize;
    let mut not_found: Vec<String> = Vec::new();

    for (i, track) in pl.tracks.iter().enumerate() {
        if i % 5 == 0 {
            let _ = bot.edit_message_text(msg.chat.id, status_id,
                format!("⏳ Queuing \"{}\" — {}/{} tracks...", pl.name, i, total)).await;
        }

        let full_query = if track.artist.is_empty() {
            track.title.clone()
        } else {
            format!("{} {}", track.title, track.artist)
        };
        let mut link = deemix::search(state, &full_query, "track").await.ok()
            .and_then(|r| first_link(&r));
        if link.is_none() && !track.artist.is_empty() {
            link = deemix::search(state, &track.title, "track").await.ok()
                .and_then(|r| first_link(&r));
        }

        let label = if track.artist.is_empty() {
            track.title.clone()
        } else {
            format!("{} — {}", track.title, track.artist)
        };
        match link {
            Some(l) => match deemix::add_to_queue(state, &l).await {
                Ok(_) => queued += 1,
                Err(e) => {
                    log::warn!("[playlist] failed to queue {:?}: {}", label, e);
                    not_found.push(label);
                }
            },
            None => not_found.push(label),
        }
    }

    let mut text = format!("✅ Playlist \"{}\": queued {}/{} tracks.", pl.name, queued, total);
    if !not_found.is_empty() {
        text.push_str("\n\n😕 Couldn't queue these:\n");
        for t in not_found.iter().take(15) {
            let t = if t.chars().count() > 80 {
                format!("{}…", t.chars().take(79).collect::<String>())
            } else {
                t.clone()
            };
            text.push_str(&format!("• {}\n", t));
        }
        if not_found.len() > 15 {
            text.push_str(&format!("…and {} more", not_found.len() - 15));
        }
    }
    bot.edit_message_text(msg.chat.id, status_id, text).await?;
    Ok(())
}
