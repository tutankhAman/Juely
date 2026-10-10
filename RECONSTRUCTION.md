# Pluely 1.1.0 — Clean-Room Reconstruction

Reconstructed from the shipped `.deb` (static observation + runtime replay).
Target: `Pluely_1.1.0_amd64.deb` → `usr/bin/pluely`, sha256 `a47307…`.

## Layout
```
pluely-phase1/
  dist/                      recovered production frontend (verbatim)
  src-tauri/src/
    lib.rs                   IPC router: traces + dispatches
    window.rs                main/dashboard/meeting-overlay, stealth, move/fit
    shortcuts.rs             global shortcuts + XDG portal status
    db.rs                    exact recovered migrations (10 tables)
    shared_prefs.rs          pluely:shared-prefs broadcast
    secure_store.rs          vault.dat + byok_store
    session.rs               machine uid + bearer/session lifecycle
    api.rs                   chat streaming, models, license status
    server_api.rs            transforms, ocr, prompts, entitlements
    capture.rs               xcap screenshots/region + portal note
    speaker.rs               cpal device enum, system-audio access
    listen.rs                VAD config, listen/ptt, meeting detect
    activate.rs              license activate/validate/mask
    purchase.rs              plans, checkout, claim, leads
    stealth_keys.rs          hidden global shortcuts
    cli.rs                   Claude/Gemini/Codex/Qwen/Ollama bridge
    single_instance.rs       first-instance guard
  evidence/                  REA process captures + IPC traces per phase
```

## Status by phase
| Phase | Modules | Build | Boot | IPC |
|---|---|---|---|---|
| 1 | shell + tracer | OK | OK | 15 cmds, 0 unknown |
| 2a | window, shortcuts | OK | OK | OK |
| 2b | db, shared_prefs, secure_store | OK | OK | OK |
| 2c | api, server_api, session | OK | OK | OK |
| 2d | capture, speaker, listen | OK | OK | OK |
| 2e | activate, purchase, stealth_keys, cli, single_instance | OK | OK | OK |

All runtime IPC commands are recognized (no `UNKNOWN/NEW`). Rebuild stays
resident (Tauri event loop); the original exits immediately after boot, which
is recorded as an observed behavioral difference, not an equivalence claim.

## Server-dependent surfaces (documented, not reachable without credentials)
`api.pluely.com`, `/payments/v1/licenses`, `wss://api.deepgram.com/v1/listen`,
PostHog. These return "not configured" shapes identical to the recovered
response fields.

## REA evidence IDs (controlled-replay)
- original  ev_445f95feff303ad993dfee3c80d3858633295c30838e72c5e03bd2c7118bcbc6
- phase1    ev_99d2bb473f30025a2da1d7658d4bc6c1ceac36d09a117be819e3ea36832532e3
- phase2b   ev_3a5ff223313b898753260a2f16f0f404dc99140f266255ba9e58cb74bdb7fcd2
- phase2c   ev_d36bd515dd97878c773656823b3786c67c9d0381c6f75960177df571ae532c31
- phase2d   ev_089a551f0dbfcea88ab5fda24888af7e46745f6f34cf9be851aeb9f47e343df2
- phase2e   (see tool-output/tool_11f6befed001RMQzIFLWCfKb7U)

## Phase 4 — maintainable sources + real BYOK streaming

Added under `src/` (TypeScript, `tsc --noEmit` passes):
- `config/storage-keys.ts`   20 localStorage keys
- `config/ai-providers.ts`   10 LLM providers (curl templates)
- `config/stt-providers.ts`  9 STT providers (curl templates)
- `config/prompts.ts`        default system prompt
- `routes/routes.ts`         14 routes (dashboard/chats/system-prompts/files/
                             shortcuts/screenshot/capture/settings/audio/
                             responses/dev-space + view routes)
- `lib/tauri.ts`             invoke + listen wrappers
- `lib/commands.txt`         72-command IPC surface

`api.rs` now performs a real OpenAI-compatible streaming request when a BYOK
provider is selected in the vault (`selected_pluely_model` + `byok_providers`),
emitting `chat_stream_chunk` / `chat_stream_complete`; falls back to a labeled
placeholder stream when no provider is configured. Anthropic uses the
`/v1/messages` SSE path.

## Final run
- 25 IPC commands observed, 0 unknown, 0 errors.
- REA phase4 capture: ev_62cd7dc3c7f6b0505c097e1049acaa03a2260e92e108a37af8e428ac3a46a1c5

## Local dev license (flipped)
- `activate_license_api` accepts self-issued `LOCAL-…` keys offline, persists
  `pluely_license_key` + `pluely_instance_id` to the vault, marks LicenseState
  and ValidateCache active+dev. Nothing touches Pluely's servers.
- `validate_license_api` honors the cache, then a `LOCAL-` vault key.
- `get_entitlements` returns dev tier + quotas when locally licensed.
- Vault keyed per machine with `phase1`/HOSTNAME fallback on read.
- Verified: `validate -> active=true`, frontend takes the `is_active` path
  (`hasActiveLicense` + `isDevLicense` set, licensed branch executes).
  Boot trace: `evidence/licensed-boot-trace.log`. Vault seeded at
  `~/.local/share/com.pluely.phase1/vault.dat` (`LOCAL-DEV-0001`).

## Installed (2026-10-09)
- Removed original `~/.local/bin/pluely` (sha256 `a47307…`, 50M) + its desktop
  entry. Installer `.deb` kept in Downloads as backup. User data dirs untouched.
- Installed release build (15M, LTO+strip) to `~/.local/bin/pluely`,
  icons to `~/.local/share/icons/hicolor/*/apps/pluely.png`,
  desktop entry `~/.local/share/applications/pluely.desktop`.
- Running on the user session (PID 2784410), 38 IPC calls, 0 unknown, 0 errors.
  Local dev license (`LOCAL-DEV-0001`) active via seeded vault.

## Bugfix round (2026-10-09): dashboard + drag
- **Dashboard showed overlay, not dashboard.** Root cause: the dashboard
  window was created with the default URL (`/`), which renders the overlay
  route; the shipped frontend routes the dashboard at path `/dashboard`
  (route table) with label-based rendering only for overlay variants
  (`kat()`). Fix: `ensure_dashboard` now uses
  `WebviewUrl::App("dashboard")`; the asset server falls back to index.html.
  A/B: 0 dashboard IPC calls in overlay-only boots vs 17
  (`get_entitlements`, `fetch_prompts`, `cli_detect`, `get_banner`) with the
  dashboard open.
- **Overlay not draggable.** The bundle drives moves purely through native
  `data-tauri-drag-region` elements (no `startDragging()` call sites). Fixes:
  (1) granted all 71 invoked `core:window:allow-*` permissions incl.
  `allow-start-dragging` (also added the 16 webview, 4 event, 5 image,
  4 sql, updater/autostart defaults — 108 total; this also un-breaks
  `plugin:sql`, which is why `pluely.db` never initialized);
  (2) registered `tauri-plugin-autostart` (frontend invokes it);
  (3) global init script routes left-button mousedowns in drag regions
  (excluding buttons/inputs/links) through `startDragging()` as a fallback.
  Headless drag-motion verification was not possible (no WM/xdotool
  visibility); please confirm on the live session.

## Model/listen/STT fix round (2026-10-09, uninstalled)
- **No models in overlay.** `fetch_models` returned provider-grouped objects;
  the overlay maps each entry as `{id, name, isAvailable}` (+ optional
  `modality`, `description`). Now returns a flat 21-model hosted catalog
  matching the recovered contract.
- **Saved keys not surfacing.** Rust never emitted the `byok-changed` /
  `license-changed` cross-window events the frontend subscribes to, so the
  overlay never refreshed providers after Dev Space saves. Both are emitted now.
- **Listen contract.** `listen_start` now enforces the observed string-error
  contract ("already running" / license / STT-key errors) and, with an
  `ownKey`, runs real mic capture (cpal → 16 kHz mono) with chunked Deepgram
  transcription, emitting `listen-transcript {text,isFinal,channel,speaker,
  start}` + `listen-status`. Without credentials it fails with the observed
  user-facing strings instead of hanging silently.
- Running as dev build (target/debug) for verification; release NOT reinstalled.

## Dashboard crash fix (2026-10-09, dev only)
- Symptom: dashboard showed the "Something unexpected happened" boundary.
- Cause 1 (masked first): `update_shortcuts` expected `bindings` as an
  array; the frontend sends a map (+ `customActions`). Fixed `ShortcutsConfig`
  (map + renamed field); `ShortcutBinding.id` now optional (key is the id).
- Cause 2 (the crash): my flat `fetch_models` entries lacked `provider`;
  dashboard license section maps `pe.provider` → TypeError on undefined.
  Entries now carry `provider` (+ `id/model/name/description/isAvailable/
  modality`), satisfying both overlay and dashboard consumers.
- Also registered the updater plugin with the recovered pubkey/endpoints
  (update check now runs for real) and verified the dashboard renders
  (sidebar + Home + 21-model catalog, 364 nodes, no boundary).

## Audio reliability round (2026-10-09, dev only)
- **Per-chunk mic reopen removed.** Capture is now one persistent input per
  channel for the whole session (`audio::open_input` once, owned by its
  thread — audio handles are `!Send`), with rebuild-on-failure + backoff and
  the observed "[listen] microphone stream failed; rebuilding" notice.
- **PulseAudio enumeration on Linux** (`libpulse-binding` 2.30.1, same as
  upstream deps): picker now lists friendly names incl. Bluetooth and monitor
  sources; cpal remains the fallback and the only path on macOS/Windows
  (all PA code is `#[cfg(target_os = "linux")]`-gated).
- **System-audio channel**: monitor of the selected output is captured alongside
  the mic and transcribed as channel 1 ("them"); best-effort, mic-only elsewhere.
- **Bluetooth profile pin**: while a `bluez_*` mic is selected, the owning card
  is pinned to `headset_head_unit` for the session and restored on drop
  (pactl, best-effort) to stop A2DP↔HSP flapping from tearing down the mic.
- Fixed a real teardown abort: PA `Context` must drop before `Mainloop`
  (field order in `Session`); verified by unit test (no SIGABRT).

## Listen reliability round (2026-10-09, dev only)
- **System audio silent.** Two causes: (1) the installed release predates PA
  enumeration, so stored ALSA output ids resolve to no monitor → system thread
  never started; (2) no fallback. Now falls back to the default sink's monitor
  and emits `listen-notice` stating the active monitor (or "mic only").
- **Cut sentences.** Fixed 4 s windows chopped utterances; replaced with a
  VAD segmenter (onset on `sensitivity_rms`, trailing-silence offset per
  `silence_chunks`, pre-roll backfill, 30–120 s cap). Unit-tested: speech
  across a mid-pause stays one piece; silence yields nothing.
- Debug without mic: rebuild/open loops verified headless (30 IPC, 0 unknown).

## Stealth verdict (Linux): NOT undetectable — verified in tao 0.37.1 source
- `tao::Window::set_content_protection` exists ONLY in the macos
  (`NSWindowSharingType::None`) and windows (`WDA_EXCLUDEFROMCAPTURE`)
  backends. The shared docs state: "iOS / Android / Linux: Unsupported."
- On Linux our `set_content_protected(true)` is a silent no-op: the overlay
  WILL appear in screenshots, recordings, and screenshares. The original
  binary calls the same API, so it has the identical limitation on Linux.
- Current posture is "inconspicuous, not invisible": transparent borderless
  overlay + skip-taskbar + always-on-top. True capture exclusion is not
  achievable via Tauri/tao on Linux (no client opt-out in X11/Wayland capture
  paths); would require e.g. a layer-shell surface outside Tauri.

## UI notice fix (2026-10-09, dev only)
- `listen-notice` for system capture now shows the PulseAudio description
  ("Monitor of Airdopes 280 ANC") via `audio::display_name_for_source`,
  never the raw `bluez_output.…​.monitor` id.

## Windows bundle (2026-10-09)
- Cross-compiled with `cargo-xwin` (xwin SDK cache) targeting
  `x86_64-pc-windows-msvc`; `icon.ico` generated from the recovered PNG.
- NSIS bundling on Linux: official nsis-3.11 zip + `nsis_tauri_utils.dll`
  v0.5.3 run under Wine via a `~/.local/bin/makensis` shim that translates
  POSIX flags and rewrites POSIX paths in `installer.nsi` to `Z:\` form.
- Artifacts in `~/Downloads`:
  - `Pluely_1.1.0_x64-setup.exe` (6.9 MB NSIS installer, sha256 4998a7be…)
  - `Pluely_1.1.0_x64_standalone.exe` (bare binary, portable)
- Notes: unsigned (no Windows cert); WebView2 runtime is downloaded by the
  installer on first run (downloadBootstrapper mode, as shipped upstream).

## Windows console-window fix (2026-10-09)
- Cause: `src/main.rs` lacked `#![windows_subsystem = "windows"]`, so the exe
  was built with PE subsystem 3 (CONSOLE) — Windows allocated a console
  alongside the app for both the installer-launched and standalone exe.
- Fix: added the attribute behind `not(debug_assertions)` (debug builds keep
  the console for logs). Verified: PE subsystem now 2 (GUI). Both
  `Pluely_1.1.0_x64-setup.exe` and `_standalone.exe` in Downloads rebuilt.

## Windows dashboard + shortcut fix (2026-10-09, round 3)
- **Cause A (dashboard):** `ensure_dashboard` created the dashboard window
  lazily inside the `open_dashboard` IPC callback. Upstream pre-creates it at
  startup on the main thread (binary string: "Failed to pre-create dashboard
  window on startup"). On Windows, building a webview window from an IPC
  thread fails/deadlocks, so the button and shortcut both did nothing.
- **Fix A:** `window::precreate_dashboard` runs in `setup` (main thread) with
  `setup_dashboard_close_handler` (close → hide, not destroy), matching
  upstream symbols `create_dashboard_window` / `toggle_dashboard_window`.
- **Cause B (shortcuts):** the rebuilt shortcut handler only emitted a
  `"shortcut"` event which no frontend listener consumes (verified: 0
  occurrences of `sn("shortcut")` in the bundle). Upstream executes actions
  natively (`handle_shortcut_action`, `start_move_window`, `WindowVisibility`)
  and only emits trigger events for frontend-owned actions.
- **Fix B:** implemented `shortcuts::handle_shortcut_action`: toggle_window
  (state-tracked show/hide), toggle_dashboard, focus_input (show + emit
  `focus-text-input`), screenshot/audio_recording/system_audio (license-gated
  emit of `trigger-screenshot`/`start-audio-recording`/`toggle-system-audio`),
  listen/push_to_talk (native listen_start/listen_stop toggle via
  `ListenToggle` hook), move_window via `move_window_{left,right,up,down}`
  bindings (ctrl+arrows default, license-gated, released → `stop_move_window`).
- License gating matches upstream strings: "Skipping move_window registration
  - license inactive", "Ignoring move_window start for direction '<d>' -
  license inactive".
- Also added windowicap permissions `core:window:allow-primary-monitor`,
  `allow-available-monitors`, `allow-monitor-from-point` (frontend
  `plugin:window|current_monitor` call in overlay resize path).
- Tauri 2.12 asset resolver confirmed to fall back `/dashboard` →
  `index.html` (tauri-2.12.2/src/manager/mod.rs:392-412), so the SPA route
  renders without a shipped `dist/dashboard` directory.
- Verified on Linux: xvfb boot prints `[startup] dashboard window pre-created`
  and the env-gated test hook `PLUELY_TEST_OPEN_DASHBOARD=1` prints
  `[test] open_dashboard OK`; 53 IPC commands at boot, 0 unknown, 0 errors.
- Rebuilt Windows artifacts (cargo-xwin 0.23.1 + NSIS/Wine shim):
  - `Pluely_1.1.0_x64-setup.exe` sha256 `fa33c241…`, installer 6.9 MB
  - `Pluely_1.1.0_x64_standalone.exe` sha256 `9fdcc690…`, PE subsystem 2.

## Full IPC/event wiring audit (2026-10-09, round 4)
- Contract sweep: all 73 real frontend `re()` commands dispatch (frontend's
  74th `re("util")` is a false positive inside a lodash template string).
- Event sweep (frontend `sn()` listeners 26 ↔ Rust emits): fixed three gaps.
  - **captured-selection**: `capture_selected_area` now emits the cropped PNG
    (upstream string "Failed to emit captured-selection event"); the selection
    overlay ignores the invoke result — the event is the delivery channel.
  - **capture-region-picked**: emitted alongside, payload
    `{monitor,x,y,width,height}` matching frontend `ome()`.
  - **capture_to_base64 { region }**: now honors the saved autoRegion crop
    instead of always returning the full monitor.
- **meeting-detected / meeting-ended**: real watcher added
  (`listen::spawn_meeting_watcher`): 5 s poll of top-level window titles;
  X11 via `_NET_CLIENT_LIST`/`_NET_WM_NAME` (x11rb, already in graph), Windows
  via `EnumWindows`+`GetWindowTextW` (windows crate added for the win32
  feature). Keyword→app map recovered from the binary: google meet /
  meet.google.com / "meet - / –" → Google Meet, "zoom meeting" → Zoom,
  webex → Webex, microsoft teams / teams → Microsoft Teams, slack+huddle →
  Slack, discord (+voice) → Discord, whereby → Whereby, webinar → Webinar.
  Emits `{ app }`; `meeting_detect_set_enabled` gates it.
- **listen-level**: per-channel RMS beacons ~4 Hz as `{mic, system}` from a
  shared meter in `channel_loop` (frontend scales sqrt(level)·2.2 for bars).
- **shortcuts-portal-status**: emitted once after shortcut setup (plus the
  initial `get_shortcuts_portal_status` fetch the frontend already does).
- **disable_stealth_keys** dispatch added (upstream command recovered from
  binary strings; frontend never calls it but parity kept).
- Remaining accepted gaps: `listen-budget` (hosted-Pro quota, N/A with
  local/byok), `stealth-text-input`/`stealth-text-blur` (require a global
  keyboard hook), `focus-license-input` (emitted by the frontend itself via
  `plugin:event|emit`).
- Verified on Linux: boot → 53 IPC, 0 unknown, 0 errors, x11 watcher
  graceful without a working display.
- Rebuilt artifacts (sha256):
  - `Pluely_1.1.0_x64-setup.exe` `9f7dbfca…`
  - `Pluely_1.1.0_x64_standalone.exe` `0891c66e…` (PE subsystem 2)
