//! Telegram reply/inline keyboard builders and bitrate helpers.
//!
//! The settings keyboard reflects the user's settings and the addon
//! configuration; the main menu shows the song recognition button when
//! ACRCloud is configured.

use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup,
};

use crate::config::Config;
use crate::users::UserSettings;

// Reply-keyboard labels — the single source for the builders below and the
// match arms in main.rs: Telegram echoes the exact button text back as the
// message text, so both sides must use the same bytes. Toggle rows get an
// " ON"/" OFF" suffix appended at build time.
pub(crate) const BTN_SEARCH_TRACK: &str = "🔍 Search a track";
pub(crate) const BTN_SEARCH_ALBUM: &str = "💿 Search an album";
pub(crate) const BTN_RECOGNIZE_SONG: &str = "🎵 Recognize song";
pub(crate) const BTN_CHECK_STATUS: &str = "📊 Check status";
pub(crate) const BTN_CLEAR_QUEUE: &str = "🧹 Clear queue";
pub(crate) const BTN_SETTINGS: &str = "⚙️ Settings";
pub(crate) const BTN_HELP: &str = "ℹ️ Help";
pub(crate) const BTN_BACK_MENU: &str = "🔙 Back to menu";
pub(crate) const BTN_UPDATE_ARL: &str = "🔑 Update ARL";
pub(crate) const BTN_RESTART_NOTIF_ON: &str = "🔔 Restart notifications:";
pub(crate) const BTN_RESTART_NOTIF_OFF: &str = "🔕 Restart notifications:";
pub(crate) const BTN_QUALITY: &str = "🎚️ Quality:";
pub(crate) const BTN_QUALITY_LOCKED: &str = "🔒 Quality:";

/// Label for a deemix bitrate value (9 = FLAC, 3/1 = MP3).
pub(crate) fn bitrate_label(bitrate: u8) -> &'static str {
    match bitrate {
        9 => "FLAC (lossless)",
        3 => "MP3 320kbps",
        1 => "MP3 128kbps",
        _ => "Unknown",
    }
}

/// Cycle through the supported bitrates: FLAC → MP3 320 → MP3 128 → FLAC.
pub(crate) fn next_bitrate(current: u8) -> u8 {
    match current { 9 => 3, 3 => 1, _ => 9 }
}

pub(crate) fn settings_keyboard(s: &UserSettings, config: &Config, bitrate: u8) -> KeyboardMarkup {
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

pub(crate) fn arl_cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback("❌ Cancel", "cancel_arl")]])
}

pub(crate) fn main_keyboard(config: &Config) -> KeyboardMarkup {
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
