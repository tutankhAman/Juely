//! Cross-window shared preferences — reconstructed from Pluely 1.1.0.
//!
//! Upstream stores app-level settings in the `app_settings` SQLite table and
//! broadcasts changes to all windows over the `pluely:shared-prefs` event with
//! a `ChangePayload { key, value }`. The recovered frontend calls
//! `shared_prefs_broadcast { key, value }` and listens on that event.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

/// Event name observed in the binary (`pluely:shared-prefs`).
pub const EVENT: &str = "pluely:shared-prefs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePayload {
    pub key: String,
    pub value: String,
}

#[derive(Default)]
pub struct SharedPrefs {
    pub cache: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

/// `shared_prefs_broadcast { key, value }`
///
/// Updates the in-memory cache and emits the change to every window. The
/// SQLite write is done by the frontend via `plugin:sql|execute` in the
/// recovered design; this command only synchronizes windows.
pub fn shared_prefs_broadcast(app: &AppHandle, args: &Value) -> Value {
    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let value = args
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    if let Ok(mut c) = app.state::<SharedPrefs>().cache.lock() {
        c.insert(key.clone(), value.clone());
    }

    let payload = ChangePayload { key, value };
    let _ = app.emit(EVENT, payload);
    Value::Null
}

/// Read the in-memory cache (helper for other modules).
pub fn get(app: &AppHandle, key: &str) -> Option<String> {
    app.state::<SharedPrefs>()
        .cache
        .lock()
        .ok()
        .and_then(|c| c.get(key).cloned())
}