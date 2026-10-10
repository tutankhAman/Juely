//! Session + machine identity — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `session::{bearer_token, call_start, call_refresh,
//! rotate_or_restart, session_url, warm, TokenPair, SessionState, MACHINE_ID,
//! ROTATING, SESSION}`. Upstream talks to `api.pluely.com/api/session/` and
//! keeps a short-lived bearer/refresh pair. The proprietary service is not
//! reachable without an access key, so this reconstruction keeps the same
//! surface and lifecycle but returns "not configured" until a base URL and
//! license are supplied.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub const API_END_POINT: &str = "https://api.pluely.com";

#[derive(Default)]
pub struct SessionState {
    pub machine_id: Mutex<Option<String>>,
    pub tokens: Mutex<Option<TokenPair>>,
    pub rotating: Mutex<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

/// `get_machine_uid` — stable per-install identity (hostname-derived here).
pub fn get_machine_uid(app: &AppHandle) -> Value {
    let state = app.state::<SessionState>();
    if let Ok(mut g) = state.machine_id.lock() {
        if let Some(v) = g.as_ref() {
            return Value::String(v.clone());
        }
        let id = std::env::var("HOSTNAME")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "phase1".to_string());
        *g = Some(id.clone());
        return Value::String(id);
    }
    Value::String("phase1".to_string())
}

/// `bearer_token` — current access token if a session was established.
pub fn bearer_token(app: &AppHandle) -> Option<String> {
    app.state::<SessionState>()
        .tokens
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|t| t.access_token.clone()))
}

/// `session_url` — endpoint builder.
pub fn session_url(path: &str) -> String {
    format!("{API_END_POINT}{path}")
}

/// `warm` — pre-establish a session (no-op until configured).
pub fn warm(app: &AppHandle) -> Value {
    let state = app.state::<SessionState>();
    let configured = state.tokens.lock().map(|g| g.is_some()).unwrap_or(false);
    json!({ "warm": true, "configured": configured })
}

/// `rotate_or_restart` — mark rotation; real refresh needs the service.
pub fn rotate_or_restart(app: &AppHandle) -> Value {
    let state = app.state::<SessionState>();
    if let Ok(mut r) = state.rotating.lock() {
        *r = true;
    }
    json!({ "rotating": true })
}