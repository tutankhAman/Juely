//! Licensed server API + transforms — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `server_api::{api_url, fetch_listen_session, generate_followups,
//! get_banner, get_entitlements, http_client, licensed_request, listen_heartbeat,
//! ocr_document, transcribe_wav, ...}`. All of these target `api.pluely.com`
//! and require a license/access key, so this reconstruction preserves the
//! command surface and response shapes but returns "not configured" locally.

use serde_json::{json, Value};

use crate::session;

pub fn api_url(path: &str) -> String {
    session::session_url(path)
}

/// `transform_text { text, mode, language, instructions }`
///
/// Transform modes recovered from the frontend: summary, concise, longer,
/// keypoints, translate, walkthrough.
pub fn transform_text(args: &Value) -> Value {
    let text = args.get("text").and_then(|v| v.as_str()).unwrap_or_default();
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("summary");
    json!({
        "text": text,
        "mode": mode,
        "result": format!("[phase2c] transform '{mode}' requires the licensed service"),
        "configured": false
    })
}

/// `generate_followups { history, customPrompt }`
pub fn generate_followups(_args: &Value) -> Value {
    json!({ "chips": [], "max_chips": 0, "configured": false })
}

/// `ocr_document { filename, mimeType, dataBase64 }`
pub fn ocr_document(args: &Value) -> Value {
    let pages = 0;
    json!({
        "text": "",
        "pages": pages,
        "filename": args.get("filename").cloned().unwrap_or(Value::Null),
        "configured": false
    })
}

/// `transcribe_audio { audioBase64 }`
pub fn transcribe_audio(_args: &Value) -> Value {
    json!({ "success": false, "transcription": "", "error": "Transcription service not configured" })
}

/// `get_banner`
pub fn get_banner() -> Value {
    json!({ "banner": null })
}

/// `get_entitlements` — dev tier when a LOCAL- license is present.
pub fn get_entitlements(app: &tauri::AppHandle) -> Value {
    let vault = crate::secure_store::secure_storage_get(app);
    let local = vault
        .get("pluely_license_key")
        .and_then(|v| v.as_str())
        .map(|k| k.starts_with("LOCAL-"))
        .unwrap_or(false);
    if local {
        return json!({
            "success": true,
            "tier": "dev",
            "plan_type": "paid",
            "byo_keys": true,
            "live": { "limit": 36000, "used": 0, "remaining": 36000, "included": true },
            "uploads": { "limit": 1000, "used": 0, "remaining": 1000, "included": true },
            "completions": { "limit": 100000, "used": 0, "remaining": 100000, "included": true },
            "followups": { "limit": 100000, "used": 0, "remaining": 100000, "included": true },
            "followupChips": 4,
            "renews_at": null,
            "resets_at": null
        });
    }
    json!({ "success": false, "tier": "free", "plan_type": "free", "configured": false })
}

/// `fetch_prompts`
pub fn fetch_prompts() -> Value {
    json!({ "prompts": [], "last_updated": null })
}

/// `create_system_prompt { userPrompt }`
pub fn create_system_prompt(args: &Value) -> Value {
    let user_prompt = args
        .get("userPrompt")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    json!({
        "system_prompt": user_prompt,
        "prompt_name": "Custom",
        "configured": false
    })
}

/// `get_activity`
pub fn get_activity() -> Value {
    json!({ "success": false, "data": [], "total_tokens_used": 0 })
}

/// `licensed_request` — single entry point for authed calls (stub).
pub fn licensed_request(path: &str) -> Value {
    json!({ "ok": false, "url": api_url(path), "configured": false })
}