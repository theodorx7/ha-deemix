//! Core configuration, shared state and dialogue types for the telegram bot.
//!
//! - `Config`: per-run static configuration assembled from /config files
//!   and ENV variables.
//! - `BotState`: shared runtime state (users db, pending voices, bitrate, ARL).
//! - `State` / `MyDialogue`: teloxide dialogue machine types.
//! - `Command`: bot command enum used by dptree dispatching.

use std::collections::{HashMap, VecDeque};
use std::env;
use std::sync::Arc;

use reqwest::Client;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;

use crate::users::UsersDb;

// ── Dialogue State ────────────────────────────────────────────────────────────
#[derive(Clone, Default, Debug)]
pub enum State {
    #[default]
    Idle,
    AwaitingArl,
    AwaitingSearch,
    AwaitingAlbum,
    AwaitingVoiceTranscribe,
    AwaitingVoiceRecognize,
}

pub(crate) type MyDialogue = Dialogue<State, InMemStorage<State>>;

// ── Config ────────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct Config {
    pub deemix_url: String,
    pub deemix_arl: String,
    pub users_file: String,
    pub audd_api_key: String,
    pub openai_api_key: String,
    pub whisper_url: String,
    pub deemix_bitrate: u8,
    pub deemix_bitrate_lock: bool,
    pub whitelist_enabled: bool,
    pub whitelist_ids: Vec<i64>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            deemix_url: env::var("DEEMIX_URL")
                .unwrap_or_else(|_| "http://localhost:6595".to_string()),
            deemix_arl: std::fs::read_to_string("/config/login.json")
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v["arl"].as_str().map(|s| s.to_string()))
                .unwrap_or_default(),
            users_file: env::var("USERS_FILE")
                .unwrap_or_else(|_| "/config/telegram_bot/users.json".to_string()),
            audd_api_key: env::var("AUDD_API_KEY").unwrap_or_default(),
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
            whisper_url: env::var("WHISPER_URL").unwrap_or_default(),
            deemix_bitrate: env::var("BOT_BITRATE")
                .unwrap_or_else(|_| "9".to_string())
                .parse()
                .unwrap_or(9),
            deemix_bitrate_lock: env::var("DEEMIX_BITRATE_LOCK").unwrap_or_else(|_| "false".to_string()).to_lowercase() == "true",
            whitelist_enabled: env::var("WHITELIST_ENABLED").unwrap_or_else(|_| "true".to_string()).to_lowercase() == "true",
            whitelist_ids: env::var("WHITELIST_IDS").unwrap_or_default()
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<i64>().ok())
                .collect(),
        }
    }


    pub fn audd_enabled(&self) -> bool { !self.audd_api_key.is_empty() }
    pub fn whisper_enabled(&self) -> bool { !self.openai_api_key.is_empty() || !self.whisper_url.is_empty() }
    pub fn is_user_allowed(&self, user_id: i64) -> bool {
        !self.whitelist_enabled || self.whitelist_ids.contains(&user_id)
    }
    pub fn is_whitelist_empty(&self) -> bool {
        self.whitelist_enabled && self.whitelist_ids.is_empty()
    }
}

// ── Deemix WS Events ─────────────────────────────────────────────────────────
/// One deemix WebSocket event buffered by the ws listener, stamped on
/// arrival. Only the two outcomes the HTTP addToQueue response cannot
/// report are kept; see telegram_bot/ws.rs.
pub struct WsEvent {
    pub ts: std::time::Instant,
    pub kind: WsEventKind,
}

pub enum WsEventKind {
    /// The server rejected a link while generating download objects.
    QueueError { link: Option<String>, error: String, errid: Option<String> },
    /// The requested object is already in the download queue.
    AlreadyInQueue { title: String, artist: String, size: u64 },
}

// ── Bot State ─────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct BotState {
    pub config: Arc<Config>,
    pub http: Client,
    pub users: UsersDb,
    pub pending_voices: Arc<Mutex<HashMap<String, String>>>, // short_id -> file_id
    pub current_bitrate: Arc<Mutex<u8>>, // runtime-changeable bitrate
    pub current_arl: Arc<Mutex<String>>, // updated via /updatearl, used for auto re-login
    /// Recent deemix WS events (queueError / alreadyInQueue), consumed by add_to_queue_confirmed.
    pub ws_events: Arc<Mutex<VecDeque<WsEvent>>>,
}

impl BotState {
    pub fn new(config: Config, users: UsersDb) -> Self {
        let http = Client::builder()
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        let default_bitrate = config.deemix_bitrate;
        let default_arl = config.deemix_arl.clone();
        Self {
            config: Arc::new(config),
            http,
            users,
            pending_voices: Arc::new(Mutex::new(HashMap::new())),
            current_bitrate: Arc::new(Mutex::new(default_bitrate)),
            current_arl: Arc::new(Mutex::new(default_arl)),
            ws_events: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub(crate) enum Command {
    Start,
    Help,
    Status,
    Search,
    Album,
    Clearqueue,
    Menu,
    Settings,
    Updatearl,
}
