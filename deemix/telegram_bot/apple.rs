//! Apple Music link handling: songs and albums.
//!
//! Resolves an Apple Music URL to title + artist via the free iTunes lookup
//! API (no key required), mirroring the Spotify metadata approach.

use std::sync::LazyLock;

use regex::Regex;
use reqwest::Client;
use serde_json::Value;

// music.apple.com/{storefront}/song/{slug}/{id} — storefront and slug optional
pub(crate) static APPLE_SONG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"(?i)https?://music\.apple\.com/(?:([a-z]{2})/)?song/(?:[^/?#]+/)?(\d+)"
).unwrap());
pub(crate) static APPLE_ALBUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"(?i)https?://music\.apple\.com/(?:([a-z]{2})/)?album/(?:[^/?#]+/)?(\d+)"
).unwrap());
// ?i= track id in an album link — the shared "song from album" form
static TRACK_IN_ALBUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"[?&]i=(\d+)"
).unwrap());

pub struct AppleMeta {
    pub query: String,
    pub label: String,
    pub search_type: &'static str, // "track" | "album"
}

/// Resolve an Apple Music URL to title + artist via the iTunes lookup API.
/// An album link carrying a `?i=` track id resolves to that single song.
pub async fn resolve(url: &str) -> Option<AppleMeta> {
    let (caps, is_song) = match APPLE_SONG_RE.captures(url) {
        Some(c) => (c, true),
        None => (APPLE_ALBUM_RE.captures(url)?, false),
    };
    let country = caps.get(1).map(|m| m.as_str().to_lowercase());
    let mut id = caps.get(2)?.as_str().to_string();
    // An album link carrying ?i= points at a single track of that album
    if !is_song {
        if let Some(t) = TRACK_IN_ALBUM_RE.captures(url).and_then(|c| c.get(1)) {
            id = t.as_str().to_string();
        }
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to build HTTP client");
    lookup(&client, &id, country.as_deref()).await
}

/// Query the iTunes lookup API for a numeric catalog id and build search
/// metadata from the response. The response's wrapperType tells whether the
/// id resolved to a single track or a whole collection.
async fn lookup(client: &Client, id: &str, country: Option<&str>) -> Option<AppleMeta> {
    let mut req = client.get("https://itunes.apple.com/lookup").query(&[("id", id)]);
    if let Some(c) = country {
        req = req.query(&[("country", c)]);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => { log::info!("[apple] iTunes lookup request failed (id={}): {}", id, e); return None; }
    };
    let data: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => { log::info!("[apple] iTunes lookup response parse failed (id={}): {}", id, e); return None; }
    };
    let first = match data["results"].as_array().and_then(|a| a.first()) {
        Some(f) => f,
        None => { log::info!("[apple] iTunes lookup returned no results (id={} country={:?})", id, country); return None; }
    };

    let is_track = first["wrapperType"].as_str() == Some("track");
    let title = if is_track { first["trackName"].as_str()? } else { first["collectionName"].as_str()? };
    let artist = first["artistName"].as_str().unwrap_or("");

    let query = format!("{} {}", title, artist).trim().to_string();
    let label = if artist.is_empty() { title.to_string() } else { format!("{} — {}", title, artist) };
    let search_type: &'static str = if is_track { "track" } else { "album" };

    Some(AppleMeta { query, label, search_type })
}
