# Pluely 1.1.0 — Clean-Room Reconstruction

A working reconstruction of Pluely 1.1.0 (Tauri 2 + React/Vite), built by
reverse-engineering the shipped `.deb` (`Pluely_1.1.0_amd64.deb`).
See `RECONSTRUCTION.md` for the full phase log and `reference/` for the
recovered evidence (symbols, schema, bundle, provider configs).

## Layout

```
pluely-phase1/
├── README.md                 this file
├── RECONSTRUCTION.md         phase log + REA evidence IDs
├── dist/                     recovered production frontend (verbatim, runnable)
│   ├── index.html
│   └── assets/               381 files (JS/CSS/fonts)
├── src/                      maintainable TS sources recovered from the bundle
│   ├── config/               storage keys, AI/STT providers, prompts
│   ├── routes/routes.ts      14-route table
│   ├── lib/tauri.ts          invoke + listen wrappers
│   └── lib/commands.txt      72-command IPC surface
├── src-tauri/                Rust backend (Tauri 2)
│   ├── Cargo.toml            deps (tauri 2, plugins, cpal, xcap, reqwest…)
│   ├── tauri.conf.json       windows, bundle, CSP
│   ├── capabilities/         default capability set
│   ├── icons/                app icons (from the .deb)
│   └── src/                  17 modules
│       ├── lib.rs            IPC router (trace + dispatch)
│       ├── window.rs         overlay/dashboard/meeting windows, stealth
│       ├── shortcuts.rs      global shortcuts + portal status
│       ├── db.rs             exact migrations (10 tables)
│       ├── shared_prefs.rs   pluely:shared-prefs broadcast
│       ├── secure_store.rs   vault.dat + BYOK store
│       ├── session.rs        machine uid + session lifecycle
│       ├── api.rs            chat streaming (BYOK SSE) + models
│       ├── server_api.rs     transforms/ocr/prompts/entitlements
│       ├── capture.rs        xcap screenshots/region
│       ├── speaker.rs        cpal devices, system-audio access
│       ├── listen.rs         VAD config, listen/ptt, meeting detect
│       ├── activate.rs       license activate/validate/mask
│       ├── purchase.rs       plans/checkout/claim/leads
│       ├── stealth_keys.rs   hidden shortcuts
│       ├── cli.rs            Claude/Gemini/Codex/Qwen/Ollama bridge
│       └── single_instance.rs first-instance guard
├── reference/                raw recovered evidence (symbols, schema, bundle)
└── evidence/                 per-phase IPC traces + REA capture IDs
```

## Requirements

- Rust stable (tested 1.93), Node 20+, system webkit2gtk-4.1 + gtk-3
  (`libwebkit2gtk-4.1-0`, `libgtk-3-0` on Debian/Ubuntu).

## Build & run

```bash
cd src-tauri
cargo build            # debug binary at target/debug/pluely
./target/debug/pluely  # needs a display; headless: xvfb-run -a ./target/debug/pluely
```

Every frontend IPC call is logged to stdout as `[ipc] <command> [args]`.
All 25 boot commands are recognized (no `UNKNOWN/NEW`).

## BYOK chat (real streaming)

Add a provider entry via the Dev Space UI (persisted to the vault as
`byok_providers` with the selected model in `selected_pluely_model`).
`api.rs` then issues a real OpenAI-compatible SSE request and emits
`chat_stream_chunk` / `chat_stream_complete`. Anthropic uses `/v1/messages`.
Without a key it emits a labeled placeholder stream.

## License notes

The recovered `dist/` bundle and response shapes are observations of the
shipped artifact, used for interoperability. Server-side services
(`api.pluely.com`, payments, Deepgram, PostHog) return "not configured"
shapes until credentials are supplied. `secure_store` uses a machine-derived
keystream for at-rest obfuscation — a reconstruction choice, documented in code.
