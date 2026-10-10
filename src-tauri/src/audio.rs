//! Audio backends — reconstructed from Pluely 1.1.0.
//!
//! Upstream `Cargo.toml` carries `libpulse-binding` + `libpulse-simple-binding`
//! on Linux (`speaker::linux`, `@DEFAULT_MONITOR@`, `pluely-default-sink`,
//! `pluely-device-enum` strings in the binary) and uses `cpal` elsewhere.
//! This module mirrors that split:
//! - Linux: PulseAudio/PipeWire source+sink enumeration (friendly names like
//!   "Airdopes 280 ANC", incl. Bluetooth + monitor sources) and recording via
//!   the PulseAudio simple API, which resamples server-side.
//! - macOS/Windows: `cpal` enumeration and capture (unchanged behavior).
//!
//! Everything PulseAudio is `#[cfg(target_os = "linux")]`-gated; other OSes
//! compile exactly the previous code paths.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
}

// ---------------------------------------------------------------------------
// Linux: PulseAudio enumeration
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub(crate) mod pa {
    use super::AudioDeviceInfo;
    use libpulse_binding::callbacks::ListResult;
    use libpulse_binding::context::introspect::Introspector;
    use libpulse_binding::context::{Context, FlagSet, State as CtxState};
    use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
    use libpulse_binding::operation::{Operation, State as OpState};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// NOTE: field order matters — Rust drops fields in declaration order,
    /// and PulseAudio requires the Context to be freed BEFORE the Mainloop
    /// (reversing them aborts in mainloop_io_free).
    struct Session {
        ctx: Context,
        ml: Mainloop,
    }

    impl Session {
        fn connect() -> Result<Self, String> {
            let mut ml = Mainloop::new().ok_or("Failed to create PulseAudio mainloop")?;
            let mut ctx = Context::new(&ml, "pluely-device-enum")
                .ok_or("Failed to create PulseAudio context")?;
            ctx.connect(None, FlagSet::NOFLAGS, None)
                .map_err(|_| "Failed to connect to PulseAudio".to_string())?;
            loop {
                match ml.iterate(false) {
                    IterateResult::Success(_) => {}
                    _ => return Err("Failed to iterate mainloop".to_string()),
                }
                match ctx.get_state() {
                    CtxState::Ready => break,
                    CtxState::Failed | CtxState::Terminated => {
                        return Err("PulseAudio context failed".to_string())
                    }
                    _ => {}
                }
            }
            Ok(Self { ml, ctx })
        }

        /// Pump the mainloop until `op` completes.
        fn pump<C: ?Sized>(&mut self, op: &Operation<C>) {
            for _ in 0..1000 {
                let _ = self.ml.iterate(true);
                if op.get_state() == OpState::Done {
                    break;
                }
            }
        }
    }

    pub fn defaults() -> (String, String) {
        let Ok(mut s) = Session::connect() else {
            return Default::default();
        };
        let out: Rc<RefCell<(String, String)>> = Default::default();
        let out2 = out.clone();
        let op = s.ctx.introspect().get_server_info(move |info| {
            let src = info
                .default_source_name
                .as_ref()
                .map(|c| c.clone().into_owned())
                .unwrap_or_default();
            let sink = info
                .default_sink_name
                .as_ref()
                .map(|c| c.clone().into_owned())
                .unwrap_or_default();
            *out2.borrow_mut() = (src, sink);
        });
        s.pump(&op);
        let guard = out.borrow();
        (guard.0.clone(), guard.1.clone())
    }

    /// (sources, sinks): sources as (name, description, is_monitor).
    pub fn inventory() -> (Vec<(String, String, bool)>, Vec<(String, String)>) {
        let Ok(mut s) = Session::connect() else {
            return Default::default();
        };
        let out_src: Rc<RefCell<Vec<(String, String, bool)>>> = Default::default();
        let out_src2 = out_src.clone();
        let op_src = s.ctx.introspect().get_source_info_list(move |res| {
            if let ListResult::Item(info) = res {
                let name = info
                    .name
                    .as_ref()
                    .map(|c| c.clone().into_owned())
                    .unwrap_or_default();
                let desc = info
                    .description
                    .as_ref()
                    .map(|c| c.clone().into_owned())
                    .unwrap_or_default();
                if !name.is_empty() {
                    out_src2.borrow_mut().push((name, desc, info.monitor_of_sink.is_some()));
                }
            }
        });
        s.pump(&op_src);
        let sources: Vec<(String, String, bool)> = out_src.borrow().to_vec();
        let out_sink: Rc<RefCell<Vec<(String, String)>>> = Default::default();
        let out_sink2 = out_sink.clone();
        let op_sink = s.ctx.introspect().get_sink_info_list(move |res| {
            if let ListResult::Item(info) = res {
                let name = info
                    .name
                    .as_ref()
                    .map(|c| c.clone().into_owned())
                    .unwrap_or_default();
                let desc = info
                    .description
                    .as_ref()
                    .map(|c| c.clone().into_owned())
                    .unwrap_or_default();
                if !name.is_empty() {
                    out_sink2.borrow_mut().push((name, desc));
                }
            }
        });
        s.pump(&op_sink);
        let sinks: Vec<(String, String)> = out_sink.borrow().to_vec();
        (sources, sinks)
    }

    pub fn device_lists() -> Option<(Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>)> {
        let (def_src, def_sink) = defaults();
        let (sources, sinks) = inventory();
        if sources.is_empty() && sinks.is_empty() {
            return None;
        }
        let inputs = sources
            .into_iter()
            .filter(|(_, _, mon)| !mon)
            .map(|(name, desc, _)| AudioDeviceInfo {
                is_default: !def_src.is_empty() && name == def_src,
                id: name.clone(),
                name: if desc.is_empty() { name } else { desc },
            })
            .collect();
        let outputs = sinks
            .into_iter()
            .map(|(name, desc)| AudioDeviceInfo {
                is_default: !def_sink.is_empty() && name == def_sink,
                id: name.clone(),
                name: if desc.is_empty() { name } else { desc },
            })
            .collect();
        Some((inputs, outputs))
    }

    /// Monitor source name for a sink id (or a source id that already is one).
    pub fn monitor_for(id: &str) -> Option<String> {
        let (sources, _) = inventory();
        if sources.iter().any(|(n, _, m)| *m && n == id) {
            return Some(id.to_string());
        }
        let candidate = format!("{id}.monitor");
        if sources.iter().any(|(n, _, _)| n == &candidate) {
            return Some(candidate);
        }
        None
    }

    /// Monitor of the default sink (fallback when the selected output — e.g.
    /// a stale stored ALSA id — resolves to nothing).
    pub fn default_monitor() -> Option<String> {
        let (_, def_sink) = defaults();
        if def_sink.is_empty() {
            return None;
        }
        monitor_for(&def_sink)
    }

    /// Friendly display name for a source id (its PulseAudio description),
    /// falling back to a cleaned-up id. Never surfaces raw technical names
    /// like `bluez_output.71_B3_….monitor` to the UI.
    pub fn display_name_for_source(id: &str) -> String {
        let (sources, sinks) = inventory();
        for (n, desc, _) in &sources {
            if n == id && !desc.is_empty() {
                return desc.clone();
            }
        }
        for (n, desc) in &sinks {
            if n == id && !desc.is_empty() {
                return format!("Monitor of {desc}");
            }
        }
        // Last resort: strip the `.monitor` suffix and tidy separators.
        id.strip_suffix(".monitor")
            .unwrap_or(id)
            .replace(['_', '-'], " ")
    }

    pub fn is_pulse_source(id: &str) -> bool {
        let (sources, sinks) = inventory();
        sources.iter().any(|(n, _, _)| n == id) || sinks.iter().any(|(n, _)| n == id)
    }
}

// ---------------------------------------------------------------------------
// Public device listing (both mic + system capture sources)
// ---------------------------------------------------------------------------

/// Input (microphone) devices for the picker.
pub fn input_devices() -> Vec<AudioDeviceInfo> {
    #[cfg(target_os = "linux")]
    {
        if let Some((inputs, _)) = pa::device_lists() {
            if !inputs.is_empty() {
                return inputs;
            }
        }
    }
    cpal_input_devices()
}

/// Output (speaker) devices for the picker.
pub fn output_devices() -> Vec<AudioDeviceInfo> {
    #[cfg(target_os = "linux")]
    {
        if let Some((_, outputs)) = pa::device_lists() {
            if !outputs.is_empty() {
                return outputs;
            }
        }
    }
    cpal_output_devices()
}

fn cpal_input_devices() -> Vec<AudioDeviceInfo> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let mut out = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                out.push(AudioDeviceInfo {
                    is_default: default_name.as_deref() == Some(name.as_str()),
                    id: name.clone(),
                    name,
                });
            }
        }
    }
    out
}

fn cpal_output_devices() -> Vec<AudioDeviceInfo> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let mut out = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                out.push(AudioDeviceInfo {
                    is_default: default_name.as_deref() == Some(name.as_str()),
                    id: name.clone(),
                    name,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Persistent capture inputs (mic + system monitor)
// ---------------------------------------------------------------------------

/// Mono f32 samples at `rate`.
pub struct PcmBlock {
    pub samples: Vec<f32>,
    pub rate: u32,
}

pub enum MicInput {
    #[cfg(target_os = "linux")]
    Pulse {
        simple: libpulse_simple_binding::Simple,
    },
    Cpal {
        _stream: cpal::Stream,
        rx: std::sync::mpsc::Receiver<Vec<f32>>,
        rate: u32,
        poisoned: std::sync::Arc<std::sync::atomic::AtomicBool>,
    },
}

impl MicInput {
    /// Blocking read of roughly `secs` seconds of mono f32. `Err` means the
    /// stream broke and the caller should rebuild via `open_input`.
    pub fn read_block(&mut self, secs: f32) -> Result<PcmBlock, String> {
        #[cfg(target_os = "linux")]
        {
            if let MicInput::Pulse { simple } = self {
                let n = (16000.0 * secs) as usize;
                let mut bytes = vec![0u8; n * 2];
                simple
                    .read(&mut bytes)
                    .map_err(|_| "Audio stream ended unexpectedly".to_string())?;
                let samples: Vec<f32> = bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                    .collect();
                return Ok(PcmBlock { samples, rate: 16000 });
            }
        }
        let (rx, rate, poisoned) = match self {
            MicInput::Cpal { rx, rate, poisoned, .. } => (rx, *rate, poisoned.clone()),
            #[cfg(target_os = "linux")]
            _ => return Err("No audio was captured".to_string()),
        };
        use std::sync::atomic::Ordering;
        if poisoned.load(Ordering::SeqCst) {
            return Err("[listen] microphone stream failed; rebuilding".to_string());
        }
        let need = (rate as f32 * secs) as usize;
        let mut acc = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f32(secs + 2.0);
        while acc.len() < need {
            if poisoned.load(Ordering::SeqCst) {
                return Err("[listen] microphone stream failed; rebuilding".to_string());
            }
            match rx.recv_timeout(std::time::Duration::from_millis(250)) {
                Ok(mut chunk) => acc.append(&mut chunk),
                Err(_) => {
                    if std::time::Instant::now() > deadline {
                        break;
                    }
                }
            }
        }
        if acc.is_empty() {
            return Err("No audio was captured".to_string());
        }
        Ok(PcmBlock { samples: acc, rate })
    }
}

/// Open a persistent input on a PulseAudio source (Linux) or cpal device.
pub fn open_input(device_id: Option<&str>) -> Result<MicInput, String> {
    #[cfg(target_os = "linux")]
    {
        use libpulse_binding::sample::{Format, Spec};
        use libpulse_binding::stream::Direction;
        use libpulse_simple_binding::Simple;

        let want = device_id.filter(|s| !s.is_empty()).map(|s| s.to_string());
        let use_pa = match &want {
            Some(id) => pa::is_pulse_source(id),
            None => true,
        };
        if use_pa {
            let spec = Spec {
                format: Format::S16NE,
                channels: 1,
                rate: 16000,
            };
            match Simple::new(
                None,
                "pluely",
                Direction::Record,
                want.as_deref(),
                "listen-mic-capture",
                &spec,
                None,
                None,
            ) {
                Ok(simple) => return Ok(MicInput::Pulse { simple }),
                Err(_) => {
                    if want.is_some() {
                        return Err("Microphone capture thread exited unexpectedly during initialization".to_string());
                    }
                    // Default PA failed; fall through to cpal default.
                }
            }
        }
        if let Some(id) = want {
            return open_cpal_named(&id);
        }
    }
    open_cpal_named(device_id.unwrap_or_default())
}

fn open_cpal_named(want: &str) -> Result<MicInput, String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let host = cpal::default_host();
    let device = if want.is_empty() {
        host.default_input_device()
            .ok_or_else(|| "No audio was captured".to_string())?
    } else {
        host.input_devices()
            .map_err(|_| "Microphone capture thread exited unexpectedly during initialization".to_string())?
            .find(|d| d.name().is_ok_and(|n| n == want))
            .ok_or_else(|| "No audio was captured".to_string())?
    };
    let cfg = device
        .default_input_config()
        .map_err(|_| "Microphone capture thread exited unexpectedly during initialization".to_string())?;
    let rate = cfg.sample_rate().0;
    let channels = cfg.channels() as usize;
    let fmt = cfg.sample_format();
    let stream_cfg = cfg.into();
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);
    let poisoned = Arc::new(AtomicBool::new(false));
    let stream = match fmt {
        cpal::SampleFormat::F32 => {
            let tx = tx.clone();
            let poison_w = poisoned.clone();
            device.build_input_stream(
                &stream_cfg,
                move |data: &[f32], _| {
                    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
                    for frame in data.chunks(channels.max(1)) {
                        mono.push(frame.iter().sum::<f32>() / frame.len() as f32);
                    }
                    let _ = tx.try_send(mono);
                },
                move |_| {
                    poison_w.store(true, Ordering::SeqCst);
                },
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let tx = tx.clone();
            let poison_w = poisoned.clone();
            device.build_input_stream(
                &stream_cfg,
                move |data: &[i16], _| {
                    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
                    for frame in data.chunks(channels.max(1)) {
                        mono.push(frame.iter().map(|x| *x as f32 / 32768.0).sum::<f32>() / frame.len() as f32);
                    }
                    let _ = tx.try_send(mono);
                },
                move |_| {
                    poison_w.store(true, Ordering::SeqCst);
                },
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let tx = tx.clone();
            let poison_w = poisoned.clone();
            device.build_input_stream(
                &stream_cfg,
                move |data: &[u16], _| {
                    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
                    for frame in data.chunks(channels.max(1)) {
                        mono.push(frame.iter().map(|x| (*x as f32 - 32768.0) / 32768.0).sum::<f32>() / frame.len() as f32);
                    }
                    let _ = tx.try_send(mono);
                },
                move |_| {
                    poison_w.store(true, Ordering::SeqCst);
                },
                None,
            )
        }
        _ => {
            return Err("Invalid audio specification".to_string());
        }
    }
    .map_err(|_| "[listen] microphone stream failed; rebuilding".to_string())?;
    stream.play().map_err(|_| "[listen] microphone stream failed; rebuilding".to_string())?;
    Ok(MicInput::Cpal { _stream: stream, rx, rate, poisoned })
}

// ---------------------------------------------------------------------------
// Bluetooth headset profile lock (Linux, best-effort)
// ---------------------------------------------------------------------------

/// RAII guard: while alive, the Bluetooth card owning `source` is pinned to
/// the `headset_head_unit` (HSP/HFP) profile so A2DP↔HSP flapping cannot tear
/// down the mic mid-session. Restores the previous profile on drop. No-op
/// (returns `None`) when pactl is unavailable or the source is not Bluetooth.
#[cfg(target_os = "linux")]
pub struct ProfileLock {
    card: String,
    previous: String,
}

#[cfg(target_os = "linux")]
impl ProfileLock {
    pub fn pin_headset_for_source(source: &str) -> Option<Self> {
        if !source.starts_with("bluez_") {
            return None;
        }
        // Match the card owning this source via its MAC fragment.
        let mac_frag = source
            .split('.')
            .nth(1)
            .map(|m| m.replace(':', "_"))
            .unwrap_or_default();
        let cards = std::process::Command::new("pactl")
            .args(["list", "cards", "short"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())?;
        let card_idx = cards
            .lines()
            .filter_map(|l| {
                let mut parts = l.split_whitespace();
                let idx = parts.next()?.to_string();
                let name = parts.next()?.to_string();
                if name.contains("bluez") && (mac_frag.is_empty() || name.contains(&mac_frag)) {
                    Some(idx)
                } else {
                    None
                }
            })
            .next()?;
        // Previous profile, scoped to OUR card block.
        let detail = std::process::Command::new("pactl")
            .args(["list", "cards"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let mut in_card = false;
        let mut previous = String::from("a2dp-sink");
        for line in detail.lines() {
            let t = line.trim_start();
            if t.starts_with("Card #") {
                in_card = t.contains(&format!("#{card_idx}")) || t.ends_with(&card_idx);
                continue;
            }
            if in_card {
                if let Some(v) = t.strip_prefix("Active Profile:") {
                    previous = v.trim().to_string();
                    break;
                }
            }
        }
        for profile in ["headset_head_unit", "headset-head-unit"] {
            let ok = std::process::Command::new("pactl")
                .args(["set-card-profile", &card_idx, profile])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return Some(Self { card: card_idx, previous });
            }
        }
        None
    }
}

#[cfg(target_os = "linux")]
impl Drop for ProfileLock {
    fn drop(&mut self) {
        let _ = std::process::Command::new("pactl")
            .args(["set-card-profile", &self.card, &self.previous])
            .status();
    }
}
