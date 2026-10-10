//! Stealth keys — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `stealth_keys::{enable_stealth_keys, update_stealth_keys,
//! StealthShortcut}`. `StealthShortcut` has 6 fields (see `.rodata` struct
//! table). Stealth keys are extra global shortcuts that never appear in the UI
//! list and can be toggled off for deniability.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthShortcut {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Default)]
pub struct StealthState(pub Mutex<Vec<StealthShortcut>>);

/// `enable_stealth_keys`
pub fn enable_stealth_keys(app: &AppHandle) -> Value {
    let state = app.state::<StealthState>();
    let list = state.0.lock().map(|g| g.clone()).unwrap_or_default();
    json!({ "enabled": true, "count": list.len() })
}

/// `disable_stealth_keys`
pub fn disable_stealth_keys(app: &AppHandle) -> Value {
    let state = app.state::<StealthState>();
    let count = state.0.lock().map(|g| g.len()).unwrap_or(0);
    json!({ "enabled": false, "count": count })
}

/// `update_stealth_keys { shortcuts }`
pub fn update_stealth_keys(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let shortcuts: Vec<StealthShortcut> = args
        .get("shortcuts")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let count = shortcuts.len();
    if let Ok(mut g) = app.state::<StealthState>().0.lock() {
        *g = shortcuts;
    }
    Ok(json!({ "updated": count }))
}