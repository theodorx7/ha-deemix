use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(default)]
    pub restart_notifications: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            restart_notifications: false, // off by default
        }
    }
}

pub type UsersDb = Arc<RwLock<HashMap<String, UserSettings>>>;

pub fn load(path: &str) -> UsersDb {
    let map = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Arc::new(RwLock::new(map))
}

fn save(db: &UsersDb, path: &str) {
    if let Ok(map) = db.read() {
        if let Ok(json) = serde_json::to_string_pretty(&*map) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub fn get_or_create(db: &UsersDb, path: &str, user_id: i64) -> UserSettings {
    let key = user_id.to_string();
    if let Ok(map) = db.read() {
        if let Some(settings) = map.get(&key) {
            return settings.clone();
        }
    }
    // Auto-create with defaults; persist only on actual creation.
    let settings = UserSettings::default();
    let mut created = false;
    if let Ok(mut map) = db.write() {
        created = map.insert(key, settings.clone()).is_none();
    }
    if created {
        save(db, path);
    }
    settings
}

pub fn update<F>(db: &UsersDb, path: &str, user_id: i64, f: F)
where
    F: FnOnce(&mut UserSettings),
{
    let key = user_id.to_string();
    if let Ok(mut map) = db.write() {
        let settings = map.entry(key).or_default();
        f(settings);
    }
    save(db, path);
}

pub fn all_with_notifications(db: &UsersDb) -> Vec<i64> {
    if let Ok(map) = db.read() {
        map.iter()
            .filter(|(_, s)| s.restart_notifications)
            .filter_map(|(k, _)| k.parse::<i64>().ok())
            .collect()
    } else {
        vec![]
    }
}
