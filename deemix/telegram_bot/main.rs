use std::env;
use std::sync::Arc;

use regex::Regex;
use reqwest::Client;
use teloxide::{
    dispatching::dialogue::InMemStorage,
    prelude::*,
    types::{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup},
    utils::command::BotCommands,
};

mod config;
mod spotify;
mod keyboards;
mod supervisor;
mod deemix;
mod users;
mod voice;
mod youtube;

pub(crate) use config::{BotState, MyDialogue};
use config::{Command, Config, State};
use users::UserSettings;
use keyboards::{arl_cancel_keyboard, bitrate_label, main_keyboard, next_bitrate, settings_keyboard};
use voice::{receive_voice_recognize, receive_voice_transcribe};

// ── Dialogue State ────────────────────────────────────────────────────────────
#[derive(Clone, Default, Debug)]
pub enum State {
    #[default]
    Idle,
    AwaitingArl,
    AwaitingSearch,
    AwaitingAlbum,
    AwaitingDl,
    AwaitingSpotify,
    AwaitingVoiceTranscribe,
    AwaitingVoiceRecognize,
}

pub(crate) type MyDialogue = Dialogue<State, InMemStorage<State>>;

// ── URL Patterns ──────────────────────────────────────────────────────────────
lazy_static::lazy_static! {
    static ref DEEZER_URL_RE: Regex = Regex::new(
        r"https?://(?:www\.)?deezer\.com/(?:[a-z]+/)?(track|album|playlist|artist)/(\d+)"
    ).unwrap();
    static ref DEEZER_SHORT_RE: Regex = Regex::new(
        r"https?://link\.deezer\.com/s/\S+"
    ).unwrap();
    static ref SPOTIFY_TRACK_RE: Regex = Regex::new(
        r"https?://open\.spotify\.com/track/([A-Za-z0-9]+)"
    ).unwrap();
    static ref SPOTIFY_ALBUM_RE: Regex = Regex::new(
        r"https?://open\.spotify\.com/album/([A-Za-z0-9]+)"
    ).unwrap();
    static ref SPOTIFY_PLAYLIST_RE: Regex = Regex::new(
        r"https?://open\.spotify\.com/playlist/[A-Za-z0-9]+"
    ).unwrap();
    static ref YOUTUBE_RE: Regex = Regex::new(
        r"https?://(?:(?:www\.)?youtube\.com/|youtu\.be/|music\.youtube\.com/)"
    ).unwrap();
    static ref APPLE_MUSIC_RE: Regex = Regex::new(
        r"https?://music\.apple\.com/"
    ).unwrap();
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init();

    let config = Config::from_env();
    if config.is_whitelist_empty() {
        log::warn!("Whitelist is enabled but WHITELIST_IDS is empty. All users will be blocked until configured.");
    }
    let users = users::load(&config.users_file);
    let state = Arc::new(BotState::new(config, users));

    let token = env::var("TELEGRAM_TOKEN").expect("TELEGRAM_TOKEN must be set");
    let bot = Bot::new(token);

    deemix::login(&state).await;

    log::info!("Telegram bot starting...");

    // Send startup notification (skip if quality change restart)
    let quality_flag_path = "/config/telegram_bot/pending_quality_change.json";
    let has_quality_flag = std::path::Path::new(&quality_flag_path).exists();
    
    if !has_quality_flag {
        let startup_msg = "🎵 Deemix is back online!\n\nTap /menu for quick actions.";
        for chat_id in users::all_with_notifications(&state.users) {
            let _ = bot.send_message(teloxide::types::ChatId(chat_id), startup_msg).await;
        }
    }
    // Check for pending quality change notification (from pre-restart)
    {
        let flag_path = "/config/telegram_bot/pending_quality_change.json";
        if let Ok(contents) = std::fs::read_to_string(flag_path) {
            let bitrate = serde_json::from_str::<serde_json::Value>(&contents)
                .ok()
                .and_then(|d| d["bitrate"].as_u64());
            
            if let Some(bitrate) = bitrate {
                let msg_text = format!(
                    "🎚️ Download quality changed to: {}\n\n⚠️ This affects ALL users on this server.",
                    bitrate_label(bitrate as u8)
                );
                
                if let Ok(map) = state.users.read() {
                    for chat_id in map.keys().filter_map(|k| k.parse::<i64>().ok()) {
                        let _ = bot.send_message(teloxide::types::ChatId(chat_id), &msg_text).await;
                    }
                }
            }
            
            // Always remove the flag file
            let _ = std::fs::remove_file(flag_path);
        }
    }
    
    let storage = InMemStorage::<State>::new();

    let handler = dptree::entry()
        // Whitelist: block unauthorized messages
        .branch(
            Update::filter_message()
            .filter(|msg: Message, state: Arc<BotState>| {
                let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
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
                .branch(dptree::case![State::AwaitingDl].endpoint(receive_dl))
                .branch(dptree::case![State::AwaitingSpotify].endpoint(receive_spotify))
                .branch(dptree::case![State::AwaitingVoiceTranscribe].endpoint(receive_voice_transcribe))
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
    msg.from().map(|u| u.id.0 as i64).unwrap_or(0)
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
        bot.answer_callback_query(&query.id)
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
    let user_settings = users::get_or_create(&state.users, user_id_from_msg(&msg));
    users::save(&state.users, &state.config.users_file);

    match cmd {
        Command::Start => {
            let kb = main_keyboard(&user_settings, &state.config);
            bot.send_message(
                msg.chat.id,
                "👋 Hey! I'm your personal music download assistant.\n\nJust send me a song name, a Deezer link, or a Spotify link and I'll find it and queue it for download on your server. No technical stuff needed!\n\n📲 Use /menu to see quick action buttons.\n\nFor a full list of what I can do, type /help.",
            )
            .reply_markup(kb)
            .await?;
        }

        Command::Help => {
            bot.send_message(
                msg.chat.id,
                "ℹ️ What I do?\n\n\
I connect to your personal deemix server and queue music downloads for you. Just tell me what you want!\n\n\
📥 Ways to request music:\n\
• Type any song or artist name → search and pick from results\n\
• Send a Deezer link (track, album, playlist) → queued instantly\n\
• Send a Spotify, YouTube, YouTube Music, or Apple Music link → found on Deezer and queued\n\
• Send a Spotify or YouTube playlist link → every track is scanned and queued individually\n\
• Send a voice note → transcribe what you said or recognize the song\n\n\
🔧 All commands:\n\
/menu — quick action buttons\n\
/search — search for a track\n\
/album — search for an album\n\
/dl — queue from a Deezer URL\n\
/sp — queue from a streaming link (Spotify, YouTube, Apple Music)\n\
/status — check deemix connection and queue\n\
/clearqueue — clear completed downloads from queue\n\
/settings — manage your personal preferences\n\
/updatearl — update your Deezer ARL\n\n\
⚙️ Settings (via /settings):\n\
• Restart notifications — get notified when the bot restarts\n\
• Voice search — transcribe voice notes to search\n\
• Song recognition — identify songs from voice recordings\n\n\
💡 Tip: You don't need commands — just send a song name or link directly!",
            )
            .await?;
        }

        Command::Status => {
            match deemix::get_queue(&state).await {
                Ok(q) => {
                    let mut text = "✅ Deemix is reachable\n".to_string();
                    if q.downloading > 0 { text.push_str(&format!("⬇️ Downloading: {}\n", q.downloading)); }
                    if q.pending > 0 { text.push_str(&format!("⏳ Pending: {}\n", q.pending)); }
                    if q.failed > 0 { text.push_str(&format!("❌ Failed: {}\n", q.failed)); }
                    if q.done > 0 { text.push_str(&format!("✅ Completed (in queue): {}", q.done)); }
                    if q.downloading == 0 && q.pending == 0 && q.done == 0 && q.failed == 0 { text.push_str("📭 Queue is empty"); }
                    bot.send_message(msg.chat.id, text).await?;
                }
                Err(e) => { bot.send_message(msg.chat.id, format!("❌ Can't reach deemix: {}", e)).await?; }
            }
        }

        Command::Dl => {
            dialogue.update(State::AwaitingDl).await
                .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
            bot.send_message(msg.chat.id, "🎵 Send me a Deezer URL:").await?;
        }

        Command::Search => {
            dialogue.update(State::AwaitingSearch).await
                .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
            bot.send_message(msg.chat.id, "🔍 What song or artist are you looking for?").await?;
        }

        Command::Album => {
            dialogue.update(State::AwaitingAlbum).await
                .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
            bot.send_message(msg.chat.id, "💿 What album are you looking for?").await?;
        }

        Command::Sp => {
            dialogue.update(State::AwaitingSpotify).await
                .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
            bot.send_message(msg.chat.id, "🔗 Send me a link from Spotify, YouTube, YouTube Music, or Apple Music:").await?;
        }

        Command::Clearqueue => {
            match deemix::clear_completed(&state).await {
                Ok(0) => { bot.send_message(msg.chat.id, "📭 No completed downloads to clear.").await?; }
                Ok(n) => { bot.send_message(msg.chat.id, format!("🧹 Cleared {} completed download(s) from queue.", n)).await?; }
                Err(e) => { bot.send_message(msg.chat.id, format!("❌ Failed to clear queue: {}", e)).await?; }
            }
        }

        Command::Menu => {
            let kb = main_keyboard(&user_settings, &state.config);
            bot.send_message(msg.chat.id, "Choose an action:").reply_markup(kb).await?;
        }

        Command::Settings => {
            let current_br = *state.current_bitrate.lock().await;
    let kb = settings_keyboard(&user_settings, &state.config, current_br);
            bot.send_message(msg.chat.id, "⚙️ Your settings — tap to toggle:").reply_markup(kb).await?;
        }

        Command::Updatearl => {
            dialogue.update(State::AwaitingArl).await
                .map_err(|e| teloxide::RequestError::Api(teloxide::ApiError::Unknown(e.to_string())))?;
            bot.send_message(msg.chat.id, "Please send your new Deezer ARL:")
                .reply_markup(arl_cancel_keyboard())
                .await?;
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

async fn receive_dl(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    dialogue.exit().await.ok();
    if let Some(url) = msg.text() { queue_url(&bot, &msg, &state, url.trim()).await?; }
    Ok(())
}

async fn receive_spotify(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    dialogue.exit().await.ok();
    if let Some(url) = msg.text() { handle_streaming_link(&bot, &msg, &state, url.trim()).await?; }
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
    
    // 1. Update the bot's memory
    let new_bitrate = {
        let mut br = state.current_bitrate.lock().await;
        *br = next_bitrate(*br);
        *br
    };
    
    // 2. Write to config.json
    let config_json_path = "/config/config.json";
    if let Ok(contents) = std::fs::read_to_string(config_json_path) {
        if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&contents) {
            config["maxBitrate"] = serde_json::json!(new_bitrate);
            if let Ok(updated_json) = serde_json::to_string_pretty(&config) {
                let _ = std::fs::write(config_json_path, updated_json);
            }
        }
    }
    
    // 3. Update ENV
    std::env::set_var("DEEMIX_BITRATE", new_bitrate.to_string());
    
    // 4. Send a message about the process (with updated keyboard!)
    let updated = users::get_or_create(&state.users, user_id_from_msg(&msg));
    let kb = settings_keyboard(&updated, &state.config, new_bitrate);
    bot.send_message(msg.chat.id, "⏳ Applying quality setting... please wait.")
        .reply_markup(kb)
        .await?;

    // 5. Write flag file for post-restart notification
    let flag_path = "/config/telegram_bot/pending_quality_change.json";
    let flag_data = serde_json::json!({ "bitrate": new_bitrate });
    let _ = std::fs::write(flag_path, serde_json::to_string(&flag_data).unwrap());
    
    // 6. Updating HA options via the Supervisor API
    match supervisor::update_ha_option("deemix_bitrate", serde_json::json!(new_bitrate)).await {
        Ok(_) => {
            log::info!("[ha-sync] Bitrate synced to HA options");
            
            // 7. Restart Add-on
            let _ = supervisor::restart_addon().await;
        }
        Err(e) => {
            log::warn!("[ha-sync] Failed to sync bitrate to HA: {}", e);
            // Remove flag file — restart won't happen, notification not needed
            let _ = std::fs::remove_file("/config/telegram_bot/pending_quality_change.json");
        }
    }
    Ok(())
}

// ── Message Handler ───────────────────────────────────────────────────────────
async fn handle_message(bot: Bot, msg: Message, state: Arc<BotState>, dialogue: MyDialogue) -> ResponseResult<()> {
    // Auto-create user
    let user_settings = users::get_or_create(&state.users, user_id_from_msg(&msg));
    users::save(&state.users, &state.config.users_file);

    // ── Voice note handling ──
    if msg.voice().is_some() {
        return voice::handle_voice_message(bot, msg, state, dialogue).await;
    }

    let text = match msg.text() {
        Some(t) => t.trim().to_string(),
        None => return Ok(()),
    };

    // ── Keyboard button presses ──
    match text.as_str() {
        "🔍 Search a track" => {
            dialogue.update(State::AwaitingSearch).await.ok();
            bot.send_message(msg.chat.id, "🔍 What song or artist are you looking for?").await?;
            return Ok(());
        }
        "💿 Search an album" => {
            dialogue.update(State::AwaitingAlbum).await.ok();
            bot.send_message(msg.chat.id, "💿 What album are you looking for?").await?;
            return Ok(());
        }
        "🔗 From streaming link" => {
            dialogue.update(State::AwaitingSpotify).await.ok();
            bot.send_message(msg.chat.id, "🔗 Send me a link from Spotify, YouTube, YouTube Music, or Apple Music:").await?;
            return Ok(());
        }
        "🎵 From Deezer URL" => {
            dialogue.update(State::AwaitingDl).await.ok();
            bot.send_message(msg.chat.id, "🎵 Send me a Deezer URL:").await?;
            return Ok(());
        }
        "🎤 Voice search" => {
            if !state.config.whisper_enabled() || !user_settings.voice_search {
                bot.send_message(msg.chat.id, "⚠️ Voice search is not configured. Add OPENAI_API_KEY or WHISPER_URL to your .env, or enable it in /settings.").await?;
            } else {
                dialogue.update(State::AwaitingVoiceTranscribe).await.ok();
                bot.send_message(msg.chat.id, "🎤 Send me a voice note and I'll transcribe what you said and search for it.

⏱ You have 60 seconds.").await?;
                // Spawn timeout to reset dialogue after 60s
                let dialogue_clone = dialogue.clone();
                let chat_id = msg.chat.id;
                let bot_clone = bot.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    if let Ok(Some(State::AwaitingVoiceTranscribe)) = dialogue_clone.get().await {
                        dialogue_clone.exit().await.ok();
                        let _ = bot_clone.send_message(chat_id, "⏱ Voice search timed out. Send a voice note or use /menu to start again.").await;
                    }
                });
            }
            return Ok(());
        }
        "🎵 Recognize song" => {
            if !state.config.audd_enabled() || !user_settings.song_recognition {
                bot.send_message(msg.chat.id, "⚠️ Song recognition is not configured. Add AUDD_API_KEY to your .env, or enable it in /settings.").await?;
            } else {
                dialogue.update(State::AwaitingVoiceRecognize).await.ok();
                bot.send_message(msg.chat.id, "🎵 Send me a voice recording of a song and I'll identify it.

⏱ You have 60 seconds.").await?;
                // Spawn timeout to reset dialogue after 60s
                let dialogue_clone = dialogue.clone();
                let chat_id = msg.chat.id;
                let bot_clone = bot.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    if let Ok(Some(State::AwaitingVoiceRecognize)) = dialogue_clone.get().await {
                        dialogue_clone.exit().await.ok();
                        let _ = bot_clone.send_message(chat_id, "⏱ Song recognition timed out. Send a voice note or use /menu to start again.").await;
                    }
                });
            }
            return Ok(());
        }
        "📊 Check status" => {
            match deemix::get_queue(&state).await {
                Ok(q) => {
                    let mut t = "✅ Deemix is reachable\n".to_string();
                    if q.downloading > 0 { t.push_str(&format!("⬇️ Downloading: {}\n", q.downloading)); }
                    if q.pending > 0 { t.push_str(&format!("⏳ Pending: {}\n", q.pending)); }
                    if q.failed > 0 { t.push_str(&format!("❌ Failed: {}\n", q.failed)); }
                    if q.done > 0 { t.push_str(&format!("✅ Completed: {}", q.done)); }
                    if q.downloading == 0 && q.pending == 0 && q.done == 0 && q.failed == 0 { t.push_str("📭 Queue is empty"); }
                    bot.send_message(msg.chat.id, t).await?;
                }
                Err(e) => { bot.send_message(msg.chat.id, format!("❌ Can't reach deemix: {}", e)).await?; }
            }
            return Ok(());
        }
        "🧹 Clear queue" => {
            match deemix::clear_completed(&state).await {
                Ok(0) => { bot.send_message(msg.chat.id, "📭 No completed downloads to clear.").await?; }
                Ok(n) => { bot.send_message(msg.chat.id, format!("🧹 Cleared {} completed download(s) from queue.", n)).await?; }
                Err(e) => { bot.send_message(msg.chat.id, format!("❌ Failed to clear queue: {}", e)).await?; }
            }
            return Ok(());
        }
        "⚙️ Settings" => {
            let current_br = *state.current_bitrate.lock().await;
    let kb = settings_keyboard(&user_settings, &state.config, current_br);
            bot.send_message(msg.chat.id, "⚙️ Your settings — tap to toggle:").reply_markup(kb).await?;
            return Ok(());
        }
        "🔙 Back to menu" => {
            let kb = main_keyboard(&user_settings, &state.config);
            bot.send_message(msg.chat.id, "Choose an action:").reply_markup(kb).await?;
            return Ok(());
        }
        "🔑 Update ARL" => {
            dialogue.update(State::AwaitingArl).await.ok();
            bot.send_message(msg.chat.id, "Please send your new Deezer ARL:")
                .reply_markup(arl_cancel_keyboard())
                .await?;
            return Ok(());
        }
        "ℹ️ Help" => {
            bot.send_message(msg.chat.id,
                "ℹ️ What I do\n\n\
I connect to your personal deemix server and queue music downloads. Just tell me what you want!\n\n\
📥 Ways to request music:\n\
• Type any song or artist name → search and pick\n\
• Send a Deezer link → queued instantly\n\
• Send a Spotify, YouTube, YouTube Music, or Apple Music link → found on Deezer and queued\n\
• Send a Spotify or YouTube playlist link → every track is scanned and queued individually\n\
• Send a voice note → transcribe or recognize\n\n\
🔧 Commands:\n\
/menu — quick action buttons\n\
/search — search for a track\n\
/album — search for an album\n\
/dl — queue from a Deezer URL\n\
/sp — queue from a streaming link (Spotify, YouTube, Apple Music)\n\
/status — check deemix\n\
/clearqueue — clear completed downloads from queue\n\
/settings — your personal settings\n\
/updatearl — update your Deezer ARL\n\n\
💡 Tip: Just send a song name or link — no commands needed!"
            ).await?;
            return Ok(());
        }
        // Settings toggles
        t if t.starts_with("🔔 Restart notifications:") || t.starts_with("🔕 Restart notifications:") => {
            users::update(&state.users, &state.config.users_file, user_id_from_msg(&msg), |s| {
                s.restart_notifications = !s.restart_notifications;
            });
            let updated = users::get_or_create(&state.users, user_id_from_msg(&msg));
            let current_br = *state.current_bitrate.lock().await;
    let kb = settings_keyboard(&updated, &state.config, current_br);
            let status = if updated.restart_notifications { "ON" } else { "OFF" };
            bot.send_message(msg.chat.id, format!("🔔 Restart notifications: {}", status)).reply_markup(kb).await?;
            return Ok(());
        }
        t if t.starts_with("🎤 Voice search:") => {
            if !state.config.whisper_enabled() {
                bot.send_message(msg.chat.id, "⚠️ Voice search is not configured. Add OPENAI_API_KEY to your .env to enable it.").await?;
                return Ok(());
            }
            users::update(&state.users, &state.config.users_file, user_id_from_msg(&msg), |s| {
                s.voice_search = !s.voice_search;
            });
            let updated = users::get_or_create(&state.users, user_id_from_msg(&msg));
            let current_br = *state.current_bitrate.lock().await;
    let kb = settings_keyboard(&updated, &state.config, current_br);
            let status = if updated.voice_search { "ON" } else { "OFF" };
            bot.send_message(msg.chat.id, format!("🎤 Voice search: {}", status)).reply_markup(kb).await?;
            return Ok(());
        }
        t if t.starts_with("🎚️ Quality:") => {
            handle_quality_change(&bot, &msg, &state).await?;
            return Ok(());
        }
        t if t.starts_with("🔒 Quality:") => {
            bot.send_message(msg.chat.id, "🔒 Download quality is locked by the administrator.").await?;
            return Ok(());
        }
        t if t.starts_with("🎵 Song recognition:") => {
            if !state.config.audd_enabled() {
                bot.send_message(msg.chat.id, "⚠️ Song recognition is not configured. Add AUDD_API_KEY to your .env to enable it.").await?;
                return Ok(());
            }
            users::update(&state.users, &state.config.users_file, user_id_from_msg(&msg), |s| {
                s.song_recognition = !s.song_recognition;
            });
            let updated = users::get_or_create(&state.users, user_id_from_msg(&msg));
            let current_br = *state.current_bitrate.lock().await;
    let kb = settings_keyboard(&updated, &state.config, current_br);
            let status = if updated.song_recognition { "ON" } else { "OFF" };
            bot.send_message(msg.chat.id, format!("🎵 Song recognition: {}", status)).reply_markup(kb).await?;
            return Ok(());
        }
        _ => {}
    }

    // ── Streaming service URLs ──
    if SPOTIFY_TRACK_RE.is_match(&text) || SPOTIFY_ALBUM_RE.is_match(&text)
        || SPOTIFY_PLAYLIST_RE.is_match(&text) || YOUTUBE_RE.is_match(&text)
        || APPLE_MUSIC_RE.is_match(&text)
    {
        handle_streaming_link(&bot, &msg, &state, &text).await?;
        return Ok(());
    }

    // ── Deezer short URL ──
    if DEEZER_SHORT_RE.is_match(&text) {
        let resolved = resolve_short_link(&state.http, &text).await;
        if let Some(url) = resolved {
            queue_url(&bot, &msg, &state, &url).await?;
        } else {
            bot.send_message(msg.chat.id, "❌ Could not resolve that link.").await?;
        }
        return Ok(());
    }

    // ── Full Deezer URL ──
    if DEEZER_URL_RE.is_match(&text) {
        queue_url(&bot, &msg, &state, &text).await?;
        return Ok(());
    }

    // ── Plain text search ──
    do_search(&bot, &msg, &state, &text, "track").await?;
    Ok(())
}

// ── Callback Handler ──────────────────────────────────────────────────────────
async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    storage: Arc<InMemStorage<State>>,
) -> ResponseResult<()> {
    bot.answer_callback_query(&q.id).await?;

    let data = match &q.data {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    if data == "cancel" {
        if let Some(msg) = &q.message {
            bot.edit_message_text(msg.chat.id, msg.id, "Cancelled.").await?;
        }
        return Ok(());
    }

    if data == "cancel_arl" {
        if let Some(msg) = &q.message {
            let dialogue: MyDialogue = Dialogue::new(storage, msg.chat.id);
            dialogue.exit().await.ok();
            bot.edit_message_text(msg.chat.id, msg.id, "❌ ARL update cancelled.").await?;
        }
        return Ok(());
    }

    if let Some(url) = data.strip_prefix("dl:") {
        if let Some(msg) = &q.message {
            let kind = DEEZER_URL_RE.captures(url)
                .and_then(|c| c.get(1))
                .map(|m| capitalize(m.as_str()))
                .unwrap_or_else(|| "Item".to_string());
            bot.edit_message_text(msg.chat.id, msg.id, format!("⏳ Queuing {}...", kind.to_lowercase())).await?;
            match deemix::add_to_queue(&state, url).await {
                Ok(_) => { bot.edit_message_text(msg.chat.id, msg.id, format!("✅ {} added to queue!", kind)).await?; }
                Err(e) => { bot.edit_message_text(msg.chat.id, msg.id, format!("❌ Failed: {}", e)).await?; }
            }
        }
    }

    // ── Voice callbacks ──
    if data.starts_with("vt:") || data.starts_with("vr:") {
        voice::handle_voice_callback(&bot, &q, &data, &state).await?;
    }
    Ok(())
}

// ── Core Helpers ──────────────────────────────────────────────────────────────
async fn resolve_short_link(http: &Client, url: &str) -> Option<String> {
    let resp = http.head(url).send().await.ok()?;
    Some(resp.url().to_string())
}

async fn queue_url(bot: &Bot, msg: &Message, state: &Arc<BotState>, url: &str) -> ResponseResult<()> {
    let cap = match DEEZER_URL_RE.captures(url) {
        Some(c) => c,
        None => {
            bot.send_message(msg.chat.id, "❌ That doesn't look like a valid Deezer URL.").await?;
            return Ok(());
        }
    };
    let kind = cap.get(1).map(|m| m.as_str()).unwrap_or("item");
    let sent = bot.send_message(msg.chat.id, format!("⏳ Queuing {}...", kind)).await?;

    match deemix::add_to_queue(state, url).await {
        Ok(_) => { bot.edit_message_text(msg.chat.id, sent.id, format!("✅ {} added to queue!", capitalize(kind))).await?; }
        Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Failed to queue: {}", e)).await?; }
    }
    Ok(())
}

/// Telegram truncates long button labels, so the message text carries the full
/// numbered track list and the buttons reference the numbers.
pub(crate) fn build_search_results(results: &[serde_json::Value], icon: &str) -> (String, Vec<Vec<InlineKeyboardButton>>) {
    let mut listing = String::new();
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (i, item) in results.iter().enumerate() {
        let title = item["title"].as_str().unwrap_or("?");
        let artist = item["artist"]["name"].as_str().unwrap_or("?");
        listing.push_str(&format!("{}. {} — {}\n", i + 1, title, artist));
        let label = format!("{} {}. {} — {}", icon, i + 1, title, artist);
        let label = if label.chars().count() > 60 { format!("{}…", label.chars().take(59).collect::<String>()) } else { label };
        buttons.push(vec![InlineKeyboardButton::callback(label, format!("dl:{}", item["link"].as_str().unwrap_or("")))]);
    }
    (listing, buttons)
}

async fn do_search(bot: &Bot, msg: &Message, state: &Arc<BotState>, query: &str, search_type: &str) -> ResponseResult<()> {
    let sent = bot.send_message(msg.chat.id, format!("🔍 Searching for {}...", query)).await?;

    match deemix::search(state, query, search_type).await {
        Ok(results) if results.is_empty() => { bot.edit_message_text(msg.chat.id, sent.id, "😕 No results found.").await?; }
        Ok(results) => {
            let icon = if search_type == "track" { "🎵" } else { "💿" };
            let (listing, mut buttons) = build_search_results(&results, icon);
            buttons.push(vec![InlineKeyboardButton::callback("❌ Cancel", "cancel")]);
            bot.edit_message_text(msg.chat.id, sent.id, format!("Results for {}:\n\n{}\nTap a button to download.", query, listing))
                .reply_markup(InlineKeyboardMarkup::new(buttons))
                .await?;
        }
        Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ Search failed: {}", e)).await?; }
    }
    Ok(())
}

async fn handle_streaming_link(bot: &Bot, msg: &Message, state: &Arc<BotState>, url: &str) -> ResponseResult<()> {
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

async fn handle_updatearl(bot: &Bot, msg: &Message, state: &Arc<BotState>, arl: &str) -> ResponseResult<()> {
    let sent = bot.send_message(msg.chat.id, "🔄 Validating new ARL...").await?;

    match deemix::login_arl(state, arl).await {
        Ok(_username) => {
            *state.current_arl.lock().await = arl.to_string();
            std::env::set_var("DEEMIX_ARL", arl);
            
            // Synchronization with HA via the Supervisor API
            match supervisor::update_ha_option("deemix_arl", serde_json::json!(arl)).await {
                Ok(_) => {
                    log::info!("[ha-sync] ARL synced to HA options");
                }
                Err(e) => {
                    log::warn!("[ha-sync] Failed to sync ARL to HA: {}", e);
                }
            }
            
            bot.edit_message_text(msg.chat.id, sent.id, "✅ ARL updated and logged in! Downloads will use the new ARL immediately.").await?;
        }
        Err(e) => { bot.edit_message_text(msg.chat.id, sent.id, format!("❌ ARL rejected by deemix: {}", e)).await?; }
    }
    Ok(())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
