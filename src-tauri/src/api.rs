//! AI API layer — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `api::{audio, catalog, chat, get_api_base,
//! get_app_endpoint, get_stored_credentials, license, server_error_message,
//! transform, types}`. The frontend drives model/provider selection and BYOK
//! keys; chat streams over the `chat_stream_chunk` / `chat_stream_complete`
//! events (observed) and `chat_stream_response` accepts
//! `{ userMessage, systemPrompt, imageBase64, history }`.
//!
//! The proprietary `api.pluely.com` relay needs an access key, so this module
//! implements the local BYOK path: it resolves the selected provider from the
//! secure vault (`selected_pluely_model` / BYOK map) and performs a direct
//! OpenAI-compatible streaming request. Without credentials it emits a
//! deterministic placeholder stream so the UI stays exercisable.

use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::secure_store;

pub const EVT_CHUNK: &str = "chat_stream_chunk";
pub const EVT_COMPLETE: &str = "chat_stream_complete";

static STREAM_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
struct ResolvedProvider {
    endpoint: String,
    api_key: String,
    model: String,
    /// `openai` (chat/completions) or `anthropic` (messages).
    kind: &'static str,
}

/// Map a provider id to its OpenAI-compatible endpoint (from recovered `BK`).
fn endpoint_for(provider: &str) -> Option<(&'static str, &'static str)> {
    Some(match provider {
        "openai" => ("https://api.openai.com/v1/chat/completions", "openai"),
        "groq" => ("https://api.groq.com/openai/v1/chat/completions", "openai"),
        "mistral" => ("https://api.mistral.ai/v1/chat/completions", "openai"),
        "perplexity" => ("https://api.perplexity.ai/chat/completions", "openai"),
        "openrouter" => ("https://openrouter.ai/api/v1/chat/completions", "openai"),
        "cohere" => ("https://api.cohere.ai/v2/chat", "openai"),
        "gemini" => (
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            "openai",
        ),
        "grok" => ("https://api.x.ai/v1/chat/completions", "openai"),
        "ollama" => ("http://localhost:11434/v1/chat/completions", "openai"),
        "claude" => ("https://api.anthropic.com/v1/messages", "anthropic"),
        _ => return None,
    })
}

/// Resolve provider + key from the secure vault.
fn resolve_provider(app: &AppHandle) -> Option<ResolvedProvider> {
    let vault = match secure_store::secure_storage_get(app) {
        Value::Object(m) => m,
        _ => return None,
    };

    // `selected_pluely_model` holds a JSON { provider, model, ... } selection.
    let (provider, model) = vault
        .get("selected_pluely_model")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|sel| {
            let provider = sel.get("provider")?.as_str()?.to_string();
            let model = sel
                .get("model")
                .or_else(|| sel.get("id"))
                .and_then(|m| m.as_str())
                .unwrap_or("gpt-4o-mini")
                .to_string();
            Some((provider, model))
        })?;

    // BYOK keys live in the `byok_providers` map.
    let byok = vault
        .get("byok_providers")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| json!({}));
    let api_key = byok
        .get(&provider)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if api_key.is_empty() {
        return None;
    }

    let (endpoint, kind) = endpoint_for(&provider)?;
    Some(ResolvedProvider {
        endpoint: endpoint.to_string(),
        api_key,
        model,
        kind,
    })
}

/// `fetch_models` — hosted model catalog.
///
/// Recovered contract (overlay maps each entry as `{id, name, isAvailable}`,
/// with optional `modality: ["image"]` and `description`; prompt-select also
/// matches `model === modelId`). Entries below mirror the provider set and
/// model IDs observed in the shipped frontend (`Ype` catalogs, `xnt`
/// web-search IDs); availability is uniform because this rebuild serves its
/// own relay/BYOK rather than the proprietary catalog endpoint.
pub fn fetch_models() -> Value {
    let m = |id: &str, name: &str, desc: &str, image: bool| {
        let provider = id.split('/').next().unwrap_or("openai");
        let mut o = json!({
            "id": id,
            "model": id,
            "name": name,
            "description": desc,
            "provider": provider,
            "isAvailable": true
        });
        if image {
            o["modality"] = json!(["text", "image"]);
        }
        o
    };
    Value::Array(vec![
        m("openai/gpt-4o-mini", "GPT-4o Mini", "Fast everyday model", true),
        m("openai/gpt-4o-mini-search-preview", "GPT-4o Mini Search", "Web-grounded answers", true),
        m("openai/gpt-5.1", "GPT-5.1", "Frontier reasoning", true),
        m("openai/gpt-5.1-codex", "GPT-5.1 Codex", "Agentic coding", false),
        m("openai/gpt-5-codex-mini", "GPT-5 Codex Mini", "Lightweight coding", false),
        m("claude/sonnet", "Sonnet (latest)", "Balanced Claude model", true),
        m("claude/opus", "Opus (latest)", "Most capable Claude model", true),
        m("claude/haiku", "Haiku (latest)", "Fastest Claude model", false),
        m("gemini/gemini-3-pro-preview", "Gemini 3 Pro (preview)", "Google flagship preview", true),
        m("gemini/gemini-2.5-pro", "Gemini 2.5 Pro", "Strong reasoning + vision", true),
        m("gemini/gemini-2.5-flash", "Gemini 2.5 Flash", "Fast Gemini model", true),
        m("grok/grok-4", "Grok 4", "xAI flagship", true),
        m("mistral/mistral-large", "Mistral Large", "EU flagship", false),
        m("cohere/command-r-plus", "Command R+", "RAG-optimized chat", false),
        m("groq/llama-3.3-70b", "Llama 3.3 70B (Groq)", "Ultra-low latency", false),
        m("perplexity/sonar-pro", "Sonar Pro", "Web search answers", false),
        m("openrouter/auto", "Auto (OpenRouter)", "Best-route selection", true),
        m("qwen/qwen3-coder-plus", "Qwen3 Coder Plus", "Coding specialist", false),
        m("qwen/qwen3-vl", "Qwen3 VL", "Vision-language", true),
        m("ollama/llama3.2-vision", "Llama 3.2 Vision", "Local vision model", true),
        m("ollama/qwen2.5vl", "Qwen2.5 VL", "Local vision model", true),
    ])
}

/// `get_stored_credentials`
pub fn get_stored_credentials(app: &AppHandle) -> Value {
    match secure_store::byok_store_get(app) {
        Value::String(s) => serde_json::from_str(&s).unwrap_or_else(|_| json!({})),
        _ => json!({}),
    }
}

fn build_messages(system: &str, user: &str, history: &Value, image: Option<&str>) -> Value {
    let mut messages = vec![json!({ "role": "system", "content": system })];
    if let Some(arr) = history.as_array() {
        for m in arr {
            messages.push(m.clone());
        }
    }
    if let Some(img) = image.filter(|s| !s.is_empty()) {
        messages.push(json!({
            "role": "user",
            "content": [
                { "type": "text", "text": user },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{img}") } }
            ]
        }));
    } else {
        messages.push(json!({ "role": "user", "content": user }));
    }
    Value::Array(messages)
}

fn run_stream(app: AppHandle, provider: ResolvedProvider, args: Value) {
    let user = args.get("userMessage").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let system = args.get("systemPrompt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let image = args.get("imageBase64").and_then(|v| v.as_str()).map(|s| s.to_string());
    let history = args.get("history").cloned().unwrap_or_else(|| json!([]));

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(_) => return,
        };
        rt.block_on(async move {
            let client = reqwest::Client::new();
            let messages = build_messages(&system, &user, &history, image.as_deref());

            let request = if provider.kind == "anthropic" {
                client
                    .post(&provider.endpoint)
                    .header("x-api-key", &provider.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&json!({
                        "model": provider.model,
                        "max_tokens": 1024,
                        "stream": true,
                        "system": system,
                        "messages": messages.as_array().map(|a| a.iter().filter(|m| m.get("role") != Some(&json!("system"))).cloned().collect::<Vec<_>>()).unwrap_or_default()
                    }))
            } else {
                client
                    .post(&provider.endpoint)
                    .header("Authorization", format!("Bearer {}", provider.api_key))
                    .json(&json!({
                        "model": provider.model,
                        "messages": messages,
                        "stream": true
                    }))
            };

            let resp = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = app.emit(EVT_CHUNK, format!("[error] {e}"));
                    let _ = app.emit(EVT_COMPLETE, ());
                    return;
                }
            };

            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                if !STREAM_ACTIVE.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(bytes) = chunk else { break };
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                // SSE: process complete `data:` lines.
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer.drain(..=pos);
                    let Some(data) = line.strip_prefix("data:") else { continue };
                    let data = data.trim();
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(v) = serde_json::from_str::<Value>(data) {
                        let delta = v
                            .pointer("/choices/0/delta/content")
                            .or_else(|| v.pointer("/delta/text"))
                            .and_then(|c| c.as_str())
                            .unwrap_or_default();
                        if !delta.is_empty() {
                            let _ = app.emit(EVT_CHUNK, delta);
                        }
                    }
                }
            }
            STREAM_ACTIVE.store(false, Ordering::SeqCst);
            let _ = app.emit(EVT_COMPLETE, ());
        });
    });
}

/// `chat_stream_response { userMessage, systemPrompt, imageBase64, history }`
pub fn chat_stream_response(app: &AppHandle, args: &Value) -> Result<Value, String> {
    STREAM_ACTIVE.store(true, Ordering::SeqCst);

    if let Some(provider) = resolve_provider(app) {
        run_stream(app.clone(), provider, args.clone());
        return Ok(Value::Null);
    }

    // No BYOK credentials: deterministic placeholder stream.
    let handle = app.clone();
    let user = args
        .get("userMessage")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    std::thread::spawn(move || {
        let text = format!(
            "[no provider configured] Received {} characters. Add a BYOK provider in Dev Space to stream real completions.",
            user.chars().count()
        );
        for word in text.split_inclusive(' ') {
            if !STREAM_ACTIVE.load(Ordering::SeqCst) {
                break;
            }
            let _ = handle.emit(EVT_CHUNK, word);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        STREAM_ACTIVE.store(false, Ordering::SeqCst);
        let _ = handle.emit(EVT_COMPLETE, ());
    });
    Ok(Value::Null)
}

/// `chat_stream_stop`
pub fn chat_stream_stop() -> Value {
    STREAM_ACTIVE.store(false, Ordering::SeqCst);
    Value::Null
}

/// `check_license_status`
pub fn check_license_status(app: &AppHandle) -> Value {
    let has = app
        .state::<crate::shortcuts::LicenseState>()
        .0
        .lock()
        .map(|g| *g)
        .unwrap_or(false);
    json!({
        "is_active": has,
        "is_dev_license": false,
        "last_validated_at": null,
        "license_key": null
    })
}

pub fn get_api_base() -> Value {
    Value::String(crate::session::API_END_POINT.to_string())
}

pub fn get_app_endpoint() -> Value {
    Value::String("https://pluely.com".to_string())
}

/// `server_error_message` — map transport errors to the observed user text.
pub fn server_error_message(kind: &str) -> Value {
    let msg = match kind {
        "document" => "Could not reach the document service",
        "transcription" => "Could not reach the transcription service",
        "live" => "Could not reach the live transcription service",
        _ => "Could not reach the server",
    };
    Value::String(msg.to_string())
}