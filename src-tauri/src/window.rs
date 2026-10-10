//! Window management — reconstructed from Pluely 1.1.0 behavior.
//!
//! Window labels observed: `main` (600x54 transparent overlay), `dashboard`,
//! `meeting-overlay`. Commands are behavior-matched to the recovered frontend
//! call sites (see recovered/frontend/src/app.pretty.js).

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, WebviewWindowBuilder};

/// Overlay top offset in logical pixels (matches upstream TOP_OFFSET).
pub const TOP_OFFSET: i32 = 54;

static APP_ICON_HIDDEN: AtomicBool = AtomicBool::new(false);

fn window_for(app: &AppHandle, label: &str) -> Result<WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' not found"))
}

fn label_of(args: &Value) -> String {
    args.get("window")
        .and_then(|w| w.get("label"))
        .and_then(|l| l.as_str())
        .unwrap_or("main")
        .to_string()
}

/// `set_window_size { window, width, height }`
pub fn set_window_size(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let w = window_for(app, &label_of(args))?;
    let width = args.get("width").and_then(|v| v.as_f64()).unwrap_or(600.0);
    let height = args.get("height").and_then(|v| v.as_f64()).unwrap_or(54.0);
    w.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// `set_window_height { window, height }` — upstream keeps width at 600.
pub fn set_window_height(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let w = window_for(app, &label_of(args))?;
    let height = args.get("height").and_then(|v| v.as_f64()).unwrap_or(54.0);
    w.set_size(tauri::Size::Logical(tauri::LogicalSize::new(600.0, height)))
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// `fit_window_to_monitor { window }`
pub fn fit_window_to_monitor(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let w = window_for(app, &label_of(args))?;
    let monitor = w
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| w.primary_monitor().ok().flatten())
        .ok_or("no monitor found")?;
    let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
    w.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
        size.width, size.height,
    )))
    .map_err(|e| e.to_string())?;
    w.set_position(PhysicalPosition::new(0, 0))
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// `move_window { window, direction }`
pub fn move_window(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let w = window_for(app, &label_of(args))?;
    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .ok_or("missing direction")?;
    let pos = w.outer_position().map_err(|e| e.to_string())?;
    let size = w.outer_size().map_err(|e| e.to_string())?;
    let step = 40i32;
    let (mut x, mut y) = (pos.x, pos.y);
    match direction {
        "left" => x -= step,
        "right" => x += step,
        "up" => y -= step,
        "down" => y += step,
        _ => return Err(format!("unknown direction '{direction}'")),
    }
    if let Ok(Some(monitor)) = w.current_monitor() {
        let mpos = monitor.position();
        let msize = monitor.size();
        x = x.clamp(mpos.x, mpos.x + msize.width as i32 - size.width as i32);
        y = y.clamp(mpos.y, mpos.y + msize.height as i32 - size.height as i32);
    }
    w.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// `track_overlay_window` — snap the overlay to top-center.
pub fn track_overlay_window(app: &AppHandle) -> Result<Value, String> {
    let Some(w) = app.get_webview_window("main") else {
        return Ok(Value::Null);
    };
    if let Ok(Some(monitor)) = w.primary_monitor() {
        let msize = monitor.size();
        let wsize = w.outer_size().map_err(|e| e.to_string())?;
        let x = (msize.width as i32 - wsize.width as i32) / 2;
        w.set_position(PhysicalPosition::new(x, TOP_OFFSET))
            .map_err(|e| e.to_string())?;
    }
    Ok(Value::Null)
}

fn ensure_dashboard(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window("dashboard") {
        return Ok(w);
    }
    create_dashboard_window(app)
}

/// `create_dashboard_window` — upstream pre-creates the dashboard at startup
/// (binary string: "Failed to pre-create dashboard window on startup") and
/// installs a close handler that hides instead of destroying the window.
pub fn create_dashboard_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    let w = WebviewWindowBuilder::new(app, "dashboard", tauri::WebviewUrl::App("dashboard".into()))
        .title("Pluely - Dashboard")
        .inner_size(1100.0, 720.0)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    setup_dashboard_close_handler(&w)?;
    Ok(w)
}

/// Dashboard close requests hide the window (keeps webview state) rather than
/// destroying it; the frontend keeps listening and `open_dashboard` still works.
pub fn setup_dashboard_close_handler(w: &WebviewWindow) -> Result<(), String> {
    let win = w.clone();
    w.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win.hide();
        }
    });
    Ok(())
}

/// Pre-create the dashboard window early (called from setup on the main thread),
/// matching upstream "Failed to pre-create dashboard window on startup".
pub fn precreate_dashboard(app: &AppHandle) {
    match create_dashboard_window(app) {
        Ok(_) => println!("[startup] dashboard window pre-created"),
        Err(e) => println!("[startup] failed to pre-create dashboard window: {e}"),
    }
}

/// `open_dashboard`
pub fn open_dashboard(app: &AppHandle) -> Result<Value, String> {
    let w = ensure_dashboard(app)?;
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    let _ = app.emit("dashboard-visibility", true);
    Ok(Value::Null)
}

/// `toggle_dashboard`
pub fn toggle_dashboard(app: &AppHandle) -> Result<Value, String> {
    let w = ensure_dashboard(app)?;
    let visible = w.is_visible().unwrap_or(false);
    if visible {
        w.hide().map_err(|e| e.to_string())?;
        let _ = app.emit("dashboard-visibility", false);
    } else {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
        let _ = app.emit("dashboard-visibility", true);
    }
    Ok(Value::Null)
}

/// `is_dashboard_open`
pub fn is_dashboard_open(app: &AppHandle) -> bool {
    app.get_webview_window("dashboard")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// `open_meeting_overlay`
pub fn open_meeting_overlay(app: &AppHandle) -> Result<Value, String> {
    if let Some(w) = app.get_webview_window("meeting-overlay") {
        w.show().map_err(|e| e.to_string())?;
        return Ok(Value::Null);
    }
    WebviewWindowBuilder::new(app, "meeting-overlay", tauri::WebviewUrl::default())
        .title("Pluely Meeting")
        .inner_size(560.0, 420.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// `close_meeting_overlay`
pub fn close_meeting_overlay(app: &AppHandle) -> Result<Value, String> {
    if let Some(w) = app.get_webview_window("meeting-overlay") {
        w.close().map_err(|e| e.to_string())?;
    }
    let _ = app.emit("meeting-ended", ());
    Ok(Value::Null)
}

/// `close_overlay_window { reason }`
pub fn close_overlay_window(app: &AppHandle, args: &Value) -> Result<Value, String> {
    for (label, w) in app.webview_windows() {
        if label.starts_with("capture-overlay-") || label == "overlay" {
            let _ = w.close();
        }
    }
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let _ = app.emit("capture-closed", reason);
    Ok(Value::Null)
}

/// `apply_window_stealth`
pub fn apply_window_stealth(app: &AppHandle) -> Result<Value, String> {
    for label in ["main", "meeting-overlay"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_content_protected(true);
            let _ = w.set_always_on_top(true);
            let _ = w.set_skip_taskbar(true);
        }
    }
    Ok(Value::Null)
}

/// `set_always_on_top { enabled }`
pub fn set_always_on_top(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if let Some(w) = app.get_webview_window("main") {
        w.set_always_on_top(enabled).map_err(|e| e.to_string())?;
    }
    Ok(Value::Null)
}

/// `set_content_protected { enabled }`
pub fn set_content_protected(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    if let Some(w) = app.get_webview_window("main") {
        w.set_content_protected(enabled).map_err(|e| e.to_string())?;
    }
    Ok(Value::Null)
}

/// `set_app_icon_visibility { visible }`
pub fn set_app_icon_visibility(args: &Value) -> Value {
    let visible = args.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
    APP_ICON_HIDDEN.store(!visible, Ordering::Relaxed);
    json!({ "appIconHidden": APP_ICON_HIDDEN.load(Ordering::Relaxed) })
}

/// `exit_app`
pub fn exit_app(app: &AppHandle) -> Value {
    app.exit(0);
    Value::Null
}

/// `setup_main_window` — top-center overlay placement on startup.
pub fn setup_main_window(app: &AppHandle) {
    let _ = track_overlay_window(app);
}

/// Show the main overlay (used by the native shortcut executor).
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Hide the main overlay.
pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    w.hide().map_err(|e| e.to_string())?;
    Ok(())
}

/// Toggle the main overlay visibility (`toggle_window` action).
pub fn toggle_main_window(app: &AppHandle, was_hidden: bool) -> Result<(), String> {
    if was_hidden {
        show_main_window(app)
    } else {
        hide_main_window(app)
    }
}