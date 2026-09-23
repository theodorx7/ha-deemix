//! Read-only WebSocket listener for the local deemix server.
//!
//! deemix (the `ws` library, RFC 6455) broadcasts queue events as
//! `{"key": "...", "data": {...}}` frames to every connected client — no
//! auth, no heartbeat. The bot keeps one background connection open solely
//! to observe `queueError` and `alreadyInQueue`, the two outcomes the HTTP
//! addToQueue response cannot report; all other events are ignored.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::{WsEvent, WsEventKind};
use crate::BotState;

/// Events older than this are pruned on every insert.
const EVENT_TTL: Duration = Duration::from_secs(60);
/// Hard cap so an event burst can never grow the buffer unbounded.
const EVENT_CAP: usize = 128;

/// Keep a WebSocket connection to deemix alive forever, buffering
/// queueError / alreadyInQueue events into BotState. Reconnects with a
/// simple backoff: 1s doubling up to 60s, reset after a successful connect.
pub(crate) async fn run_ws_listener(state: Arc<BotState>) {
    let ws_url = format!("{}/", state.config.deemix_url.replacen("http", "ws", 1));
    let mut delay = Duration::from_secs(1);
    loop {
        match connect_async(ws_url.as_str()).await {
            Ok((mut stream, _)) => {
                delay = Duration::from_secs(1);
                state.ws_connected.store(true, Ordering::Relaxed);
                log::info!("[ws] connected to {}", ws_url);
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => handle_frame(&state, text.as_str()).await,
                        Ok(_) => {}
                        Err(e) => {
                            log::warn!("[ws] read error: {}", e);
                            break;
                        }
                    }
                }
                state.ws_connected.store(false, Ordering::Relaxed);
                log::warn!("[ws] connection closed, reconnecting");
            }
            Err(e) => log::warn!("[ws] connect failed: {}", e),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(60));
    }
}

/// Parse one frame and buffer it if it is one of the two interesting events.
async fn handle_frame(state: &Arc<BotState>, text: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
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
