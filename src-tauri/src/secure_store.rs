//! Secure local storage — reconstructed from Pluely 1.1.0.
//!
//! Upstream keeps an encrypted vault (`pluely.secure_store.v1` / `vault.dat`)
//! and migrates a legacy plaintext `secure_storage.json`. Recovered commands:
//! `secure_storage_save { items:[{key,value}] }`, `secure_storage_get`,
//! `secure_storage_remove { keys:[..] }`, plus `byok_store_set { value }` and
//! `byok_store_get` (BYOK provider keys, stored as a JSON string).
//!
//! Encryption detail was not recovered byte-for-byte; this reconstruction uses
//! a machine-derived keystream for at-rest obfuscation and documents that as a
//! design choice rather than an observed fact.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageItem {
    pub key: String,
    pub value: String,
}

#[derive(Default, Serialize, Deserialize)]
struct SecureData {
    #[serde(default)]
    items: BTreeMap<String, String>,
}

fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("vault.dat"))
}

fn keystream(seed: &str, len: usize) -> Vec<u8> {
    // FNV-1a expansion; deterministic per machine + vault tag.
    let mut out = Vec::with_capacity(len);
    let mut state: u64 = 0xcbf29ce484222325;
    for b in seed.bytes() {
        state ^= b as u64;
        state = state.wrapping_mul(0x100000001b3);
    }
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn xor(data: &[u8], seed: &str) -> Vec<u8> {
    let ks = keystream(seed, data.len());
    data.iter().zip(ks).map(|(a, b)| a ^ b).collect()
}

fn load(app: &AppHandle, machine: &str) -> SecureData {
    let Ok(path) = vault_path(app) else {
        return SecureData::default();
    };
    let Ok(raw) = fs::read(&path) else {
        return SecureData::default();
    };
    // Try the current machine id, then stable fallbacks (env-dependent keys).
    for seed_machine in [machine, "phase1"] {
        let decoded = xor(&raw, &format!("pluely.secure_store.v1:{seed_machine}"));
        if let Ok(data) = serde_json::from_slice::<SecureData>(&decoded) {
            return data;
        }
    }
    SecureData::default()
}

fn save(app: &AppHandle, machine: &str, data: &SecureData) -> Result<(), String> {
    let path = vault_path(app)?;
    let bytes = serde_json::to_vec(data).map_err(|e| e.to_string())?;
    let enc = xor(&bytes, &format!("pluely.secure_store.v1:{machine}"));
    fs::write(path, enc).map_err(|e| e.to_string())
}

fn machine_of(app: &AppHandle) -> String {
    // Prefer an explicitly recorded id, else the host name (same rule as
    // session::get_machine_uid), else a stable marker.
    if let Some(id) = app
        .try_state::<MachineId>()
        .and_then(|s| s.0.lock().ok().map(|g| g.clone()))
        .filter(|id| !id.is_empty())
    {
        return id;
    }
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "phase1".to_string())
}

#[derive(Default)]
pub struct MachineId(pub std::sync::Mutex<String>);

/// `secure_storage_save { items }`
pub fn secure_storage_save(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let items: Vec<StorageItem> = args
        .get("items")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let machine = machine_of(app);
    let mut data = load(app, &machine);
    for it in items {
        data.items.insert(it.key, it.value);
    }
    save(app, &machine, &data)?;
    Ok(Value::Null)
}

/// `secure_storage_get` — returns an object keyed by stored item keys.
pub fn secure_storage_get(app: &AppHandle) -> Value {
    let machine = machine_of(app);
    let data = load(app, &machine);
    serde_json::to_value(data.items).unwrap_or_else(|_| json!({}))
}

/// `secure_storage_remove { keys }`
pub fn secure_storage_remove(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let keys: Vec<String> = args
        .get("keys")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let machine = machine_of(app);
    let mut data = load(app, &machine);
    for k in keys {
        data.items.remove(&k);
    }
    save(app, &machine, &data)?;
    Ok(Value::Null)
}

const BYOK_KEY: &str = "byok_providers";

/// `byok_store_set { value }` — value is a JSON string.
pub fn byok_store_set(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let value = args
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("{}")
        .to_string();
    let machine = machine_of(app);
    let mut data = load(app, &machine);
    data.items.insert(BYOK_KEY.to_string(), value);
    save(app, &machine, &data)?;
    // Cross-window sync: the overlay refreshes providers on this event.
    let _ = app.emit("byok-changed", ());
    Ok(Value::Null)
}

/// `byok_store_get` — returns the stored JSON string (or null).
pub fn byok_store_get(app: &AppHandle) -> Value {
    let machine = machine_of(app);
    let data = load(app, &machine);
    match data.items.get(BYOK_KEY) {
        Some(v) => Value::String(v.clone()),
        None => Value::Null,
    }
}