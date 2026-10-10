//! Pluely Phase 1/2a — clean-room reconstruction scaffold.
//!
//! Runs the recovered production frontend (`dist/`) inside a Tauri 2 shell.
//! The IPC router logs every command the UI invokes, dispatches the commands
//! implemented so far (Phase 2a: window + shortcuts), and replies with
//! type-appropriate placeholders for the rest.

use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::ipc::{InvokeBody, InvokeMessage, InvokeResolver};
use tauri::{Manager, WebviewWindowBuilder};

mod activate;
mod api;
mod audio;
mod capture;
mod cli;
mod db;
mod listen;
mod secure_store;
mod purchase;
mod server_api;
mod session;
mod single_instance;
mod speaker;
mod stealth_keys;
mod shared_prefs;
mod shortcuts;
mod window;

/// Every command observed in the frontend/backend static analysis.
const KNOWN_COMMANDS: &[&str] = &[
    "get_app_version", "get_machine_uid", "get_distinct_id", "get_config",
    "set_window_height", "set_window_size", "open_dashboard", "toggle_dashboard",
    "is_dashboard_open", "move_window", "fit_window_to_monitor", "track_overlay_window",
    "open_meeting_overlay", "close_meeting_overlay", "close_overlay_window",
    "capture_to_base64", "start_screen_capture", "capture_selected_area",
    "pick_capture_region", "enable_text_capture", "disable_text_capture",
    "apply_window_stealth", "set_always_on_top", "set_content_protected",
    "set_app_icon_visibility", "update_stealth_keys", "enable_stealth_keys", "disable_stealth_keys",
    "meeting_detect_set_enabled", "check_shortcuts_registered", "get_registered_shortcuts",
    "get_shortcuts_portal_status", "update_shortcuts", "validate_shortcut_key",
    "set_license_status", "exit_app", "activate_license_api", "deactivate_license_api",
    "validate_license_api", "check_license_status", "mask_license_key_cmd",
    "get_checkout_url", "start_checkout", "claim_purchase", "submit_lead",
    "get_billing_portal", "get_plan_catalog", "get_entitlements", "get_banner",
    "secure_storage_save", "secure_storage_get", "secure_storage_remove",
    "byok_store_set", "byok_store_get", "shared_prefs_broadcast", "transcribe_audio",
    "chat_stream_response", "chat_stream_stop", "fetch_models", "fetch_prompts",
    "create_system_prompt", "transform_text", "generate_followups", "get_activity",
    "ocr_document", "start_system_audio_capture", "stop_system_audio_capture",
    "manual_stop_continuous", "check_system_audio_access", "check_system_audio_supported",
    "request_system_audio_access", "get_vad_config", "update_vad_config",
    "get_capture_status", "get_audio_sample_rate", "get_input_devices",
    "get_output_devices", "listen_start", "listen_start_local", "listen_stop",
    "listen_set_paused", "get_listen_languages", "ptt_start", "ptt_cancel",
    "ptt_stop_and_transcribe", "ptt_stop_take_wav", "cli_detect", "cli_chat_stream",
    "cli_chat_stop", "cli_ollama_models", "open_permission_settings",
];

#[derive(Default)]
struct TraceState {
    lines: Mutex<Vec<String>>,
}

fn summarize(command: &str, body: &InvokeBody) -> String {
    let payload = match body {
        InvokeBody::Json(v) => v
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>().join(","))
            .unwrap_or_else(|| v.to_string()),
        InvokeBody::Raw(bytes) => format!("raw:{}B", bytes.len()),
    };
    format!("{command}  [{payload}]")
}

fn placeholder(command: &str) -> Value {
    match command {
        "get_app_version" => json!("1.1.0"),
        "get_machine_uid" => json!("phase1-machine-uid"),
        "get_distinct_id" => json!("phase1-distinct-id"),
        "check_system_audio_supported" => json!(null),
        "check_license_status" => json!({
            "is_active": false, "is_dev_license": false,
            "last_validated_at": null, "license_key": null
        }),
        "get_registered_shortcuts" | "get_input_devices" | "get_output_devices"
        | "get_listen_languages" | "fetch_prompts" | "cli_ollama_models"
        | "get_plan_catalog" | "get_entitlements" => json!([]),
        "get_vad_config" => json!({
            "sensitivity_rms": 0.02, "peak_threshold": 0.1, "silence_chunks": 8,
            "min_speech_chunks": 2, "pre_speech_chunks": 4,
            "noise_gate_threshold": 0.01, "max_recording_duration_secs": 300
        }),
        "get_audio_sample_rate" => json!(16000),
        "get_capture_status" => json!("idle"),
        "get_config" => json!({}),
        "get_banner" => json!(null),
        "fetch_models" => json!([]),
        "capture_to_base64" => json!(""),
        "transform_text" => json!({ "text": "" }),
        "generate_followups" => json!({ "chips": [], "max_chips": 0 }),
        "ocr_document" => json!({ "text": "", "pages": 0 }),
        _ => json!(null),
    }
}

/// Returns `Some(value)` when Phase 2a implements the command.
fn dispatch(app: &tauri::AppHandle, command: &str, args: &Value) -> Option<Result<Value, String>> {
    use window as w;
    Some(match command {
        "set_window_size" => w::set_window_size(app, args),
        "set_window_height" => w::set_window_height(app, args),
        "fit_window_to_monitor" => w::fit_window_to_monitor(app, args),
        "move_window" => w::move_window(app, args),
        "track_overlay_window" => w::track_overlay_window(app),
        "open_dashboard" => w::open_dashboard(app),
        "toggle_dashboard" => w::toggle_dashboard(app),
        "open_meeting_overlay" => w::open_meeting_overlay(app),
        "close_meeting_overlay" => w::close_meeting_overlay(app),
        "close_overlay_window" => w::close_overlay_window(app, args),
        "apply_window_stealth" => w::apply_window_stealth(app),
        "set_always_on_top" => w::set_always_on_top(app, args),
        "set_content_protected" => w::set_content_protected(app, args),
        "exit_app" => Ok(w::exit_app(app)),
        "set_app_icon_visibility" => Ok(w::set_app_icon_visibility(args)),
        "is_dashboard_open" => Ok(Value::Bool(w::is_dashboard_open(app))),
        "validate_shortcut_key" => Ok(shortcuts::validate_shortcut_key(args)),
        "get_registered_shortcuts" => Ok(shortcuts::get_registered_shortcuts(app)),
        "check_shortcuts_registered" => Ok(shortcuts::check_shortcuts_registered(app)),
        "get_shortcuts_portal_status" => Ok(shortcuts::get_shortcuts_portal_status()),
        "update_shortcuts" => shortcuts::update_shortcuts(app, args),
        "set_license_status" => Ok(shortcuts::set_license_status(app, args)),
        "shared_prefs_broadcast" => Ok(shared_prefs::shared_prefs_broadcast(app, args)),
        "secure_storage_save" => secure_store::secure_storage_save(app, args),
        "secure_storage_get" => Ok(secure_store::secure_storage_get(app)),
        "secure_storage_remove" => secure_store::secure_storage_remove(app, args),
        "byok_store_set" => secure_store::byok_store_set(app, args),
        "byok_store_get" => Ok(secure_store::byok_store_get(app)),
        "get_machine_uid" => Ok(session::get_machine_uid(app)),
        "fetch_models" => Ok(api::fetch_models()),
        "chat_stream_response" => api::chat_stream_response(app, args),
        "chat_stream_stop" => Ok(api::chat_stream_stop()),
        "check_license_status" => Ok(api::check_license_status(app)),
        "transform_text" => Ok(server_api::transform_text(args)),
        "generate_followups" => Ok(server_api::generate_followups(args)),
        "ocr_document" => Ok(server_api::ocr_document(args)),
        "transcribe_audio" => Ok(server_api::transcribe_audio(args)),
        "get_banner" => Ok(server_api::get_banner()),
        "get_entitlements" => Ok(server_api::get_entitlements(app)),
        "fetch_prompts" => Ok(server_api::fetch_prompts()),
        "create_system_prompt" => Ok(server_api::create_system_prompt(args)),
        "get_activity" => Ok(server_api::get_activity()),
        "capture_to_base64" => capture::capture_to_base64(app, args),
        "capture_selected_area" => capture::capture_selected_area(app, args),
        "pick_capture_region" => Ok(capture::pick_capture_region(app)),
        "start_screen_capture" => Ok(capture::start_screen_capture(app)),
        "get_capture_status" => Ok(capture::get_capture_status(app)),
        "enable_text_capture" => Ok(capture::set_text_capture(app, true)),
        "disable_text_capture" => Ok(capture::set_text_capture(app, false)),
        "get_input_devices" => Ok(speaker::list_input_devices()),
        "get_output_devices" => Ok(speaker::list_output_devices()),
        "get_audio_sample_rate" => Ok(speaker::get_audio_sample_rate()),
        "check_system_audio_access" => Ok(speaker::check_system_audio_access()),
        "check_system_audio_supported" => Ok(speaker::check_system_audio_supported()),
        "request_system_audio_access" => Ok(speaker::request_system_audio_access()),
        "open_permission_settings" => Ok(speaker::open_permission_settings(args)),
        "get_vad_config" => Ok(listen::get_vad_config(app)),
        "update_vad_config" => listen::update_vad_config(app, args),
        "get_listen_languages" => Ok(listen::get_listen_languages()),
        "listen_start" => listen::listen_start(app, args),
        "listen_start_local" => Ok(listen::listen_start_local(app, args)),
        "listen_stop" => Ok(listen::listen_stop(app)),
        "listen_set_paused" => Ok(listen::listen_set_paused(app, args)),
        "start_system_audio_capture" => Ok(listen::start_system_audio_capture(app)),
        "stop_system_audio_capture" => Ok(listen::stop_system_audio_capture(app)),
        "manual_stop_continuous" => Ok(listen::manual_stop_continuous(app)),
        "ptt_start" => Ok(listen::ptt_start(app, args)),
        "ptt_cancel" => Ok(listen::ptt_cancel(app)),
        "ptt_stop_take_wav" => Ok(listen::ptt_stop_take_wav(app)),
        "ptt_stop_and_transcribe" => Ok(listen::ptt_stop_and_transcribe(app)),
        "meeting_detect_set_enabled" => Ok(listen::meeting_detect_set_enabled(app, args)),
        "activate_license_api" => Ok(activate::activate_license_api(app, args)),
        "deactivate_license_api" => Ok(activate::deactivate_license_api()),
        "validate_license_api" => Ok(activate::validate_license_api(app)),
        "mask_license_key_cmd" => Ok(activate::mask_license_key_cmd(args)),
        "get_plan_catalog" => Ok(purchase::get_plan_catalog()),
        "start_checkout" => Ok(purchase::start_checkout(args)),
        "claim_purchase" => Ok(purchase::claim_purchase()),
        "submit_lead" => Ok(purchase::submit_lead(args)),
        "get_billing_portal" => Ok(purchase::get_billing_portal()),
        "enable_stealth_keys" => Ok(stealth_keys::enable_stealth_keys(app)),
        "disable_stealth_keys" => Ok(stealth_keys::disable_stealth_keys(app)),
        "update_stealth_keys" => stealth_keys::update_stealth_keys(app, args),
        "cli_detect" => Ok(cli::cli_detect()),
        "cli_ollama_models" => Ok(cli::cli_ollama_models()),
        "cli_chat_stream" => cli::cli_chat_stream(app, args),
        "cli_chat_stop" => Ok(cli::cli_chat_stop(app)),
        _ => return None,
    })
}

fn trace(message: InvokeMessage, resolver: InvokeResolver) {
    let command = message.command().to_string();
    let args = match message.payload() {
        InvokeBody::Json(v) => v.clone(),
        InvokeBody::Raw(_) => Value::Null,
    };
    let known = KNOWN_COMMANDS.contains(&command.as_str());
    println!(
        "[ipc] {}{}",
        summarize(&command, message.payload()),
        if known { "" } else { "   <-- UNKNOWN/NEW" }
    );

    let app = message.webview().app_handle().clone();
    let result = match dispatch(&app, &command, &args) {
        Some(r) => r,
        None => Ok(placeholder(&command)),
    };
    resolver.respond(result.map_err(tauri::ipc::InvokeError::from));
}

/// Fallback drag wiring: if native `data-tauri-drag-region` hit-testing does
/// not start a move (platform/window-manager dependent), route left-button
/// mousedowns inside drag regions through `plugin:window|start_dragging`.
const DRAG_FALLBACK_JS: &str = r#"
window.addEventListener('mousedown', (e) => {
  try {
    if (e.button !== 0) return;
    const t = e.target && e.target.closest ? e.target.closest('[data-tauri-drag-region="true"]') : null;
    if (!t) return;
    if (e.target.closest('button,input,textarea,select,a,[contenteditable="true"]')) return;
    const w = window.__TAURI__ && window.__TAURI__.window
      ? window.__TAURI__.window.getCurrentWindow() : null;
    if (w && w.startDragging) w.startDragging().catch(() => {});
  } catch (_) {}
}, true);
"#;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .append_invoke_initialization_script(DRAG_FALLBACK_JS)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:pluely.db", db::migrations())
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(TraceState::default())
        .manage(shortcuts::RegisteredShortcuts::default())
        .manage(shortcuts::LicenseState::default())
        .manage(shortcuts::WindowVisibility::default())
        .manage(shortcuts::MoveWindowState::default())
        .manage(shortcuts::ListenToggle::default())
        .manage(shared_prefs::SharedPrefs::default())
        .manage(secure_store::MachineId::default())
        .manage(session::SessionState::default())
        .manage(capture::CaptureState::default())
        .manage(listen::ListenState::default())
        .manage(listen::MeetingDetectState::default())
        .manage(activate::ValidateCache::default())
        .manage(stealth_keys::StealthState::default())
        .manage(cli::CliState::default())
        .invoke_handler(|invoke| {
            trace(invoke.message, invoke.resolver);
            true
        })
        .setup(|app: &mut tauri::App<tauri::Wry>| {
            let handle = app.handle().clone();
            if handle.get_webview_window("main").is_none() {
                WebviewWindowBuilder::new(&handle, "main", tauri::WebviewUrl::default())
                    .title("Pluely - AI Assistant")
                    .inner_size(600.0, 54.0)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(false)
                    .resizable(false)
                    .skip_taskbar(true)
                    .visible(true)
                    .build()?;
            }
            window::setup_main_window(&handle);
            // Upstream pre-creates the dashboard on the main thread so the
            // renderer is ready when `open_dashboard` is invoked later.
            window::precreate_dashboard(&handle);
            shortcuts::install_listen_toggle(&handle);
            listen::spawn_meeting_watcher(&handle);
            shortcuts::setup_global_shortcuts(&handle);
            // Test hook: boot-time dashboard open (verification only).
            if std::env::var("PLUELY_TEST_OPEN_DASHBOARD").is_ok() {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(2500));
                    match crate::window::open_dashboard(&h) {
                        Ok(_) => println!("[test] open_dashboard OK"),
                        Err(e) => println!("[test] open_dashboard FAILED: {e}"),
                    }
                });
            }
            if single_instance::acquire() {
                single_instance::serve(&handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Pluely Phase 1 shell");
}