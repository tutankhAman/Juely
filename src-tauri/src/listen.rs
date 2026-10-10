//! Live listen + push-to-talk + VAD — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `listen::{mic, mic_linux, system, aec, resample, ws,
//! local, commands, ListenSession, ListenState, PttSession}` and the VAD config
//! fields `sensitivity_rms, peak_threshold, silence_chunks, min_speech_chunks,
//! pre_speech_chunks, noise_gate_threshold, max_recording_duration_secs`.
//! Frontend protocol (observed):
//! - `listen_start { inputDevice, outputDevice, language, ownKey, ownModel }`
//!   throws plain strings: "A listen session is already running",
//!   "no license … activate your license", "own speech key …".
//! - transcripts arrive on `listen-transcript` as
//!   `{ text, isFinal, channel (0=me,1=them), speaker, start }`;
//!   state on `listen-status` as `{ state, message }` with
//!   `listening|connecting|paused|stopped|exhausted|error`.
//! - budget on `listen-budget`, notices on `listen-notice`.
//!
//! Capture model (mirrors upstream `listen-mic-capture`): ONE persistent input
//! per channel for the whole session, rebuilt on failure — never reopened per
//! chunk. Transcription is chunked Deepgram REST over the caller's own key
//! (upstream `wss://…/v1/listen` streaming documented as future work).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{self, MicInput};
use crate::speaker;

static LISTEN_GEN: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub sensitivity_rms: f32,
    pub peak_threshold: f32,
    pub silence_chunks: u32,
    pub min_speech_chunks: u32,
    pub pre_speech_chunks: u32,
    pub noise_gate_threshold: f32,
    pub max_recording_duration_secs: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sensitivity_rms: 0.02,
            peak_threshold: 0.1,
            silence_chunks: 8,
            min_speech_chunks: 2,
            pre_speech_chunks: 4,
            noise_gate_threshold: 0.01,
            max_recording_duration_secs: 300,
        }
    }
}

#[derive(Default)]
pub struct ListenState {
    pub vad: Mutex<VadConfig>,
    pub active: Mutex<bool>,
    pub paused: Mutex<bool>,
    pub status: Mutex<String>,
    pub ptt: Mutex<bool>,
}

fn set_status(app: &AppHandle, state: &str, message: Option<&str>) {
    if let Ok(mut g) = app.state::<ListenState>().status.lock() {
        *g = state.to_string();
    }
    let _ = app.emit(
        "listen-status",
        json!({ "state": state, "message": message }),
    );
}

fn session_alive(app: &AppHandle, gen: u64) -> bool {
    LISTEN_GEN.load(Ordering::SeqCst) == gen
        && app.state::<ListenState>().active.lock().map(|g| *g).unwrap_or(false)
}

fn paused(app: &AppHandle) -> bool {
    app.state::<ListenState>().paused.lock().map(|g| *g).unwrap_or(false)
}

/// `get_vad_config`
pub fn get_vad_config(app: &AppHandle) -> Value {
    let cfg = app
        .state::<ListenState>()
        .vad
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    serde_json::to_value(cfg).unwrap_or(Value::Null)
}

/// `update_vad_config { config }`
pub fn update_vad_config(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let incoming: VadConfig = args
        .get("config")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    if !(0.0..=1.0).contains(&incoming.sensitivity_rms) {
        return Err("Invalid sensitivity_rms: must be 0.0-1.0".into());
    }
    if incoming.max_recording_duration_secs > 3600 {
        return Err("Invalid max_recording_duration_secs: must be <= 3600 (1 hour)".into());
    }
    if let Ok(mut g) = app.state::<ListenState>().vad.lock() {
        *g = incoming;
    }
    Ok(Value::Null)
}

/// `get_listen_languages`
pub fn get_listen_languages() -> Value {
    let langs = [
        ("en", "English"), ("es", "Spanish"), ("fr", "French"), ("de", "German"),
        ("it", "Italian"), ("pt", "Portuguese"), ("nl", "Dutch"), ("ru", "Russian"),
        ("zh", "Chinese"), ("ja", "Japanese"), ("ko", "Korean"), ("ar", "Arabic"),
        ("tr", "Turkish"), ("pl", "Polish"), ("sv", "Swedish"), ("no", "Norwegian"),
        ("da", "Danish"), ("fi", "Finnish"), ("hi", "Hindi"), ("id", "Indonesian"),
        ("th", "Thai"), ("vi", "Vietnamese"), ("uk", "Ukrainian"), ("cs", "Czech"),
        ("el", "Greek"), ("he", "Hebrew"), ("hu", "Hungarian"), ("ms", "Malay"),
        ("ro", "Romanian"), ("fil", "Filipino"),
    ];
    let languages: Vec<Value> = langs
        .iter()
        .map(|(code, name)| json!({ "code": code, "name": name }))
        .collect();
    json!({ "success": true, "languages": languages, "multilingual": true })
}

fn is_licensed(app: &AppHandle) -> bool {
    app.state::<crate::shortcuts::LicenseState>()
        .0
        .lock()
        .map(|g| *g)
        .unwrap_or(false)
}

fn wav_bytes(samples_16k_mono: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + samples_16k_mono.len() * 2);
    let data_len = (samples_16k_mono.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&16000u32.to_le_bytes());
    out.extend_from_slice(&32000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples_16k_mono {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

fn resample_to_16k_i16(samples: &[f32], rate: u32) -> Vec<i16> {
    if rate == 16000 {
        return samples.iter().map(|x| (x.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
    }
    let ratio = 16000.0 / rate as f32;
    let out_len = (samples.len() as f32 * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f32 / ratio;
        let i0 = pos.floor() as usize;
        let frac = pos - i0 as f32;
        let a = samples.get(i0).copied().unwrap_or(0.0);
        let b = samples.get(i0 + 1).copied().unwrap_or(a);
        out.push(((a + (b - a) * frac).clamp(-1.0, 1.0) * 32767.0) as i16);
    }
    out
}

fn deepgram_transcribe(api_key: &str, model: &str, language: &str, wav: Vec<u8>) -> Result<Vec<(String, i64)>, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Could not reach the transcription service".to_string())?;
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.deepgram.com/v1/listen?model={}&language={}&diarize=true&smart_format=true",
            model, language
        );
        let resp = client
            .post(&url)
            .header("Authorization", format!("Token {api_key}"))
            .header("Content-Type", "audio/wav")
            .body(wav)
            .send()
            .await
            .map_err(|_| "Could not reach the transcription service".to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your speech provider rejected the API key. Check it under Dashboard › Dev Space.".to_string());
        }
        if !resp.status().is_success() {
            return Err("Could not reach the transcription service".to_string());
        }
        let v: Value = resp.json().await.map_err(|_| "Could not reach the transcription service".to_string())?;
        let mut segs = Vec::new();
        if let Some(alts) = v.pointer("/results/channels/0/alternatives").and_then(|a| a.as_array()) {
            if let Some(alt) = alts.first() {
                let mut cur_speaker: i64 = 0;
                let mut cur_text = String::new();
                let mut flush = |segs: &mut Vec<(String, i64)>, text: &mut String, sp: i64| {
                    let t = text.trim().to_string();
                    if !t.is_empty() {
                        segs.push((t, sp));
                    }
                    text.clear();
                };
                if let Some(words) = alt.get("words").and_then(|w| w.as_array()) {
                    for w in words {
                        let word = w.get("punctuated_word").or_else(|| w.get("word")).and_then(|x| x.as_str()).unwrap_or("");
                        let sp = w.get("speaker").and_then(|x| x.as_i64()).unwrap_or(0);
                        if sp != cur_speaker && !cur_text.trim().is_empty() {
                            flush(&mut segs, &mut cur_text, cur_speaker);
                            cur_speaker = sp;
                        }
                        if !cur_text.is_empty() {
                            cur_text.push(' ');
                        }
                        cur_text.push_str(word);
                    }
                    flush(&mut segs, &mut cur_text, cur_speaker);
                } else if let Some(t) = alt.get("transcript").and_then(|x| x.as_str()) {
                    if !t.trim().is_empty() {
                        segs.push((t.trim().to_string(), 0));
                    }
                }
            }
        }
        Ok(segs)
    })
}

/// VAD utterance segmenter: accumulates speech until trailing silence, so
/// transcriptions follow sentence boundaries instead of fixed windows.
struct Segmenter {
    vad: VadConfig,
    acc: Vec<f32>,
    rate: u32,
    preroll: std::collections::VecDeque<f32>,
    speaking: bool,
    silence_chunks: u32,
    speech_chunks: u32,
}

impl Segmenter {
    fn new(vad: VadConfig) -> Self {
        Self {
            vad,
            acc: Vec::new(),
            rate: 16000,
            preroll: std::collections::VecDeque::new(),
            speaking: false,
            silence_chunks: 0,
            speech_chunks: 0,
        }
    }

    /// Feed one ~0.25 s mono block at `rate`. Returns a complete 16 kHz i16
    /// utterance when trailing silence (or the length cap) ends it.
    fn push(&mut self, block: &[f32], rate: u32) -> Option<Vec<i16>> {
        self.rate = rate;
        // Keep ~1 s of pre-roll for onset backfill (bounded).
        let keep = (rate as usize).min(48000);
        for s in block {
            self.preroll.push_back(*s);
        }
        while self.preroll.len() > keep {
            self.preroll.pop_front();
        }

        let level = rms(block);
        let onset = level >= self.vad.sensitivity_rms.max(0.001);
        let silent = level < self.vad.noise_gate_threshold.max(0.001);

        if !self.speaking {
            if onset {
                self.speaking = true;
                self.silence_chunks = 0;
                self.speech_chunks = 1;
                // Backfill up to pre_speech_chunks*0.25 s so word onsets survive.
                let backfill = (self.vad.pre_speech_chunks as usize * rate as usize / 4).min(self.preroll.len());
                self.acc.clear();
                self.acc.extend(self.preroll.iter().rev().take(backfill).rev().copied());
                self.acc.extend_from_slice(block);
            }
            return None;
        }

        self.acc.extend_from_slice(block);
        if onset {
            self.speech_chunks += 1;
        }
        if silent {
            self.silence_chunks += 1;
        } else {
            self.silence_chunks = 0;
        }

        let cap_samples = (self.vad.max_recording_duration_secs.min(120) as usize)
            .max(10)
            * rate as usize;
        let ended_by_silence =
            self.silence_chunks >= self.vad.silence_chunks.max(1) && self.speech_chunks >= self.vad.min_speech_chunks.max(1);
        if ended_by_silence || self.acc.len() >= cap_samples {
            self.speaking = false;
            self.silence_chunks = 0;
            self.speech_chunks = 0;
            let acc = std::mem::take(&mut self.acc);
            if rms(&acc) < self.vad.noise_gate_threshold {
                return None;
            }
            return Some(resample_to_16k_i16(&acc, self.rate));
        }
        None
    }
}

/// Reopen a broken input (same device), with backoff. Returns None on stop.
fn rebuild_input(
    app: &AppHandle,
    gen: u64,
    input: &mut MicInput,
    device_id: Option<String>,
    channel: &str,
) -> bool {
    let _ = app.emit(
        "listen-notice",
        json!({ "message": "[listen] microphone stream failed; rebuilding" }),
    );
    for attempt in 0..6 {
        if !session_alive(app, gen) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(400 * (attempt + 1) as u64));
        match audio::open_input(device_id.as_deref()) {
            Ok(fresh) => {
                *input = fresh;
                let _ = app.emit(
                    "listen-notice",
                    json!({ "message": format!("[{channel}] audio stream rebuilt") }),
                );
                return true;
            }
            Err(_) => continue,
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn channel_loop(
    app: AppHandle,
    gen: u64,
    device_id: Option<String>,
    channel: &'static str,
    api_key: String,
    model: String,
    language: String,
    _gate: f32,
    fatal: bool,
) {
    // Opened here (not moved across threads): audio handles are !Send.
    let mut input = match audio::open_input(device_id.as_deref()) {
        Ok(i) => i,
        Err(e) => {
            if fatal {
                set_status(&app, "error", Some(&e));
            } else {
                let _ = app.emit("listen-notice", json!({ "message": e }));
            }
            return;
        }
    };
    let mut counter: u64 = 0;
    let vad = app
        .state::<ListenState>()
        .vad
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let mut seg = Segmenter::new(vad);
    // Shared level meter state: `/me` channel publishes mic level, `/them`
    // channel adds system level.
    let levels = std::sync::Arc::new((Mutex::new(0.0f32), Mutex::new(0.0f32)));
    let level_this = levels.clone();
    let level_other = levels.clone();
    let is_mic = channel == "me";
    let app_level = app.clone();
    std::thread::spawn(move || {
        // Level beacon ~4 Hz: `listen-level { mic, system }` feeds the
        // overlay's meter bars (frontend scales sqrt(level) * 2.2).
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let mic = if is_mic { level_other.0.lock().map(|g| *g).unwrap_or(0.0) } else { 0.0 };
            let system = if is_mic {
                0.0
            } else {
                level_other.1.lock().map(|g| *g).unwrap_or(0.0)
            };
            let _ = app_level.emit("listen-level", json!({ "mic": mic, "system": system }));
            if !session_alive(&app_level, gen) {
                break;
            }
        }
    });
    loop {
        if !session_alive(&app, gen) {
            break;
        }
        if paused(&app) {
            std::thread::sleep(std::time::Duration::from_millis(200));
            continue;
        }
        // Short slices: prompt stop/pause reaction + fine VAD edges.
        let pcm = match input.read_block(0.25) {
            Err(e) if e == "stopped" => break,
            Err(_) => {
                if !rebuild_input(&app, gen, &mut input, device_id.clone(), channel) {
                    if fatal {
                        set_status(&app, "error", Some("Audio input stopped unexpectedly. Please try again."));
                    }
                    break;
                }
                continue;
            }
            Ok(block) => {
                // Record this channel's level for the meter beacon.
                let lv = rms(&block.samples);
                if let Ok(mut g) = level_this.0.lock() {
                    *g = lv;
                }
                match seg.push(&block.samples, block.rate) {
                    Some(utterance) => utterance,
                    None => continue,
                }
            }
        };
        match deepgram_transcribe(&api_key, &model, &language, wav_bytes(&pcm)) {
            Ok(segs) => {
                for (text, speaker) in segs {
                    counter += 1;
                    let _ = app.emit(
                        "listen-transcript",
                        json!({
                            "text": text,
                            "isFinal": true,
                            "channel": if channel == "me" { 0 } else { 1 },
                            "speaker": speaker,
                            "start": counter
                        }),
                    );
                }
            }
            Err(e) => {
                set_status(&app, "error", Some(&e));
                break;
            }
        }
    }
}

/// `listen_start { inputDevice, outputDevice, language, ownKey, ownModel }`
pub fn listen_start(app: &AppHandle, args: &Value) -> Result<Value, String> {
    if app.state::<ListenState>().active.lock().map(|g| *g).unwrap_or(false) {
        return Err("A listen session is already running".to_string());
    }
    let own_key = args.get("ownKey").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let own_model = args.get("ownModel").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()).unwrap_or_else(|| "nova-3".to_string());
    let language = args.get("language").and_then(|v| v.as_str()).unwrap_or("en").to_string();
    let input_device = args.get("inputDevice").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let output_device = args.get("outputDevice").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    if own_key.is_none() && !is_licensed(app) {
        return Err("no license found — activate your license".to_string());
    }

    let gen = LISTEN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut a) = app.state::<ListenState>().active.lock() {
        *a = true;
    }
    let gate = app.state::<ListenState>().vad.lock().map(|g| g.noise_gate_threshold).unwrap_or(0.01);

    match own_key {
        Some(key) => {
            // Validate the mic upfront (fail fast). Session threads open
            // (and own) their persistent inputs: audio handles are !Send.
            audio::open_input(input_device.as_deref()).map_err(|e| {
                if let Ok(mut a) = app.state::<ListenState>().active.lock() {
                    *a = false;
                }
                e
            })?;

            // Pin Bluetooth headset profile for the session (Linux, best-effort).
            #[cfg(target_os = "linux")]
            let profile_guard = input_device
                .as_deref()
                .and_then(audio::ProfileLock::pin_headset_for_source);

            // System channel (best-effort): monitor of the selected output,
            // falling back to the default sink's monitor (stored device ids
            // from older builds may be stale ALSA names). Non-Linux: none.
            #[cfg(target_os = "linux")]
            let sys_monitor: Option<String> = output_device
                .as_deref()
                .and_then(|id| audio::pa::monitor_for(id))
                .or_else(audio::pa::default_monitor);
            #[cfg(not(target_os = "linux"))]
            let sys_monitor: Option<String> = None;
            #[cfg(not(target_os = "linux"))]
            let _ = &output_device;

            set_status(app, "connecting", None);
            let handle = app.clone();
            let (k2, m2, l2) = (key.clone(), own_model.clone(), language.clone());
            std::thread::spawn(move || {
                // Hold the profile guard for the session lifetime (Linux).
                #[cfg(target_os = "linux")]
                let _profile_guard = profile_guard;
                set_status(&handle, "listening", None);
                // Mic loop is fatal; system loop is best-effort.
                match sys_monitor {
                    Some(mon) => {
                        let h2 = handle.clone();
                        let (sk, sm, sl) = (k2.clone(), m2.clone(), l2.clone());
                        // Friendly name only — never the raw source id.
                        #[cfg(target_os = "linux")]
                        let mon_label = audio::pa::display_name_for_source(&mon);
                        #[cfg(not(target_os = "linux"))]
                        let mon_label = "system audio".to_string();
                        let _ = handle.emit(
                            "listen-notice",
                            json!({ "message": format!("capturing system audio from {mon_label}") }),
                        );
                        std::thread::spawn(move || {
                            channel_loop(h2, gen, Some(mon), "them", sk, sm, sl, gate, false);
                        });
                    }
                    None => {
                        let _ = handle.emit(
                            "listen-notice",
                            json!({ "message": "system audio unavailable — mic only" }),
                        );
                    }
                }
                let h3 = handle.clone();
                channel_loop(h3, gen, input_device, "me", k2, m2, l2, gate, true);
                // Session teardown (owned handle: safe to use here).
                if LISTEN_GEN.load(Ordering::SeqCst) == gen {
                    if let Ok(mut a) = handle.state::<ListenState>().active.lock() {
                        *a = false;
                    }
                }
                set_status(&handle, "stopped", None);
            });
            Ok(json!({ "status": "connecting" }))
        }
        None => {
            if let Ok(mut a) = app.state::<ListenState>().active.lock() {
                *a = false;
            }
            Err("live transcription service not configured".to_string())
        }
    }
}

/// `listen_start_local { inputDevice, outputDevice }`
///
/// Custom-STT path: the frontend transcribes each chunk itself with the
/// user's provider curl, so Rust emits `listen-local-chunk { wavBase64, start }`
/// per VAD utterance and lets the UI do the rest.
pub fn listen_start_local(app: &AppHandle, args: &Value) -> Value {
    use base64::Engine;
    if app.state::<ListenState>().active.lock().map(|g| *g).unwrap_or(false) {
        return json!({ "status": "already", "mode": "local" });
    }
    let input_device = args
        .get("inputDevice")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let gen = LISTEN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut a) = app.state::<ListenState>().active.lock() {
        *a = true;
    }
    let vad = app
        .state::<ListenState>()
        .vad
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    set_status(app, "listening", None);
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut input = match audio::open_input(input_device.as_deref()) {
            Ok(i) => i,
            Err(e) => {
                set_status(&handle, "error", Some(&e));
                return;
            }
        };
        let mut seg = Segmenter::new(vad);
        let mut counter: u64 = 0;
        loop {
            if !session_alive(&handle, gen) {
                break;
            }
            if paused(&handle) {
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }
            match input.read_block(0.25) {
                Err(e) if e == "stopped" => break,
                Err(_) => {
                    if !rebuild_input(&handle, gen, &mut input, input_device.clone(), "local") {
                        set_status(&handle, "error", Some("Audio input stopped unexpectedly. Please try again."));
                        break;
                    }
                    continue;
                }
                Ok(block) => {
                    if let Some(pcm) = seg.push(&block.samples, block.rate) {
                        counter += 1;
                        let wav = wav_bytes(&pcm);
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&wav);
                        let _ = handle.emit(
                            "listen-local-chunk",
                            json!({ "wavBase64": b64, "start": counter }),
                        );
                    }
                }
            }
        }
        set_status(&handle, "stopped", None);
    });
    json!({ "status": "listening", "mode": "local" })
}

/// `listen_stop`
pub fn listen_stop(app: &AppHandle) -> Value {
    LISTEN_GEN.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut a) = app.state::<ListenState>().active.lock() {
        *a = false;
    }
    set_status(app, "stopped", None);
    Value::Null
}

/// `listen_set_paused { paused }`
pub fn listen_set_paused(app: &AppHandle, args: &Value) -> Value {
    let paused = args.get("paused").and_then(|v| v.as_bool()).unwrap_or(false);
    if let Ok(mut p) = app.state::<ListenState>().paused.lock() {
        *p = paused;
    }
    set_status(app, if paused { "paused" } else { "listening" }, None);
    Value::Null
}

/// `start_system_audio_capture`
pub fn start_system_audio_capture(app: &AppHandle) -> Value {
    set_status(app, "capturing", None);
    json!({ "status": "capturing", "source": "system" })
}

/// `stop_system_audio_capture`
pub fn stop_system_audio_capture(app: &AppHandle) -> Value {
    set_status(app, "stopped", None);
    Value::Null
}

/// `manual_stop_continuous`
pub fn manual_stop_continuous(app: &AppHandle) -> Value {
    set_status(app, "stopped", None);
    json!({ "status": "stopped", "reason": "manual" })
}

/// `ptt_start { inputDevice }`
pub fn ptt_start(app: &AppHandle, _args: &Value) -> Value {
    if let Ok(mut p) = app.state::<ListenState>().ptt.lock() {
        *p = true;
    }
    set_status(app, "recording", None);
    json!({ "status": "recording" })
}

/// `ptt_cancel`
pub fn ptt_cancel(app: &AppHandle) -> Value {
    if let Ok(mut p) = app.state::<ListenState>().ptt.lock() {
        *p = false;
    }
    set_status(app, "stopped", None);
    Value::Null
}

/// `ptt_stop_take_wav` — returns base64 WAV of the recording (empty here).
pub fn ptt_stop_take_wav(app: &AppHandle) -> Value {
    if let Ok(mut p) = app.state::<ListenState>().ptt.lock() {
        *p = false;
    }
    Value::String(String::new())
}

/// `ptt_stop_and_transcribe` — server path; not configured locally.
pub fn ptt_stop_and_transcribe(app: &AppHandle) -> Value {
    if let Ok(mut p) = app.state::<ListenState>().ptt.lock() {
        *p = false;
    }
    json!({ "success": false, "transcription": "", "error": "Transcription service not configured" })
}

/// `meeting_detect_set_enabled { enabled }`
pub fn meeting_detect_set_enabled(app: &AppHandle, args: &Value) -> Value {
    let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    app.state::<MeetingDetectState>().set(enabled);
    Value::Null
}

#[derive(Default)]
pub struct MeetingDetectState(pub Mutex<bool>);

impl MeetingDetectState {
    pub fn set(&self, v: bool) {
        if let Ok(mut g) = self.0.lock() {
            *g = v;
        }
    }
}

/// Meeting-app title keywords → app name. Keyword list and app names match the
/// recovered binary ("teams→Microsoft Teams", "webex→Webex", "slack huddle→Slack",
/// "discord voice→Discord", "whereby→Whereby", "zoom meeting→Zoom",
/// "meet.google.com"/"google meet"/"meet - "/“meet –”→Google Meet).
fn meeting_app_for_title(title: &str) -> Option<&'static str> {
    let t = title.to_lowercase();
    for (kw, app) in [
        ("meet.google.com", "Google Meet"),
        ("google meet", "Google Meet"),
        ("meet - ", "Google Meet"),
        ("meet – ", "Google Meet"),
        ("zoom meeting", "Zoom"),
        ("webex", "Webex"),
        ("microsoft teams", "Microsoft Teams"),
        ("slack", "Slack"),
        ("huddle", "Slack"),
        ("discord", "Discord"),
        ("whereby", "Whereby"),
        ("webinar", "Webinar"),
        ("teams", "Microsoft Teams"),
    ] {
        if t.contains(kw) {
            return Some(app);
        }
    }
    None
}

/// Enumerate visible top-level window titles (best effort, per platform).
#[cfg(target_os = "linux")]
fn list_window_titles() -> Vec<String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
    use x11rb::rust_connection::RustConnection;

    let mut out = Vec::new();
    let (conn, screen_num) = match RustConnection::connect(None) {
        Ok(c) => c,
        Err(_) => return out,
    };
    let root = conn.setup().roots[screen_num].root;

    let net_client_list = match conn.intern_atom(false, b"_NET_CLIENT_LIST") {
        Ok(c) => c.reply().map(|r| r.atom).unwrap_or(AtomEnum::NONE.into()),
        Err(_) => AtomEnum::NONE.into(),
    };
    let net_wm_name = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom)
        .unwrap_or(AtomEnum::NONE.into());
    let utf8_string = conn
        .intern_atom(false, b"UTF8_STRING")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom)
        .unwrap_or(AtomEnum::NONE.into());

    let ids: Vec<u32> = if net_client_list != 0 {
        conn.get_property(
            false,
            root,
            net_client_list,
            AtomEnum::WINDOW,
            0,
            u32::MAX,
        )
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| {
            r.value32()
                .map(|it| it.collect::<Vec<u32>>())
                .unwrap_or_default()
        })
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    for wid in ids {
        let title = if net_wm_name != 0 {
            conn.get_property(false, wid, net_wm_name, utf8_string, 0, 1024)
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|r| String::from_utf8(r.value).ok())
        } else {
            None
        };
        let title = title.or_else(|| {
            conn.get_property(false, wid, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|r| String::from_utf8(r.value).ok())
        });
        if let Some(t) = title {
            if !t.is_empty() {
                out.push(t);
            }
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn list_window_titles() -> Vec<String> {
    use std::cell::RefCell;
    use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    };

    thread_local! {
        static TITLES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, _lparam: LPARAM) -> windows::core::BOOL {
        if IsWindowVisible(hwnd).as_bool() {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                let n = GetWindowTextW(hwnd, &mut buf);
                if n > 0 {
                    if let Ok(s) = String::from_utf16(&buf[..n as usize]) {
                        TITLES.with(|c| c.borrow_mut().push(s));
                    }
                }
            }
        }
        TRUE
    }

    TITLES.with(|c| c.borrow_mut().clear());
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(0));
    }
    TITLES.with(|c| c.borrow().clone())
}

/// Watcher: poll window titles every 5 s while enabled; emit
/// `meeting-detected { app }` on match and `meeting-ended` when gone.
pub fn spawn_meeting_watcher(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut last: Option<String> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if !handle
                .state::<MeetingDetectState>()
                .0
                .lock()
                .map(|g| *g)
                .unwrap_or(false)
            {
                last = None;
                continue;
            }
            let titles = list_window_titles();
            let found = titles
                .iter()
                .find_map(|t| meeting_app_for_title(t).map(|a| a.to_string()));
            match (found, last.take()) {
                (Some(cur), prev) if prev.as_deref() != Some(cur.as_str()) => {
                    let _ = handle.emit("meeting-detected", cur.clone());
                    last = Some(cur);
                }
                (Some(cur), None) => last = Some(cur),
                (None, Some(_)) => {
                    let _ = handle.emit("meeting-ended", ());
                }
                (None, None) => {}
                (_, prev) => last = prev,
            }
        }
    });
}

/// Re-export for lib wiring.
pub use speaker::list_input_devices as input_devices;
pub use speaker::list_output_devices as output_devices;
