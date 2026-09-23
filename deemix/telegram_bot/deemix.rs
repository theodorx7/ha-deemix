use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::config::WsEventKind;
use crate::BotState;

pub async fn login(state: &Arc<BotState>) {
    if state.config.deemix_arl.is_empty() {
        log::warn!("No ARL configured — login via the deemix web UI or /updatearl in Telegram.");
        return;
    }
    match login_arl(state, &state.config.deemix_arl.clone()).await {
        Ok(_) => log::info!("Successfully logged into deemix"),
        Err(e) => log::warn!("Could not login to deemix: {}", e),
    }
}

pub async fn login_arl(state: &Arc<BotState>, arl: &str) -> Result<String, String> {
    let url = format!("{}/api/loginArl", state.config.deemix_url);
    let resp = state
        .http
        .post(&url)
        .json(&serde_json::json!({ "arl": arl }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;

    // status 1 = logged in, status 2 = already logged in
    let status = data["status"].as_i64().unwrap_or(0);
    if status == 1 || status == 2 {
        let username = data["user"]["name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        Ok(username)
    } else {
        Err(format!("Login failed (status {})", status))
    }
}

/// deemix sessions live in an in-memory store with a ~24h TTL, so the bot's
/// cookie session silently expires and POSTs start failing with NotLoggedIn.
/// Re-login with the current ARL so the caller can retry.
async fn relogin(state: &Arc<BotState>) -> Result<(), String> {
    let arl = {
        let file_arl = std::fs::read_to_string("/config/login.json")
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["arl"].as_str().map(|s| s.to_string()));
        
        match file_arl {
            Some(a) if !a.is_empty() => a,
            _ => state.current_arl.lock().await.clone(),
        }
    };
    
    if arl.is_empty() {
        return Err("Session expired and no ARL is configured".to_string());
    }
    
    *state.current_arl.lock().await = arl.clone();
    
    log::warn!("deemix session expired — re-logging in");
    login_arl(state, &arl).await.map(|_| ())
}

pub async fn add_to_queue(state: &Arc<BotState>, url: &str) -> Result<Vec<Value>, String> {
    match add_to_queue_once(state, url).await {
        Err(e) if e.eq_ignore_ascii_case("notloggedin") => {
            relogin(state).await?;
            add_to_queue_once(state, url).await
        }
        other => other,
    }
}

async fn add_to_queue_once(state: &Arc<BotState>, url: &str) -> Result<Vec<Value>, String> {
    let bitrate = *state.current_bitrate.lock().await;
    let endpoint = format!("{}/api/addToQueue", state.config.deemix_url);
    let resp = state
        .http
        .post(&endpoint)
        .json(&serde_json::json!({ "url": url, "bitrate": bitrate }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;

    if data["result"].as_bool().unwrap_or(false) {
        Ok(data["data"]["obj"].as_array().cloned().unwrap_or_default())
    } else {
        Err(data["errid"]
            .as_str()
            .unwrap_or("Unknown error")
            .to_string())
    }
}

/// How long to wait after the HTTP response for the ws reader task to finish buffering the frames the server sent before that response.
const WS_GRACE: Duration = Duration::from_millis(300);

/// Verified outcome of one addToQueue request: the objects deemix actually added (from the HTTP response) plus the duplicates and generation errors that only the WebSocket events report.
#[derive(Debug)]
pub enum QueueOutcome {
    /// Objects were added. `tracks` = summed size, `is_track` = single track (no count suffix), `already`/`failed` = duplicates / failed generations of an artist request (0 for everything else).
    Added { tracks: usize, is_track: bool, already: usize, failed: usize },
    /// Nothing added — the item is already in the queue (`more` = further duplicates, artist links only).
    AlreadyInQueue { title: String, artist: String, size: u64, more: usize },
    /// Nothing added — the server rejected the link (WS queueError).
    Failed { error: String, errid: Option<String> },
    /// result:true but no objects and no matching events — the actual outcome is undetermined.
    NothingAdded,
}

/// addToQueue plus verification via the WS event buffer. `artist` must be true only for artist links — the only kind resolving to several download objects, where added / duplicate / failed can mix within one request.
pub async fn add_to_queue_confirmed(
    state: &Arc<BotState>,
    url: &str,
    artist: bool,
) -> Result<QueueOutcome, String> {
    let started = Instant::now();
    let added = add_to_queue(state, url).await?;

    // The server emits its WS events before answering the HTTP request, so give the reader task a moment to buffer them before scanning.
    tokio::time::sleep(WS_GRACE).await;

    // Drain the events of this request's time window.
    let mut errors: Vec<(Option<String>, String, Option<String>)> = Vec::new();
    let mut dupes: Vec<(String, String, u64)> = Vec::new();
    {
        let mut buf = state.ws_events.lock().await;
        while let Some(ev) = buf.pop_front() {
            if ev.ts < started {
                continue; // stale event of an earlier request — discard
            }
            match ev.kind {
                WsEventKind::QueueError { link, error, errid } => errors.push((link, error, errid)),
                WsEventKind::AlreadyInQueue { title, artist: artist_name, size } => {
                    dupes.push((title, artist_name, size))
                }
            }
        }
    }

    if !added.is_empty() {
        let tracks: usize = added.iter().map(|o| o["size"].as_u64().unwrap_or(1) as usize).sum();
        let is_track = added.len() == 1 && added[0]["type"].as_str() == Some("track");
        return Ok(if artist {
            QueueOutcome::Added { tracks, is_track, already: dupes.len(), failed: errors.len() }
        } else {
            // A single-object request that added its object cannot also have produced duplicates or errors; any buffered events belong to concurrent WebUI activity and are ignored.
            QueueOutcome::Added { tracks, is_track, already: 0, failed: 0 }
        });
    }

    if artist {
        if let Some((_, error, errid)) = errors.first() {
            return Ok(QueueOutcome::Failed { error: error.clone(), errid: errid.clone() });
        }
        if let Some((title, artist_name, size)) = dupes.first() {
            return Ok(QueueOutcome::AlreadyInQueue {
                title: title.clone(),
                artist: artist_name.clone(),
                size: *size,
                more: dupes.len() - 1,
            });
        }
    } else {
        // Exactly one object was expected: it either failed to generate (queueError — matched by link when possible), was a duplicate, or nothing happened at all.
        if let Some((_, error, errid)) = errors.iter().find(|(l, _, _)| l.as_deref() == Some(url)) {
            return Ok(QueueOutcome::Failed { error: error.clone(), errid: errid.clone() });
        }
        if let Some((title, artist_name, size)) = dupes.first() {
            return Ok(QueueOutcome::AlreadyInQueue {
                title: title.clone(),
                artist: artist_name.clone(),
                size: *size,
                more: 0,
            });
        }
        if let Some((_, error, errid)) = errors.iter().find(|(l, _, _)| l.is_none()) {
            return Ok(QueueOutcome::Failed { error: error.clone(), errid: errid.clone() });
        }
    }

    Ok(QueueOutcome::NothingAdded)
}

pub struct QueueStatus {
    pub pending: usize,
    pub downloading: usize,
    pub done: usize,
    pub failed: usize,
}

pub async fn get_queue(state: &Arc<BotState>) -> Result<QueueStatus, String> {
    let url = format!("{}/api/getQueue", state.config.deemix_url);
    let resp = state
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;
    let mut pending = 0usize;
    let mut downloading = 0usize;
    let mut done = 0usize;
    let mut failed = 0usize;

    if let Some(queue) = data["queue"].as_object() {
        for item in queue.values() {
            match item["status"].as_str() {
                Some("completed") | Some("withErrors") => done += 1,
                Some("failed") => failed += 1,
                Some("downloading") => downloading += 1,
                Some("inQueue") => pending += 1,
                // Older deemix builds don't expose status — fall back to counters
                _ => {
                    let progress = item["progress"].as_u64().unwrap_or(0);
                    let downloaded = item["downloaded"].as_u64().unwrap_or(0);
                    let size = item["size"].as_u64().unwrap_or(1);
                    if downloaded >= size && size > 0 {
                        done += 1;
                    } else if progress > 0 {
                        downloading += 1;
                    } else {
                        pending += 1;
                    }
                }
            }
        }
    }

    Ok(QueueStatus { pending, downloading, done, failed })
}

pub async fn clear_completed(state: &Arc<BotState>) -> Result<usize, String> {
    let url = format!("{}/api/getQueue", state.config.deemix_url);
    let resp = state.http.get(&url).send().await.map_err(|e| e.to_string())?;
    let data: Value = resp.json().await.map_err(|e| e.to_string())?;

    // Items deemix's own removeFinishedDownloads handles (status == "completed") vs. ones we have to remove one by one (failed / withErrors / legacy builds).
    let mut completed = 0usize;
    let mut individual: Vec<String> = Vec::new();
    if let Some(queue) = data["queue"].as_object() {
        for (uuid, item) in queue {
            match item["status"].as_str() {
                Some("completed") => completed += 1,
                Some("failed") | Some("withErrors") => individual.push(uuid.clone()),
                Some(_) => {}
                None => {
                    let downloaded = item["downloaded"].as_u64().unwrap_or(0);
                    let size = item["size"].as_u64().unwrap_or(1);
                    if downloaded >= size && size > 0 {
                        individual.push(uuid.clone());
                    }
                }
            }
        }
    }

    let mut cleared = 0usize;

    if completed > 0 {
        let url = format!("{}/api/removeFinishedDownloads", state.config.deemix_url);
        let resp = state.http.post(&url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("removeFinishedDownloads failed: {}", resp.status()));
        }
        cleared += completed;
    }

    // removeFromQueue takes the uuid as a query parameter, not a JSON body
    for uuid in &individual {
        let url = format!("{}/api/removeFromQueue", state.config.deemix_url);
        let ok = state.http
            .post(&url)
            .query(&[("uuid", uuid.as_str())])
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            cleared += 1;
        }
    }

    Ok(cleared)
}

pub async fn search(
    state: &Arc<BotState>,
    query: &str,
    search_type: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{}/api/search", state.config.deemix_url);
    let resp = state
        .http
        .get(&url)
        .query(&[("term", query), ("type", search_type)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;

    let results = data["data"]
        .as_array()
        .or_else(|| {
            data["results"]["data"].as_array()
        })
        .cloned()
        .unwrap_or_default();

    Ok(results.into_iter().take(8).collect())
}
