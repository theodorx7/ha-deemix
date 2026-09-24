//! Read-only WebSocket listener for the local deemix server.
//!
//! deemix (the `ws` library, RFC 6455) broadcasts queue events as `{"key": "...", "data": {...}}` frames to every connected client — no auth, no heartbeat. 
//! The bot keeps one background connection open to buffer `queueError`/`alreadyInQueue` for request confirmation (see add_to_queue_confirmed)
//! and to forward per-track download errors (`updateQueue` failed/postFailed) to the chat that requested the object.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use teloxide::types::ChatId;
use teloxide::{Bot, Requester};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::{WsEvent, WsEventKind};
use crate::BotState;

/// Events older than this are pruned on every insert.
const EVENT_TTL: Duration = Duration::from_secs(60);
/// Hard cap so an event burst can never grow the buffer unbounded.
const EVENT_CAP: usize = 128;

/// Keep a WebSocket connection to deemix alive forever: buffers queueError / alreadyInQueue events into BotState and forwards download errors to the
/// requesting chat. Reconnects with a simple backoff: 1s doubling up to 60s, reset after a successful connect.
pub(crate) async fn run_ws_listener(bot: Bot, state: Arc<BotState>) {
    let ws_url = format!("{}/", state.config.deemix_url.replacen("http", "ws", 1));
    let mut delay = Duration::from_secs(1);
    loop {
        match connect_async(ws_url.as_str()).await {
            Ok((mut stream, _)) => {
                delay = Duration::from_secs(1);
                log::info!("[ws] connected to {}", ws_url);
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => handle_frame(&bot, &state, text.as_str()).await,
                        Ok(_) => {}
                        Err(e) => {
                            log::warn!("[ws] read error: {}", e);
                            break;
                        }
                    }
                }
                log::warn!("[ws] connection closed, reconnecting");
            }
            Err(e) => log::warn!("[ws] connect failed: {}", e),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(60));
    }
}

/// Parse one frame: forward download errors / clean the initiator registry, or buffer queueError / alreadyInQueue for add_to_queue_confirmed.
async fn handle_frame(bot: &Bot, state: &Arc<BotState>, text: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };

    match v["key"].as_str() {
        Some("updateQueue") => {
            if v["data"]["failed"].as_bool() == Some(true) {
                forward_track_error(bot, state, &v["data"]).await;
            } else if v["data"]["postFailed"].as_bool() == Some(true) {
                forward_post_error(bot, state, &v["data"]).await;
            }
            return; // progress / downloaded / alreadyDownloaded are not errors
        }
        Some("finishDownload") | Some("removedFromQueue") => {
            // Both carry the object's uuid; the registry entry is not needed anymore
            if let Some(uuid) = v["data"]["uuid"].as_str() {
                state.queue_initiators.lock().await.remove(uuid);
            }
            return;
        }
        _ => {}
    }

    let kind = match v["key"].as_str() {
        Some("queueError") => WsEventKind::QueueError {
            link: v["data"]["link"].as_str().map(|s| s.to_string()),
            error: v["data"]["error"].as_str().unwrap_or("Unknown error").to_string(),
            errid: v["data"]["errid"].as_str().map(|s| s.to_string()),
        },
        Some("alreadyInQueue") => WsEventKind::AlreadyInQueue {
            title: v["data"]["title"].as_str().unwrap_or("?").to_string(),
            artist: v["data"]["artist"]["name"]
                .as_str()
                .or_else(|| v["data"]["artist"].as_str())
                .unwrap_or("")
                .to_string(),
            size: v["data"]["size"].as_u64().unwrap_or(1),
        },
        _ => return,
    };
    let mut buf = state.ws_events.lock().await;
    let now = Instant::now();
    buf.retain(|e| now.duration_since(e.ts) < EVENT_TTL);
    if buf.len() >= EVENT_CAP {
        buf.pop_front();
    }
    buf.push_back(WsEvent { ts: now, kind });
}

/// Forward a failed-track event to the chat that requested the object.
async fn forward_track_error(bot: &Bot, state: &Arc<BotState>, data: &serde_json::Value) {
    let Some(uuid) = data["uuid"].as_str() else { return };
    let Some((chat_id, _)) = initiator(state, uuid).await else { return };
    let title = data["data"]["title"].as_str().unwrap_or("?");
    let artist = data["data"]["artist"].as_str().unwrap_or("?");
    let error = data["error"].as_str().unwrap_or("Unknown error");
    let text = match data["errid"].as_str() {
        Some(errid) => format!("❌ Download failed: {} — {}\n{} ({})", title, artist, error, errid),
        None => format!("❌ Download failed: {} — {}\n{}", title, artist, error),
    };
    let _ = bot.send_message(ChatId(chat_id), text).await;
}

/// Forward a post-processing failure (artwork / log file / m3u8 / command);
/// the music itself has already been downloaded.
async fn forward_post_error(bot: &Bot, state: &Arc<BotState>, data: &serde_json::Value) {
    let Some(uuid) = data["uuid"].as_str() else { return };
    let Some((chat_id, title)) = initiator(state, uuid).await else { return };
    let error = data["error"].as_str().unwrap_or("Unknown error");
    let position = data["data"]["position"].as_str().unwrap_or("unknown");
    let _ = bot.send_message(
        ChatId(chat_id),
        format!("⚠️ Post-processing error on \"{}\": {} ({})", title, error, position),
    )
    .await;
}

/// Look up which chat requested the given queue object, if any.
async fn initiator(state: &Arc<BotState>, uuid: &str) -> Option<(i64, String)> {
    state.queue_initiators.lock().await.get(uuid).cloned()
}
