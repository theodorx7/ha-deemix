//! Telegram reply/inline keyboard builders and bitrate helpers.
//!
//! The settings keyboard reflects the user's settings and the addon
//! configuration; the main menu is static.

use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup,
};

use crate::config::Config;
use crate::users::UserSettings;

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
    let notif = if s.restart_notifications { "🔔 Restart notifications: ON" } else { "🔕 Restart notifications: OFF" };
    let recog = if s.song_recognition && config.acrcloud_enabled() { "🎵 Song recognition: ON" } else { "🎵 Song recognition: OFF" };
    let bitrate_btn = if config.deemix_bitrate_lock {
        format!("🔒 Quality: {} (locked)", bitrate_label(bitrate))
    } else {
        format!("🎚️ Quality: {} (tap to change)", bitrate_label(bitrate))
    };

    KeyboardMarkup::new(vec![
        vec![KeyboardButton::new(notif)],
        vec![KeyboardButton::new(recog)],
        vec![KeyboardButton::new(bitrate_btn)],
        vec![KeyboardButton::new("🔑 Update ARL")],
        vec![KeyboardButton::new("🔙 Back to menu")],
    ])
    .resize_keyboard()
}

pub(crate) fn arl_cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback("❌ Cancel", "cancel_arl")]])
}

pub(crate) fn main_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new("🔍 Search a track"),
            KeyboardButton::new("💿 Search an album"),
        ],
        vec![
            KeyboardButton::new("📊 Check status"),
            KeyboardButton::new("🧹 Clear queue"),
        ],
        vec![
            KeyboardButton::new("⚙️ Settings"),
            KeyboardButton::new("ℹ️ Help"),
        ],
    ])
    .resize_keyboard()
}
