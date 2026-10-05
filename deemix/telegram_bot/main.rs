use std::env;
use std::sync::{Arc, LazyLock};

use regex::Regex;
use reqwest::Client;
use teloxide::{
    dispatching::dialogue::InMemStorage,
    prelude::*,
    types::{
        CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup,
    },
};

mod config;
mod apple;
mod spotify;
mod streaming;
mod deemix;
mod users;
mod voice;
mod ws;

pub(crate) use config::{BotState, MyDialogue};
use config::{Command, Config, State};
use voice::receive_voice_recognize;
use streaming::{
    handle_streaming_link, SPOTIFY_ALBUM_RE,
    SPOTIFY_PLAYLIST_RE, SPOTIFY_TRACK_RE,
};
use apple::{APPLE_ALBUM_RE, APPLE_SONG_RE};

// ── URL Patterns ──────────────────────────────────────────────────────────────
// Two layers: extraction (pull a clean URL out of arbitrary message text) and classification (route a clean URL to the right handler below). Deezer classification patterns:
static DEEZER_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"(?i)https?://(?:www\.)?deezer\.com/(?:[a-z]+/)?(track|album|playlist|artist)/(\d+)"
).unwrap());
static DEEZER_SHORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"(?i)https?://(?:link\.deezer\.com/s/|deezer\.page\.link/)\S+"
).unwrap());
// Extraction layer (used by extract_link):
static ANY_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"(?i)\bhttps?://\S+"
).unwrap());
// A bare web address without a scheme — only counts as a URL when a path follows the TLD ("deezer.com/track/123"); a bare "word.word" without a slash stays plain text and goes to search
static BARE_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"(?i)(?:^|\W)((?:[\w-]+\.)+[a-z]{2,}/\S*)"
).unwrap());

// ── Keyboards ─────────────────────────────────────────────────────────────────
// Reply-keyboard labels — the single source for the keyboard builders below and the match arms in the message handler: Telegram echoes the exact button text
// back as the message text, so both sides must use the same bytes. Toggle rows get an " ON"/" OFF" suffix appended at build time.
const BTN_SEARCH_TRACK: &str = "🔍 Search a track";
const BTN_SEARCH_ALBUM: &str = "💿 Search an album";
const BTN_RECOGNIZE_SONG: &str = "🎵 Recognize song";
const BTN_CHECK_STATUS: &str = "📊 Check status";
const BTN_CLEAR_QUEUE: &str = "🧹 Clear queue";
const BTN_SETTINGS: &str = "⚙️ Settings";
const BTN_HELP: &str = "ℹ️ Help";
const BTN_BACK_MENU: &str = "🔙 Back to menu";
const BTN_UPDATE_ARL: &str = "🔑 Update ARL";
const BTN_RESTART_NOTIF_ON: &str = "🔔 Restart notifications:";
const BTN_RESTART_NOTIF_OFF: &str = "🔕 Restart notifications:";
const BTN_QUALITY: &str = "🎚️ Quality:";
const BTN_QUALITY_LOCKED: &str = "🔒 Quality:";

const ARL_PROMPT: &str = "Please send your new Deezer ARL:";

const HELP_TEXT: &str = "ℹ️ What I do?\n\n\
I connect to your personal deemix server and queue music downloads for you. Just tell me what you want!\n\n\
📥 Ways to request music:\n\
• Type any song or artist name → search and pick from results\n\
• Send a Deezer link (track, album, playlist, artist) → queued instantly\n\
• Send a Spotify or Apple Music link (track or album) → found on Deezer and queued\n\
• Send a Spotify playlist link → every track is scanned and queued individually\n\
• Send an audio recording → I'll recognize the song and offer it for download\n\n\
🔧 All commands:\n\
/menu — quick action buttons\n\
/search — search for a track\n\
/album — search for an album\n\
/status — check download queue\n\
/clearqueue — clear completed downloads from queue\n\
/settings — manage your personal preferences\n\
/updatearl — update your Deezer ARL\n\n\
⚙️ Settings (via /settings):\n\
• Restart notifications — get notified when the bot restarts\n\n\
💡 Tip: You don't need commands — just send a song name or link directly!";

/// Label for a deemix bitrate value (9 = FLAC, 3/1 = MP3).
fn bitrate_label(bitrate: u8) -> &'static str {
    match bitrate {
        9 => "FLAC (lossless)",
        3 => "MP3 320kbps",
        1 => "MP3 128kbps",
        _ => "Unknown",
    }
}

/// Cycle through the supported bitrates: FLAC → MP3 320 → MP3 128 → FLAC.
fn next_bitrate(current: u8) -> u8 {
    match current { 9 => 3, 3 => 1, _ => 9 }
}

fn settings_keyboard(s: &users::UserSettings, config: &Config, bitrate: u8) -> KeyboardMarkup {
    let notif = if s.restart_notifications { format!("{} ON", BTN_RESTART_NOTIF_ON) } else { format!("{} OFF", BTN_RESTART_NOTIF_OFF) };
    let bitrate_btn = if config.deemix_bitrate_lock {
        format!("{} {} (locked)", BTN_QUALITY_LOCKED, bitrate_label(bitrate))
    } else {
        format!("{} {} (tap to change)", BTN_QUALITY, bitrate_label(bitrate))
    };

    KeyboardMarkup::new(vec![
        vec![KeyboardButton::new(notif)],
        vec![KeyboardButton::new(bitrate_btn)],
        vec![KeyboardButton::new(BTN_UPDATE_ARL)],
        vec![KeyboardButton::new(BTN_BACK_MENU)],
    ])
    .resize_keyboard()
}

fn arl_cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback("❌ Cancel", "cancel_arl")]])
}

fn main_keyboard(config: &Config) -> KeyboardMarkup {
    let mut rows = vec![
        vec![
            KeyboardButton::new(BTN_SEARCH_TRACK),
            KeyboardButton::new(BTN_SEARCH_ALBUM),
        ],
    ];

    if config.acrcloud_enabled() {
        rows.push(vec![KeyboardButton::new(BTN_RECOGNIZE_SONG)]);
    }

    rows.push(vec![
        KeyboardButton::new(BTN_CHECK_STATUS),
        KeyboardButton::new(BTN_CLEAR_QUEUE),
    ]);
    rows.push(vec![
        KeyboardButton::new(BTN_SETTINGS),
        KeyboardButton::new(BTN_HELP),
    ]);

    KeyboardMarkup::new(rows).resize_keyboard()
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    env_logger::init();

    let config = Config::from_env();
    if config.is_whitelist_empty() {
        log::warn!("Whitelist is enabled but WHITELIST_IDS is empty. All users will be blocked until configured.");
    }
    let users = users::load(&config.users_file);
    let state = Arc::new(BotState::new(config, users));

    let token = env::var("TELEGRAM_TOKEN").expect("TELEGRAM_TOKEN must be set");
    let bot = Bot::new(token);

    // Background WebSocket listener for deemix queue events (queueError / alreadyInQueue buffering + download-error forwarding) — see telegram_bot/ws.rs
    tokio::spawn(ws::run_ws_listener(bot.clone(), Arc::clone(&state)));

    deemix::login(&state).await;

    log::info!("Telegram bot starting...");

    // Send startup notification to users with restart_notifications enabled
    {
        let startup_msg = "🎵 Deemix is back online!\n\nTap /menu for quick actions.";
        for chat_id in users::all_with_notifications(&state.users) {
            let _ = bot.send_message(teloxide::types::ChatId(chat_id), startup_msg).await;
        }
    }

    let storage = InMemStorage::<State>::new();

    let handler = dptree::entry()
        // Whitelist: block unauthorized messages
        .branch(
            Update::filter_message()
            .filter(|msg: Message, state: Arc<BotState>| {
                let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
                !state.config.is_user_allowed(user_id)
            })
            .endpoint(handle_unauthorized_message),
        )
        // Whitelist: block unauthorized callbacks
        .branch(
            Update::filter_callback_query()
            .filter(|query: CallbackQuery, state: Arc<BotState>| {
                !state.config.is_user_allowed(query.from.id.0 as i64)
            })
            .endpoint(handle_unauthorized_callback),
        )
        .branch(
            Update::filter_message()
                .enter_dialogue::<Message, InMemStorage<State>, State>()
                .branch(dptree::case![State::AwaitingArl].endpoint(receive_arl))
                .branch(dptree::case![State::AwaitingSearch].endpoint(receive_search))
                .branch(dptree::case![State::AwaitingAlbum].endpoint(receive_album))
                .branch(dptree::case![State::AwaitingVoiceRecognize].endpoint(receive_voice_recognize))
                .branch(
                    Update::filter_message()
                        .filter_command::<Command>()
                        .endpoint(handle_command),
                )
                .branch(
                    Update::filter_message()
                        .endpoint(handle_message),
                ),
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(handle_callback),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![Arc::clone(&state), storage])
        .build()
        .dispatch()
        .await;
}

// ── Helpers ───────────────────────────────────────────────────────────────────
pub(crate) fn user_id_from_msg(msg: &Message) -> i64 {
    msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0)
}

// ── Unauthorized Handlers ────────────────────────────────────────────────────
async fn handle_unauthorized_message(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> ResponseResult<()> {
    if state.config.is_whitelist_empty() {
        bot.send_message(
            msg.chat.id,
            "⚠️ The user list is empty.\nAdd users to the whitelist or disable the user filtering feature.",
        ).await?;
    }
    Ok(())
}

async fn handle_unauthorized_callback(
    bot: Bot,
    query: CallbackQuery,
    state: Arc<BotState>,
) -> ResponseResult<()> {
    if state.config.is_whitelist_empty() {
        bot.answer_callback_query(query.id.clone())
            .text("⚠️ The user list is empty.\nAdd users to the whitelist or disable the user filtering feature.")
            .await?;
    }
    Ok(())
}

// ── Command Handler ───────────────────────────────────────────────────────────
async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<BotState>,
    dialogue: MyDialogue,
) -> ResponseResult<()> {
    // Auto-create user on any interaction
    let user_settings = users::get_or_create(&state.users, &state.config.users_file, user_id_from_msg(&msg));

    match cmd {
        Command::Start => {
            let kb = main_keyboard(&state.config);
            bot.send_message(
                msg.chat.id,
                "👋 Hey! I'm your personal music download assistant.\n\nJust send me a song name or a link from Deezer, Spotify, or Apple Music, and I'll find it and queue it for download on your server. No technical stuff needed!\n\n📲 Use /menu to see quick action buttons.\n\nFor a full list of what I can do, type /help.",
            )
            .reply_markup(kb)
            .await?;
        }

        Command::Help => {
            bot.send_message(msg.chat.id, HELP_TEXT).await?;
        }

        Command::Status => {
            do_status(&bot, &msg, &state).await?;
        }

        Command::Search => {
            prompt_search(&bot, &msg, &dialogue).await?;
        }

        Command::Album => {
            prompt_album(&bot, &msg, &dialogue).await?;
        }

        Command::Clearqueue => {
            do_clearqueue(&bot, &msg, &state).await?;
        }

        Command::Menu => {
            show_menu(&bot, &msg, &state).await?;
        }

        Command::Settings => {
            show_settings(&bot, &msg, &state, &user_settings, "⚙️ Your settings — tap to toggle:").await?;
        }

        Command::Updatearl => {
            enter_arl(&bot, &msg, &dialogue).await?;
        }
    }

    Ok(())
}

// ── Dialogue Receivers ────────────────────────────────────────────────────────
async fn receive_search(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    dialogue.exit().await.ok();
    if let Some(query) = msg.text() { do_search(&bot, &msg, &state, query.trim(), "track").await?; }
    Ok(())
}

async fn receive_album(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    dialogue.exit().await.ok();
    if let Some(query) = msg.text() { do_search(&bot, &msg, &state, query.trim(), "album").await?; }
    Ok(())
}

async fn receive_arl(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    let arl = match msg.text() {
        Some(t) => t.trim().to_string(),
        None => { bot.send_message(msg.chat.id, "Please send the ARL as text.").await?; return Ok(()); }
    };
    if arl.len() < 100 {
        bot.send_message(msg.chat.id, "❌ That ARL looks too short, double check it. Try again:")
            .reply_markup(arl_cancel_keyboard())
            .await?;
        return Ok(());
    }
    dialogue.exit().await.ok();
    handle_updatearl(&bot, &msg, &state, &arl).await?;
    Ok(())
}

/// Handle quality change button press
async fn handle_quality_change(
    bot: &Bot,
    msg: &Message,
    state: &Arc<BotState>,
) -> ResponseResult<()> {
    if state.config.deemix_bitrate_lock {
        bot.send_message(msg.chat.id, "🔒 Download quality is locked by the administrator.").await?;
        return Ok(());
    }
    
    let new_bitrate = {
        let mut br = state.current_bitrate.lock().await;
        *br = next_bitrate(*br);
        *br
    };
    let updated = users::get_or_create(&state.users, &state.config.users_file, user_id_from_msg(&msg));
    let text = format!(
        "🎚️ Download quality changed to: {}\n\n⚠️ This affects ALL users of this bot.",
        bitrate_label(new_bitrate)
    );
    show_settings(bot, msg, state, &updated, &text).await?;
    Ok(())
}

// ── Message Handler ───────────────────────────────────────────────────────────
async fn handle_message(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    // Auto-create user
    let user_settings = users::get_or_create(&state.users, &state.config.users_file, user_id_from_msg(&msg));

    // ── Voice note handling ──
    if msg.voice().is_some() {
        return voice::handle_voice_message(bot, msg, state).await;
    }

    let text = match msg.text() {
        Some(t) => t.trim().to_string(),
        None => return Ok(()),
    };

    // ── Keyboard button presses ──
    match text.as_str() {
        BTN_SEARCH_TRACK => {
            prompt_search(&bot, &msg, &dialogue).await?;
            return Ok(());
        }
        BTN_SEARCH_ALBUM => {
            prompt_album(&bot, &msg, &dialogue).await?;
            return Ok(());
        }
        BTN_RECOGNIZE_SONG => {
            if !state.config.acrcloud_enabled() {
                bot.send_message(msg.chat.id, "⚠️ Song recognition is not configured.").await?;
            } else {
                dialogue.update(State::AwaitingVoiceRecognize).await.ok();
                bot.send_message(msg.chat.id, "🎵 Send me an audio recording of a song and I'll identify it.\n\n⏱ You have 60 seconds.").await?;
                // Spawn timeout to reset dialogue after 60s
                let dialogue_clone = dialogue.clone();
                let chat_id = msg.chat.id;
                let bot_clone = bot.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    if let Ok(Some(State::AwaitingVoiceRecognize)) = dialogue_clone.get().await {
                        dialogue_clone.exit().await.ok();
                        let _ = bot_clone.send_message(chat_id, "⏱ Song recognition timed out. Send an audio recording or use /menu to start again.").await;
                    }
                });
            }
            return Ok(());
        }
        BTN_CHECK_STATUS => {
            do_status(&bot, &msg, &state).await?;
            return Ok(());
        }
        BTN_CLEAR_QUEUE => {
            do_clearqueue(&bot, &msg, &state).await?;
            return Ok(());
        }
        BTN_SETTINGS => {
            show_settings(&bot, &msg, &state, &user_settings, "⚙️ Your settings — tap to toggle:").await?;
            return Ok(());
        }
        BTN_BACK_MENU => {
            show_menu(&bot, &msg, &state).await?;
            return Ok(());
        }
        BTN_UPDATE_ARL => {
            enter_arl(&bot, &msg, &dialogue).await?;
            return Ok(());
        }
        BTN_HELP => {
            bot.send_message(msg.chat.id, HELP_TEXT).await?;
            return Ok(());
        }
        // Settings toggles
        t if t.starts_with(BTN_RESTART_NOTIF_ON) || t.starts_with(BTN_RESTART_NOTIF_OFF) => {
            let saved = users::update(&state.users, &state.config.users_file, user_id_from_msg(&msg), |s| {
                s.restart_notifications = !s.restart_notifications;
            });
            let updated = users::get_or_create(&state.users, &state.config.users_file, user_id_from_msg(&msg));
            let status = if updated.restart_notifications { "ON" } else { "OFF" };
            let mut text = format!("{} {}", BTN_RESTART_NOTIF_ON, status);
            if let Err(e) = saved {
                text.push_str(&format!("\n⚠️ Failed to save settings: {} — the change will be lost on restart.", e));
            }
            show_settings(&bot, &msg, &state, &updated, &text).await?;
            return Ok(());
        }
        t if t.starts_with(BTN_QUALITY) || t.starts_with(BTN_QUALITY_LOCKED) => {
            handle_quality_change(&bot, &msg, &state).await?;
            return Ok(());
        }
        _ => {}
    }

    // ── Link extraction ──
    // Pull the first link out of the message (an explicit http(s) URL or a bare web address carrying a path) so the handlers below always receive a clean URL instead of the whole message text.
    let Some(link) = extract_link(&text) else {
        // No link found — treat the whole message as a search query
        do_search(&bot, &msg, &state, &text, "track").await?;
        return Ok(());
    };
    
    // ── Streaming service URLs ──
    if SPOTIFY_TRACK_RE.is_match(&link) || SPOTIFY_ALBUM_RE.is_match(&link)
        || SPOTIFY_PLAYLIST_RE.is_match(&link)
        || APPLE_SONG_RE.is_match(&link) || APPLE_ALBUM_RE.is_match(&link)
    {
        handle_streaming_link(&bot, &msg, &state, &link).await?;
        return Ok(());
    }

    // ── Deezer short URL ──
    if DEEZER_SHORT_RE.is_match(&link) {
        let resolved = resolve_short_link(&state.http, &link).await;
        if let Some(url) = resolved {
            queue_url(&bot, &msg, &state, &url).await?;
        } else {
            bot.send_message(msg.chat.id, "❌ Could not resolve that link.").await?;
        }
        return Ok(());
    }

    // ── Full Deezer URL ──
    if DEEZER_URL_RE.is_match(&link) {
        queue_url(&bot, &msg, &state, &link).await?;
        return Ok(());
    }

    // ── Unknown link ──
    bot.send_message(
        msg.chat.id,
        "🤷 Unsupported link.",
    )
    .await?;
    Ok(())
}

// ── Callback Handler ──────────────────────────────────────────────────────────
async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    storage: Arc<InMemStorage<State>>,
) -> ResponseResult<()> {
    bot.answer_callback_query(q.id.clone()).await?;

    let data = match &q.data {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    if data == "cancel" {
        if let Some(msg) = &q.message {
            bot.edit_message_text(msg.chat().id, msg.id(), "Cancelled.").await?;
        }
        return Ok(());
    }

    if data == "add_arl" {
        if let Some(msg) = &q.message {
            let dialogue: MyDialogue = Dialogue::new(storage, msg.chat().id);
            let _ = dialogue.update(State::AwaitingArl).await;
            bot.edit_message_text(msg.chat().id, msg.id(), ARL_PROMPT)
                .reply_markup(arl_cancel_keyboard())
                .await?;
        }
        return Ok(());
    }
    
    if data == "cancel_arl" {
        if let Some(msg) = &q.message {
            let dialogue: MyDialogue = Dialogue::new(storage, msg.chat().id);
            dialogue.exit().await.ok();
            bot.edit_message_text(msg.chat().id, msg.id(), "❌ ARL update cancelled.").await?;
        }
        return Ok(());
    }

    if let Some(url) = data.strip_prefix("dl:") {
        if let Some(msg) = &q.message {
            let kind = DEEZER_URL_RE.captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_else(|| "item".to_string());
            let artist = kind == "artist";
            let chat_id = msg.chat().id.0;
            bot.edit_message_text(msg.chat().id, msg.id(), format!("⏳ Queuing {}...", kind)).await?;
            match deemix::add_to_queue_confirmed(&state, url, artist, chat_id).await {
                Ok(oc) => { bot.edit_message_text(msg.chat().id, msg.id(), format_queue_outcome(&capitalize(&kind), &oc)).await?; }
                Err(e) => {
                    let req = bot.edit_message_text(msg.chat().id, msg.id(), format!("❌ Failed: {}", e));
                    let req = if e == deemix::ARL_ERROR { req.reply_markup(add_arl_keyboard()) } else { req };
                    req.await?;
                }
            }
        }
    }

    Ok(())
}

// ── Shared Action Helpers ────────────────────────────────────────────────────
// One home per bot action, shared by the /commands and the keyboard buttons.

/// Switch the dialogue to track-search mode and ask for the query.
async fn prompt_search(bot: &Bot, msg: &Message, dialogue: &MyDialogue) -> ResponseResult<()> {
    dialogue.update(State::AwaitingSearch).await.ok();
    bot.send_message(msg.chat.id, "🔍 What song or artist are you looking for?").await?;
    Ok(())
}

/// Switch the dialogue to album-search mode and ask for the query.
async fn prompt_album(bot: &Bot, msg: &Message, dialogue: &MyDialogue) -> ResponseResult<()> {
    dialogue.update(State::AwaitingAlbum).await.ok();
    bot.send_message(msg.chat.id, "💿 What album are you looking for?").await?;
    Ok(())
}

/// Switch the dialogue to ARL input mode (with the cancel button).
async fn enter_arl(bot: &Bot, msg: &Message, dialogue: &MyDialogue) -> ResponseResult<()> {
    dialogue.update(State::AwaitingArl).await.ok();
    bot.send_message(msg.chat.id, ARL_PROMPT)
        .reply_markup(arl_cancel_keyboard())
        .await?;
    Ok(())
}

/// Show the main menu keyboard (/menu and the "Back to menu" button).
async fn show_menu(bot: &Bot, msg: &Message, state: &Arc<BotState>) -> ResponseResult<()> {
    let kb = main_keyboard(&state.config);
    bot.send_message(msg.chat.id, "Choose an action:").reply_markup(kb).await?;
    Ok(())
}

/// Send `text` with the settings keyboard reflecting `s` and the runtime bitrate.
async fn show_settings(
    bot: &Bot,
    msg: &Message,
    state: &Arc<BotState>,
    s: &users::UserSettings,
    text: &str,
) -> ResponseResult<()> {
    let current_br = *state.current_bitrate.lock().await;
    let kb = settings_keyboard(s, &state.config, current_br);
    bot.send_message(msg.chat.id, text).reply_markup(kb).await?;
    Ok(())
}

/// Clear completed downloads and report the outcome (/clearqueue and the button).
async fn do_clearqueue(bot: &Bot, msg: &Message, state: &Arc<BotState>) -> ResponseResult<()> {
    match deemix::clear_completed(&state).await {
        Ok(0) => { bot.send_message(msg.chat.id, "📭 No completed downloads to clear.").await?; }
        Ok(n) => { bot.send_message(msg.chat.id, format!("🧹 Cleared {} completed download(s) from queue.", n)).await?; }
        Err(e) => { bot.send_message(msg.chat.id, format!("❌ Failed to clear queue: {}", e)).await?; }
    }
    Ok(())
}

// ── Core Helpers ──────────────────────────────────────────────────────────────
async fn resolve_short_link(http: &Client, url: &str) -> Option<String> {
    let resp = http.head(url).send().await.ok()?;
    Some(resp.url().to_string())
}

/// Extract the first link from a user message: an explicit http(s) URL of any host, or a bare web address carrying a path (e.g. "deezer.com/track/123"), which gets an "https://" prefix added.
fn extract_link(text: &str) -> Option<String> {
    if let Some(m) = ANY_URL_RE.find(text) {
        return Some(trim_trailing_punct(m.as_str()));
    }
    BARE_URL_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| format!("https://{}", trim_trailing_punct(m.as_str())))
}

/// Strip trailing punctuation that belongs to the surrounding sentence, not the URL itself (e.g. "…link: https://…/xyz." or "(https://…/xyz)").
fn trim_trailing_punct(url: &str) -> String {
    url.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '«', '»'])
        .to_string()
}

async fn queue_url(bot: &Bot, msg: &Message, state: &Arc<BotState>, url: &str) -> ResponseResult<()> {
    let cap = match DEEZER_URL_RE.captures(url) {
        Some(c) => c,
        None => {
            bot.send_message(msg.chat.id, "❌ That doesn't look like a valid Deezer URL.").await?;
            return Ok(());
        }
    };
    let kind = cap.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_else(|| "item".to_string());
    let artist = kind == "artist";
    let chat_id = msg.chat.id.0;
    let sent = bot.send_message(msg.chat.id, format!("⏳ Queuing {}...", kind)).await?;

    match deemix::add_to_queue_confirmed(state, url, artist, chat_id).await {
        Ok(oc) => { bot.edit_message_text(msg.chat.id, sent.id, format_queue_outcome(&capitalize(&kind), &oc)).await?; }
        Err(e) => {
            let req = bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Failed to queue: {}", e));
            let req = if e == deemix::ARL_ERROR { req.reply_markup(add_arl_keyboard()) } else { req };
            req.await?;
        }
    }
    Ok(())
}

/// Telegram truncates long button labels, so the message text carries the full numbered track list and the buttons reference the numbers.
pub(crate) fn build_search_results(results: &[serde_json::Value], icon: &str) -> (String, Vec<Vec<InlineKeyboardButton>>) {
    let mut listing = String::new();
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (i, item) in results.iter().enumerate() {
        // Skip entries without a usable Deezer link — they would produce a dead button
        let link = match item["link"].as_str() {
            Some(l) if !l.is_empty() => l,
            _ => continue,
        };
        let title = item["title"].as_str().unwrap_or("?");
        let artist = item["artist"]["name"].as_str().unwrap_or("?");
        listing.push_str(&format!("{}. {} — {}\n", i + 1, title, artist));
        let label = format!("{} {}. {} — {}", icon, i + 1, title, artist);
        let label = if label.chars().count() > 60 { format!("{}…", label.chars().take(59).collect::<String>()) } else { label };
        buttons.push(vec![InlineKeyboardButton::callback(label, format!("dl:{}", link))]);
    }
    (listing, buttons)
}

/// Search Deezer for `query` and render the outcome into message `msg_id`: `empty_text` on zero results, the numbered results with a Cancel button otherwise, or the search error.
pub(crate) async fn search_and_show(
    bot: &Bot,
    state: &Arc<BotState>,
    chat_id: teloxide::types::ChatId,
    msg_id: teloxide::types::MessageId,
    query: &str,
    search_type: &str,
    empty_text: &str,
) -> ResponseResult<()> {
    match deemix::search(state, query, search_type).await {
        Ok(results) if results.is_empty() => { bot.edit_message_text(chat_id, msg_id, empty_text.to_string()).await?; }
        Ok(results) => {
            let icon = if search_type == "track" { "🎵" } else { "💿" };
            let (listing, mut buttons) = build_search_results(&results, icon);
            buttons.push(vec![InlineKeyboardButton::callback("❌ Cancel", "cancel")]);
            bot.edit_message_text(chat_id, msg_id, format!("Results for {}:\n\n{}\nTap a button to download.", query, listing))
                .reply_markup(InlineKeyboardMarkup::new(buttons))
                .await?;
        }
        Err(e) => { bot.edit_message_text(chat_id, msg_id, format!("❌ Search failed: {}", e)).await?; }
    }
    Ok(())
}

async fn do_search(bot: &Bot, msg: &Message, state: &Arc<BotState>, query: &str, search_type: &str) -> ResponseResult<()> {
    let sent = bot.send_message(msg.chat.id, format!("🔍 Searching for {}...", query)).await?;
    search_and_show(bot, state, msg.chat.id, sent.id, query, search_type, "😕 No results found.").await?;
    Ok(())
}

/// Fetch queue counters from deemix and send them as a message. Shared by the /status command and the "📊 Check status" keyboard button.
async fn do_status(bot: &Bot, msg: &Message, state: &Arc<BotState>) -> ResponseResult<()> {
    match deemix::get_queue(state).await {
        Ok(q) => {
            let mut text = String::new();
            if q.downloading > 0 { text.push_str(&format!("⬇️ Downloading: {}\n", q.downloading)); }
            if q.pending > 0 { text.push_str(&format!("⏳ Pending: {}\n", q.pending)); }
            if q.failed > 0 { text.push_str(&format!("❌ Failed: {}\n", q.failed)); }
            if q.done > 0 { text.push_str(&format!("✅ Completed (in queue): {}", q.done)); }
            if q.downloading == 0 && q.pending == 0 && q.done == 0 && q.failed == 0 { text.push_str("📭 Queue is empty"); }
            bot.send_message(msg.chat.id, text).await?;
        }
        Err(e) => { bot.send_message(msg.chat.id, format!("❌ Failed to get queue status: {}", e)).await?; }
    }
    Ok(())
}

async fn handle_updatearl(bot: &Bot, msg: &Message, state: &Arc<BotState>, arl: &str) -> ResponseResult<()> {
    let sent = bot.send_message(msg.chat.id, "🔄 Validating new ARL...").await?;

    match deemix::login_arl(state, arl).await {
        Ok(_username) => {
            *state.current_arl.lock().await = arl.to_string();
            bot.edit_message_text(msg.chat.id, sent.id, "✅ ARL updated and logged in! Downloads will use the new ARL immediately.").await?;
        }
        Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ ARL rejected by deemix: {}", e)).await?; }
    }
    Ok(())
}

/// Inline keyboard with the single "Add ARL" button, attached to queue-failure messages caused by a missing/invalid ARL.
fn add_arl_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback("➕ Add ARL", "add_arl")]])
}

/// Render a verified addToQueue outcome as the user-facing message. `label` is the capitalized request kind ("Track", "Album", ...).
fn format_queue_outcome(label: &str, oc: &deemix::QueueOutcome) -> String {
    use deemix::QueueOutcome;
    match oc {
        QueueOutcome::Added { tracks, is_track, already, failed } => {
            let mut text = format!("✅ {} added to queue!", label);
            if !is_track {
                text.push_str(&format!(" ({} tracks", tracks));
                if *already > 0 { text.push_str(&format!(", {} already in queue", already)); }
                if *failed > 0 { text.push_str(&format!(", {} not added", failed)); }
                text.push(')');
            }
            text
        }
        QueueOutcome::AlreadyInQueue { title, artist, size, more } => {
            let mut text = format!("ℹ️ Already in queue: {} — {}", title, artist);
            if *size > 1 { text.push_str(&format!(" ({} tracks)", size)); }
            if *more > 0 { text.push_str(&format!(" (+{} more)", more)); }
            text
        }
        QueueOutcome::Failed { error, errid } => match errid {
            Some(errid) => format!("❌ Error adding to queue: {} ({})", error, errid),
            None => format!("❌ Error adding to queue: {}", error),
        },
        QueueOutcome::NothingAdded => "⚠️ Connection to Deemix lost.".to_string(),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
