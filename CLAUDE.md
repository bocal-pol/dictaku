# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

dictaku — Windows desktop dictation app (speech-to-text) built with Tauri 2 (Rust backend + HTML/CSS/JS frontend). Press `Ctrl+Alt+D` from any app, speak, text is injected into the active field. 100% offline via whisper.cpp. No cloud, no account, no telemetry.

## Build and Development Commands

```powershell
# Prerequisites: Rust stable + MSVC Build Tools 2022 + Node.js (for Tauri CLI)
cargo install tauri-cli

# Development (hot-reload WebView frontend, recompile Rust on change)
cargo tauri dev

# Production build (.msi + .exe installer)
cargo tauri build

# Rust checks only (faster than full Tauri build)
cargo check
cargo clippy -- -D warnings
cargo test

# Run a single test
cargo test <test_name>
cargo test --package dictaku -- <module>::<test_name>

# Format
cargo fmt

# Download a Whisper model (required before first run)
# Place the .bin file in ~/.dictaku/models/
# ggml-small.bin (~460 MB) is the recommended default
```

## Architecture

### Process boundary

Tauri splits the app into two processes that communicate via IPC:

- **Rust backend** (`src-tauri/src/`) — owns all system interactions: hotkey registration, audio capture, Whisper inference, text injection, tray icon, config persistence.
- **WebView frontend** (`src/`) — HTML/CSS/JS UI rendered in a WebView. In v0.1 this is minimal (tray menu only). Frontend calls Rust via `invoke()`.

### Rust backend structure (anticipated)

```
src-tauri/src/
├── main.rs          -- Tauri app builder, plugin registration, tray setup
├── commands.rs      -- #[tauri::command] handlers exposed to frontend
├── dictation.rs     -- State machine: Idle → Listening → Inserting → Idle
├── audio.rs         -- cpal audio capture, ring buffer management (16 kHz mono f32)
├── transcribe.rs    -- whisper-rs context management, model loading, inference
├── injection.rs     -- enigo text injection, UAC level detection
├── hotkey.rs        -- tauri-plugin-global-shortcut registration/conflict detection
├── config.rs        -- ~/.dictaku/config.json read/write via serde_json
└── tray.rs          -- TrayIconBuilder, state-driven icon switching
```

### State machine

The core dictation flow is a 3-state machine owned by `dictation.rs`:

```
Idle  --[Ctrl+Alt+D]--> Listening --[Ctrl+Alt+D or silence timeout]--> Inserting --[done]--> Idle
```

State transitions are triggered by the hotkey thread and post audio + transcription results back via Tauri's async runtime (tokio).

### Audio → Whisper pipeline

```
Microphone (WASAPI) → cpal stream → ringbuf (f32 samples) → Whisper thread → transcribed text → enigo injection
```

The ring buffer decouples the real-time audio thread from the (slower) Whisper inference thread. Whisper.cpp processes chunks on a dedicated thread; results are sent back to the main Tauri thread via a channel.

### Key constraints

- `whisper-rs` `WhisperState` is not `Send` — keep inference on a single dedicated thread, pass audio data via channels.
- `enigo` injection fails silently against elevated-integrity processes (UAC). Detect via `GetTokenInformation(TokenIntegrityLevel)` and show a tray alert rather than silently dropping text.
- `RegisterHotKey` (Win32) fails if the shortcut is already taken. Always check the return value and surface the conflict to the user.
- Whisper expects 16 kHz mono f32 audio. `cpal` may capture at a different rate/format — resample in `audio.rs` before passing to Whisper.

### Config file

`~/.dictaku/config.json` — created on first launch with defaults. Schema and field documentation: `intake/data-dictionary.md`.

### Whisper model files

Stored in `~/.dictaku/models/` (not bundled with the app). The app checks for the configured model at startup and enters degraded mode with a tray alert if absent. Model files are never committed to this repo.

## Key Dependencies

| Crate | Purpose |
|---|---|
| `tauri` v2 | Desktop shell, WebView, IPC, tray |
| `tauri-plugin-global-shortcut` v2 | Cross-process hotkey registration |
| `tauri-plugin-notification` v2 | System tray notifications |
| `whisper-rs` | Safe Rust bindings for whisper.cpp |
| `cpal` | Audio device enumeration and capture (WASAPI on Windows) |
| `ringbuf` | Lock-free ring buffer between audio and inference threads |
| `enigo` v0.2+ | Keyboard injection via SendInput/Win32 |
| `serde` + `serde_json` | Config serialization |
| `tokio` | Async runtime (required by Tauri v2) |

## UI / Design

Palette and typography are locked (from `dictaku_fiche.html`):
- Colors: `#081408` (dark bg) · `#2a6a3a` (forest) · `#4a9a5a` (jade accent) · `#e0f0e4` (light text) · `#f0f7f2` (page bg)
- Fonts: Playfair Display (display/titles) + Inter (UI/body)
- Icons: Solar icon set only

Tray icon states: pulsing jade circle (Listening), faint circle (Idle), check mark (Inserted).

## Intake Documents

Full project context in `intake/`:
- `app-spec.md` — functional spec, Given/When/Then acceptance criteria
- `brand-brief.md` — visual identity, tone, icon SVG reference
- `data-dictionary.md` — config.json schema with all field definitions
- `feature-backlog.md` — MoSCoW backlog v0.1 + roadmap v0.2/v0.3
- `integrations.md` — all technical integrations with API examples
- `error-journal.md` — 8 anticipated failure modes with mitigations
