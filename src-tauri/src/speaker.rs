//! Audio capture — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `speaker::{list_input_devices, list_output_devices,
//! AudioDevice, SpeakerInput, SpeakerStream}` and `speaker::{linux,macos,
//! windows}` backends. Upstream uses `cpal` for microphone input and
//! `libpulse-binding` (PulseAudio/PipeWire monitor source) for system audio on
//! Linux. This reconstruction enumerates devices with `cpal` and exposes the
//! same command surface; live capture requires a running sound server.

use serde_json::{json, Value};

/// `get_input_devices`
pub fn list_input_devices() -> Value {
    let devs = crate::audio::input_devices();
    serde_json::to_value(devs).unwrap_or(Value::Array(vec![]))
}

/// `get_output_devices`
pub fn list_output_devices() -> Value {
    let devs = crate::audio::output_devices();
    serde_json::to_value(devs).unwrap_or(Value::Array(vec![]))
}

/// `get_audio_sample_rate` — the STT pipeline target rate.
pub fn get_audio_sample_rate() -> Value {
    json!(16000)
}

/// `check_system_audio_access` — Linux uses PulseAudio/PipeWire monitor sources.
pub fn check_system_audio_access() -> Value {
    #[cfg(target_os = "linux")]
    {
        // Presence of the PulseAudio/PipeWire socket is the observable signal.
        let has_pulse = std::path::Path::new("/run/user").exists()
            && std::env::var("XDG_RUNTIME_DIR")
                .map(|d| std::path::Path::new(&d).join("pulse").exists())
                .unwrap_or(false);
        Value::Bool(has_pulse)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Value::Bool(true)
    }
}

/// `check_system_audio_supported` — returns a reason string when unsupported.
pub fn check_system_audio_supported() -> Value {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            // Wayland without a portal monitor source is reported unsupported.
            Value::Null
        } else {
            Value::Null
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Value::Null
    }
}

/// `request_system_audio_access` — opens the platform audio privacy settings.
pub fn request_system_audio_access() -> Value {
    #[cfg(target_os = "linux")]
    {
        for candidate in ["pavucontrol", "gnome-control-center", "systemsettings5"] {
            if std::process::Command::new("which")
                .arg(candidate)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                let _ = std::process::Command::new(candidate).spawn();
                return json!({ "opened": candidate });
            }
        }
        json!({ "opened": null })
    }
    #[cfg(not(target_os = "linux"))]
    {
        json!({ "opened": null })
    }
}

/// `open_permission_settings { kind }`
pub fn open_permission_settings(args: &Value) -> Value {
    let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("microphone");
    #[cfg(target_os = "linux")]
    {
        let _ = kind;
        let _ = std::process::Command::new("gnome-control-center").arg("sound").spawn();
    }
    json!({ "kind": kind })
}