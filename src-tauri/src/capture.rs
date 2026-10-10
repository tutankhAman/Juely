//! Screen capture — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `capture::{capture_to_base64, capture_selected_area,
//! pick_capture_region, start_screen_capture, close_overlay_window,
//! encode_png_for_ai, encode_jpeg_for_ai, CapturedMonitor, CaptureRegion,
//! SelectionCoords, MonitorGeom}` plus `capture_portal::{is_wayland,
//! screenshot_png}`. Upstream uses `xcap` on X11 and the XDG Screenshot portal
//! on Wayland. This reconstruction uses `xcap` and returns base64 PNG.

use std::sync::Mutex;

use base64::Engine;
use tauri::{Emitter, Manager};
use serde_json::{json, Value};

#[derive(Default)]
pub struct CaptureState {
    pub status: Mutex<String>,
    pub last_region: Mutex<Option<SelectionCoords>>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SelectionCoords {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

fn set_status(app: &tauri::AppHandle, s: &str) {

    if let Ok(mut g) = app.state::<CaptureState>().status.lock() {
        *g = s.to_string();
    }
}

/// Enumerate monitors as `{ id, name, x, y, width, height, scale }`.
pub fn list_monitors() -> Vec<Value> {
    match xcap::Monitor::all() {
        Ok(monitors) => monitors
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                json!({
                    "id": i,
                    "name": m.name(),
                    "x": m.x(),
                    "y": m.y(),
                    "width": m.width(),
                    "height": m.height(),
                    "scale": m.scale_factor()
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn encode_png(image: &image::RgbaImage) -> String {
    use image::ImageEncoder;
    let mut buf = Vec::new();
    let enc = image::codecs::png::PngEncoder::new(&mut buf);
    if enc
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .is_ok()
    {
        base64::engine::general_purpose::STANDARD.encode(buf)
    } else {
        String::new()
    }
}

/// `capture_to_base64 { region }` — full primary monitor as base64 PNG, or the
/// region subset when the frontend sends its saved autoRegion
/// (`{monitor,x,y,width,height}` from `ome()`).
pub fn capture_to_base64(app: &tauri::AppHandle, args: &Value) -> Result<Value, String> {
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors.into_iter().next().ok_or("no monitor found")?;
    let img = monitor.capture_image().map_err(|e| e.to_string())?;
    let region = args.get("region");
    let crop = region
        .and_then(|r| {
            let get = |k: &str| r.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
            let (rx, ry) = (get("x").max(0), get("y").max(0));
            let (rw, rh) = (get("width").max(0), get("height").max(0));
            (rw > 0 && rh > 0).then(|| (rx as u32, ry as u32, rw as u32, rh as u32))
        })
        .and_then(|(x, y, w, h)| {
            let (mw, mh) = (img.width(), img.height());
            let x = x.min(mw.saturating_sub(1));
            let y = y.min(mh.saturating_sub(1));
            let w = w.min(mw.saturating_sub(x)).max(1);
            let h = h.min(mh.saturating_sub(y)).max(1);
            Some(image::imageops::crop_imm(&img, x, y, w, h).to_image())
        });
    let final_img = crop.unwrap_or(img);
    set_status(app, "captured");
    Ok(Value::String(encode_png(&final_img)))
}

/// `capture_selected_area { coords, monitorIndex }`
pub fn capture_selected_area(app: &tauri::AppHandle, args: &Value) -> Result<Value, String> {
    let coords: SelectionCoords = args
        .get("coords")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(SelectionCoords {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
    let index = args.get("monitorIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors.get(index).or_else(|| monitors.first()).ok_or("no monitor")?;
    let full = monitor.capture_image().map_err(|e| e.to_string())?;

    let (w, h) = (full.width(), full.height());
    let x = coords.x.clamp(0, w as i32) as u32;
    let y = coords.y.clamp(0, h as i32) as u32;
    let cw = coords.width.min(w.saturating_sub(x)).max(1);
    let ch = coords.height.min(h.saturating_sub(y)).max(1);
    let cropped = image::imageops::crop_imm(&full, x, y, cw, ch).to_image();

    if let Ok(mut g) = app.state::<CaptureState>().last_region.lock() {
        *g = Some(coords);
    }
    set_status(app, "captured");
    // Upstream emits the cropped PNG to the main overlay via `captured-selection`
    // (binary string: "Failed to emit captured-selection event"); the overlay's
    // picker ignores the invoke result, so this event is the delivery channel.
    // The picked region also feeds the frontend's saved autoRegion
    // (`capture-region-picked` payload must expose {monitor,x,y,width,height}).
    let b64 = encode_png(&cropped);
    let _ = app.emit("captured-selection", b64.clone());
    let _ = app.emit(
        "capture-region-picked",
        json!({
            "monitor": index,
            "x": x,
            "y": y,
            "width": cw,
            "height": ch,
        }),
    );
    Ok(Value::String(b64))
}

/// `pick_capture_region` — returns monitor geometry for the selection overlay.
pub fn pick_capture_region(app: &tauri::AppHandle) -> Value {
    set_status(app, "picking");
    json!({ "monitors": list_monitors() })
}

/// `start_screen_capture` — enters selection mode.
pub fn start_screen_capture(app: &tauri::AppHandle) -> Value {
    set_status(app, "started");
    json!({ "status": "started", "monitors": list_monitors() })
}

/// `get_capture_status`
pub fn get_capture_status(app: &tauri::AppHandle) -> Value {

    let s = app
        .state::<CaptureState>()
        .status
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "idle".to_string());
    Value::String(s)
}

/// `enable_text_capture` / `disable_text_capture`
pub fn set_text_capture(app: &tauri::AppHandle, enabled: bool) -> Value {
    set_status(app, if enabled { "text-capture-on" } else { "text-capture-off" });
    Value::Null
}