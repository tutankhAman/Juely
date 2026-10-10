//! License activation — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `activate::{activate_license, activate_license_api,
//! deactivate_license, deactivate_license_api, validate_license_api,
//! mask_license_key_cmd, secure_storage_save/get/remove, ActivationRequest,
//! ActivationResponse, ValidateResponse, VALIDATE_CACHE, StorageItem,
//! StorageResult}`. Targets `api.pluely.com` + `/payments/v1/licenses`.
//! This reconstruction preserves the surface and response shapes; the remote
//! license service is not reachable without credentials.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;

use crate::secure_store;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidateResponse {
    pub is_active: bool,
    #[serde(default)]
    pub is_dev_license: bool,
    #[serde(default)]
    pub last_validated_at: Option<String>,
}

#[derive(Default)]
pub struct ValidateCache(pub Mutex<Option<ValidateResponse>>);

fn key_fingerprint(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }
    let head = &key[..4];
    let tail = &key[key.len() - 4..];
    format!("{head}{}{tail}", "*".repeat(key.len() - 8))
}

/// `activate_license_api { licenseKey }`
///
/// Local dev licenses (`LOCAL-…`, self-issued for this rebuild) are accepted
/// offline and persisted to the vault. Anything else requires the real
/// license service and is reported as not configured.
pub fn activate_license_api(app: &tauri::AppHandle, args: &Value) -> Value {
    use tauri::Manager;
    let key = args
        .get("licenseKey")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if key.is_empty() {
        return json!({ "activated": false, "error": "License key not found" });
    }
    if key.starts_with("LOCAL-") {
        let machine = app
            .state::<crate::session::SessionState>()
            .machine_id
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| "phase1".to_string());
        let instance_id = format!("{machine}-local-dev");
        let _ = secure_store::secure_storage_save(
            app,
            &json!({ "items": [
                { "key": "pluely_license_key", "value": key },
                { "key": "pluely_instance_id", "value": instance_id }
            ]}),
        );
        app.state::<crate::shortcuts::LicenseState>().set(true);
        if let Ok(mut c) = app.state::<ValidateCache>().0.lock() {
            *c = Some(ValidateResponse {
                is_active: true,
                is_dev_license: true,
                last_validated_at: Some("local".to_string()),
            });
        }
        let _ = app.emit("license-changed", true);
        return json!({ "activated": true, "instance_id": instance_id });
    }
    json!({
        "activated": false,
        "instance_id": null,
        "error": "Activation service not configured"
    })
}

/// `deactivate_license_api`
pub fn deactivate_license_api() -> Value {
    json!({ "deactivated": true, "configured": false })
}

/// `validate_license_api` — honors a cached result, then a LOCAL- vault key.
pub fn validate_license_api(app: &tauri::AppHandle) -> Value {
    use tauri::Manager;
    if let Some(cached) = app
        .state::<ValidateCache>()
        .0
        .lock()
        .ok()
        .and_then(|g| g.clone())
    {
        return serde_json::to_value(cached).unwrap_or(Value::Null);
    }
    // Local dev license present in the vault?
    let vault = secure_store::secure_storage_get(app);
    let local = vault
        .get("pluely_license_key")
        .and_then(|v| v.as_str())
        .map(|k| k.starts_with("LOCAL-"))
        .unwrap_or(false);
    let resp = ValidateResponse {
        is_active: local,
        is_dev_license: local,
        last_validated_at: local.then(|| "local".to_string()),
    };
    if local {
        app.state::<crate::shortcuts::LicenseState>().set(true);
        if let Ok(mut c) = app.state::<ValidateCache>().0.lock() {
            *c = Some(resp.clone());
        }
    }
    serde_json::to_value(resp).unwrap_or(Value::Null)
}

/// `mask_license_key_cmd { licenseKey }`
pub fn mask_license_key_cmd(args: &Value) -> Value {
    let key = args.get("licenseKey").and_then(|v| v.as_str()).unwrap_or_default();
    Value::String(key_fingerprint(key))
}

/// `secure_storage_save` alias used during activation (delegates to vault).
pub fn persist_activation(app: &tauri::AppHandle, args: &Value) -> Result<Value, String> {
    secure_store::secure_storage_save(app, args)
}