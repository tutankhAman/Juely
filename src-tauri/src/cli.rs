//! Dev Space CLI bridge — reconstructed from Pluely 1.1.0.
//!
//! Recovered symbols: `cli::{cli_chat_stream, cli_detect, cli_ollama_models,
//! kill_active, login_shell_path, resolve_bin, write_image_files, CliInfo,
//! CliSession, ACTIVE_CHILD, GENERATION}`. Detected CLIs and their launch
//! flags observed in `.rodata`:
//!   claude  -> `claude`            (Claude Code)
//!   gemini  -> `gemini`            (`--i`, GEMINI_CLI_TRUST_WORKSPACE)
//!   codex   -> `codex`             (`--skip-git-repo-check`, `-m`, `--model`)
//!   qwen    -> `qwen`              (QWEN_CLI_TRUST_WORKSPACE, `-p`, `--session-id`)
//!   ollama  -> `ollama`            (`--version`, local server)
//! Streaming uses `cli_stream_chunk` / `cli_stream_complete` events.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

pub const EVT_CHUNK: &str = "cli_stream_chunk";
pub const EVT_COMPLETE: &str = "cli_stream_complete";

#[derive(Default)]
pub struct CliState {
    pub active_child: Mutex<Option<u32>>,
    pub generation: Mutex<u64>,
}

const CLIS: &[(&str, &str)] = &[
    ("claude", "Claude Code"),
    ("gemini", "Gemini CLI"),
    ("codex", "OpenAI Codex"),
    ("qwen", "Qwen Code"),
    ("ollama", "Ollama"),
];

fn which(bin: &str) -> Option<String> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        let candidate = std::path::Path::new(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

/// `cli_detect` — returns the list of installed CLIs.
pub fn cli_detect() -> Value {
    let mut out = Vec::new();
    for (id, name) in CLIS {
        let path = which(id);
        out.push(json!({
            "cli": id,
            "name": name,
            "installed": path.is_some(),
            "path": path
        }));
    }
    Value::Array(out)
}

/// `cli_ollama_models` — `ollama list` parsed into model objects.
pub fn cli_ollama_models() -> Value {
    let Some(_path) = which("ollama") else {
        return Value::Array(vec![]);
    };
    match Command::new("ollama").arg("list").output() {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let models: Vec<Value> = text
                .lines()
                .skip(1)
                .filter_map(|l| l.split_whitespace().next())
                .filter(|s| !s.is_empty())
                .map(|m| json!({ "id": m, "name": m }))
                .collect();
            Value::Array(models)
        }
        _ => Value::Array(vec![]),
    }
}

/// `cli_chat_stream { cli, model, prompt }` — spawn the CLI and stream stdout.
pub fn cli_chat_stream(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let cli = args.get("cli").and_then(|v| v.as_str()).unwrap_or("claude");
    let model = args.get("model").and_then(|v| v.as_str());
    let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or_default();

    let Some(bin) = which(cli) else {
        return Err(format!("Couldn't start the CLI: {cli} not found"));
    };

    let mut cmd = Command::new(&bin);
    let mut envs: HashMap<&str, &str> = HashMap::new();
    match cli {
        "gemini" => {
            cmd.arg("--i");
            envs.insert("GEMINI_CLI_TRUST_WORKSPACE", "1");
        }
        "codex" => {
            cmd.arg("--skip-git-repo-check");
            if let Some(m) = model {
                cmd.args(["-m", m]);
            }
        }
        "qwen" => {
            cmd.args(["-p", prompt]);
            envs.insert("QWEN_CLI_TRUST_WORKSPACE", "1");
        }
        _ => {}
    }
    if cli != "qwen" {
        cmd.arg(prompt);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't start the CLI: {e}"))?;

    let pid = child.id();
    if let Ok(mut g) = app.state::<CliState>().active_child.lock() {
        *g = Some(pid);
    }
    let gen = {
        let state = app.state::<CliState>();
        let mut g = state.generation.lock().map_err(|e| e.to_string())?;
        *g += 1;
        *g
    };

    let handle = app.clone();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let current = handle
                    .state::<CliState>()
                    .generation
                    .lock()
                    .map(|g| *g)
                    .unwrap_or(0);
                if current != gen {
                    break;
                }
                let _ = handle.emit(EVT_CHUNK, format!("{line}\n"));
            }
        }
        let _ = child.wait();
        let _ = handle.emit(EVT_COMPLETE, ());
    });

    Ok(json!({ "started": true, "cli": cli, "pid": pid }))
}

/// `cli_chat_stop`
pub fn cli_chat_stop(app: &AppHandle) -> Value {
    let state = app.state::<CliState>();
    if let Ok(mut g) = state.generation.lock() {
        *g += 1;
    }
    if let Some(pid) = state.active_child.lock().ok().and_then(|g| *g) {
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
    Value::Null
}

/// `login_shell_path` — resolve the user's login PATH (for GUI-launched apps).
pub fn login_shell_path() -> Value {
    match Command::new("/bin/sh").args(["-lc", "printf %s \"$PATH\""]).output() {
        Ok(o) if o.status.success() => {
            Value::String(String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        _ => Value::String(std::env::var("PATH").unwrap_or_default()),
    }
}