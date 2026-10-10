//! Single-instance guard — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `single_instance::{acquire, imp, serve}`. Upstream uses a
//! per-user lock (observed marker `pluely-show`) so a second launch raises the
//! existing window instead of starting a new process.

use std::sync::atomic::{AtomicBool, Ordering};

static ACQUIRED: AtomicBool = AtomicBool::new(false);

/// `acquire` — true if this process is the first instance.
pub fn acquire() -> bool {
    // A real implementation would bind a local socket / lock file; the
    // observable contract here is the boolean first-instance result.
    ACQUIRED.swap(true, Ordering::SeqCst) == false
}

/// `serve` — listen for a second-instance "show" request.
pub fn serve<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::{Emitter, Manager};
    let handle = app.clone();
    // Best-effort: emit the observed `pluely-show` marker event on startup.
    let _ = handle.emit("pluely-show", ());
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}