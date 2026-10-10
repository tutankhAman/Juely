//! Global shortcuts — reconstructed surface.
//!
//! Upstream uses `tauri-plugin-global-shortcut` on macOS/Windows and the XDG
//! `org.freedesktop.portal.GlobalShortcuts` portal on Linux/Wayland. Phase 2a
//! implements registration via the plugin plus the portal status/validation
//! commands the frontend expects. Portal binding is best-effort.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutBinding {
    #[serde(default)]
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    /// Map of action id -> binding (the frontend sends an object, not an array).
    #[serde(default)]
    pub bindings: HashMap<String, ShortcutBinding>,
    #[serde(default, rename = "customActions")]
    pub custom_actions: Vec<Value>,
}

#[derive(Default)]
pub struct RegisteredShortcuts(pub Mutex<Vec<String>>);

/// Main-overlay visibility — tracks whether the last state we applied was
/// hidden, so `toggle_window` show/hide on shortcut press matches upstream
/// (`WindowVisibility { is_hidden }`).
#[derive(Default)]
pub struct WindowVisibility(pub Mutex<bool>);

/// Move-mode state for `move_window` (hold ctrl + arrows).
#[derive(Default)]
pub struct MoveWindowState {
    pub active: Mutex<bool>,
}

/// Callback registered by `listen` so shortcut-driven "start or stop
/// listening" can reach the mic pipeline without a Piece state cycle.
pub type ListenToggleFn = Box<dyn Fn(&AppHandle) + Send + Sync>;

#[derive(Default)]
pub struct ListenToggle(pub Mutex<Option<ListenToggleFn>>);

/// Wire the global-shortcut "start or stop listening" action to the mic
/// pipeline: toggles between `listen_start` (empty args → stored provider
/// config) and `listen_stop`, matching upstream's native handling.
pub fn install_listen_toggle(app: &AppHandle) {
    let cb: ListenToggleFn = Box::new(move |app: &tauri::AppHandle| {
        let listening = app
            .state::<crate::listen::ListenState>()
            .active
            .lock()
            .map(|g| *g)
            .unwrap_or(false);
        if listening {
            crate::listen::listen_stop(app);
        } else if let Err(e) = crate::listen::listen_start(app, &serde_json::json!({})) {
            let _ = app.emit("listen-notice", format!("Could not start listening: {e}"));
        }
    });
    if let Ok(mut g) = app.state::<ListenToggle>().0.lock() {
        *g = Some(cb);
    }
}

/// `validate_shortcut_key { key }`
pub fn validate_shortcut_key(args: &Value) -> Value {
    let key = args.get("key").and_then(|v| v.as_str()).unwrap_or_default();
    Value::Bool(key.parse::<Shortcut>().is_ok())
}

/// `get_registered_shortcuts`
pub fn get_registered_shortcuts(app: &AppHandle) -> Value {
    let state = app.state::<RegisteredShortcuts>();
    let list = state.0.lock().map(|v| v.clone()).unwrap_or_default();
    serde_json::to_value(list).unwrap_or(Value::Array(vec![]))
}

/// `check_shortcuts_registered`
pub fn check_shortcuts_registered(app: &AppHandle) -> Value {
    let state = app.state::<RegisteredShortcuts>();
    Value::Bool(state.0.lock().map(|v| !v.is_empty()).unwrap_or(false))
}

/// `get_shortcuts_portal_status` — Linux/Wayland only; null elsewhere.
pub fn get_shortcuts_portal_status() -> Value {
    #[cfg(target_os = "linux")]
    {
        serde_json::json!({
            "supported": true,
            "bound": false,
            "backend": "org.freedesktop.portal.GlobalShortcuts"
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        Value::Null
    }
}

/// `update_shortcuts { config }`
pub fn update_shortcuts(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let config: ShortcutsConfig = args
        .get("config")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let mut registered = Vec::new();
    for (id, b) in &config.bindings {
        if !b.enabled {
            continue;
        }
        let Ok(sc) = b.key.parse::<Shortcut>() else {
            continue;
        };
        let id = if b.id.is_empty() { id.clone() } else { b.id.clone() };
        let is_move = id.starts_with("move_window_") || id == "move_window";
        if is_move && !app.state::<LicenseState>().licensed() {
            eprintln!("shortcut-registration-error; Skipping move_window registration - license inactive");
            continue;
        }
        let action_id = id.clone();
        let handle = app.clone();
        if gs
            .on_shortcut(sc, move |_app, _shortcut, event| {
                match event.state {
                    ShortcutState::Pressed => {
                        if let Some(dir) = action_id.strip_prefix("move_window_") {
                            start_move_window(&handle, dir);
                        } else {
                            handle_shortcut_action(&handle, &action_id);
                        }
                    }
                    ShortcutState::Released => {
                        if action_id.starts_with("move_window_") {
                            stop_move_window(&handle);
                        }
                    }
                }
            })
            .is_ok()
        {
            registered.push(id);
        }
    }

    let state = app.state::<RegisteredShortcuts>();
    if let Ok(mut v) = state.0.lock() {
        *v = registered;
    }
    Ok(Value::Null)
}

/// `set_license_status { hasLicense }` — recorded for shortcut gating.
pub fn set_license_status(app: &AppHandle, args: &Value) -> Value {
    let has = args
        .get("hasLicense")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    app.state::<LicenseState>().set(has);
    Value::Null
}

#[derive(Default)]
pub struct LicenseState(pub Mutex<bool>);

impl LicenseState {
    pub fn set(&self, v: bool) {
        if let Ok(mut g) = self.0.lock() {
            *g = v;
        }
    }

    pub fn licensed(&self) -> bool {
        self.0.lock().map(|g| *g).unwrap_or(false)
    }
}

/// `handle_shortcut_action` — native executor for the recovered action ids.
/// Upstream performs window/listen actions in Rust and routes capture actions
/// to the frontend via trigger events (`trigger-screenshot`,
/// `start-audio-recording`, `toggle-system-audio`, `focus-text-input`).
pub fn handle_shortcut_action(app: &AppHandle, action: &str) {
    let licensed = app.state::<LicenseState>().licensed();
    match action {
        "toggle_window" => {
            let was_hidden = app
                .state::<WindowVisibility>()
                .0
                .lock()
                .map(|g| *g)
                .unwrap_or(false);
            let _ = crate::window::toggle_main_window(app, !was_hidden);
            if let Ok(mut g) = app.state::<WindowVisibility>().0.lock() {
                *g = !was_hidden;
            }
        }
        "toggle_dashboard" => {
            if let Err(e) = crate::window::toggle_dashboard(app) {
                eprintln!("Failed to toggle dashboard window: {e}");
            }
        }
        "focus_input" => {
            if crate::window::show_main_window(app).is_err() {
                eprintln!("Main window not found");
            }
            let _ = app.emit("focus-text-input", ());
        }
        "screenshot" => {
            if licensed {
                let _ = app.emit("trigger-screenshot", ());
            } else {
                eprintln!("Ignoring screenshot - license inactive");
            }
        }
        "audio_recording" => {
            if licensed {
                let _ = app.emit("start-audio-recording", ());
            } else {
                eprintln!("Ignoring audio_recording - license inactive");
            }
        }
        "system_audio" => {
            if licensed {
                let _ = app.emit("toggle-system-audio", ());
            } else {
                eprintln!("Ignoring system_audio - license inactive");
            }
        }
        "listen" | "push_to_talk" => {
            if licensed {
                if let Ok(guard) = app.state::<ListenToggle>().0.lock() {
                    if let Some(f) = guard.as_ref() {
                        f(app);
                    }
                }
            }
        }
        "move_window" => {
            // Directional move starts on the arrow-key shortcuts; the modifier
            // binding itself only cancels any active move.
            stop_move_window(app);
        }
        _ => {
            eprintln!("Unknown shortcut action '{action}'");
        }
    }
}

const MOVE_STEP: i32 = 24;

/// Start/step one directional move (`move_window_{left,right,up,down}`).
pub fn start_move_window(app: &AppHandle, direction: &str) {
    if !app.state::<LicenseState>().licensed() {
        eprintln!("Ignoring move_window start for direction '{direction}' - license inactive");
        return;
    }
    let Some(w) = app.get_webview_window("main") else {
        eprintln!("Main window not found");
        return;
    };
    let Ok(pos) = w.outer_position() else { return };
    let Ok(size) = w.outer_size() else { return };
    let (mut x, mut y) = (pos.x, pos.y);
    match direction {
        "left" => x -= MOVE_STEP,
        "right" => x += MOVE_STEP,
        "up" => y -= MOVE_STEP,
        "down" => y += MOVE_STEP,
        _ => return,
    }
    if let Ok(Some(m)) = w.current_monitor() {
        let mp = m.position();
        let ms = m.size();
        x = x.clamp(mp.x, mp.x + ms.width as i32 - size.width as i32);
        y = y.clamp(mp.y, mp.y + ms.height as i32 - size.height as i32);
    }
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    if let Ok(mut g) = app.state::<MoveWindowState>().active.lock() {
        *g = true;
    }
}

/// Release the move-mode modifier.
pub fn stop_move_window(app: &AppHandle) {
    if let Ok(mut g) = app.state::<MoveWindowState>().active.lock() {
        *g = false;
    }
}

/// Default Linux shortcut set recovered from the frontend config.
pub fn default_bindings() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("toggle_dashboard", "ctrl+shift+d"),
        ("toggle_window", "ctrl+backslash"),
        ("focus_input", "ctrl+shift+i"),
        ("system_audio", "ctrl+shift+m"),
        ("audio_recording", "ctrl+shift+a"),
        ("screenshot", "ctrl+shift+s"),
        ("listen", "ctrl+shift+l"),
        ("push_to_talk", "ctrl+shift+p"),
        ("move_window_left", "ctrl+left"),
        ("move_window_right", "ctrl+right"),
        ("move_window_up", "ctrl+up"),
        ("move_window_down", "ctrl+down"),
    ])
}

/// `setup_global_shortcuts` — install defaults at startup (best-effort).
pub fn setup_global_shortcuts(app: &AppHandle) {
    let bindings: HashMap<String, ShortcutBinding> = default_bindings()
        .into_iter()
        .map(|(id, key)| {
            (
                id.to_string(),
                ShortcutBinding { id: id.to_string(), key: key.to_string(), enabled: true },
            )
        })
        .collect();
    let cfg = serde_json::to_value(ShortcutsConfig { bindings, custom_actions: Vec::new() }).unwrap_or(Value::Null);
    let args = serde_json::json!({ "config": cfg });
    let _ = update_shortcuts(app, &args);
    // Broadcast the portal status so the dashboard's shortcuts panel renders
    // its "Auto (portal)" state (frontend listens for updates too).
    let _ = app.emit("shortcuts-portal-status", get_shortcuts_portal_status());
}