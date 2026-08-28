# Changelog

All notable changes to dictaku will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-08-27

### Added

- Global hotkey `Ctrl+Alt+D` to toggle dictation from any Windows application
- Whisper local transcription using whisper.cpp (supports tiny / base / small models)
- Audio capture via WASAPI through cpal (16 kHz mono f32 pipeline)
- Keyboard injection via enigo (SendInput Win32 API)
- System tray icon with 3 visual states: Idle / Listening (pulsing jade) / Inserted (check)
- Auto-stop on silence after configurable timeout (default 3 s)
- Multi-language support: FR / EN / NL with Whisper auto-detection mode
- Local JSON configuration persisted in `%APPDATA%\dictaku\config.json`
- Startup registration in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- PowerShell model download script `scripts/download-model.ps1` (tiny / base / small / medium)
- Degraded mode with tray alert when the configured Whisper model file is absent

### Security

- Zero network calls — all inference runs locally, no telemetry, no account required
- Model files stored in user profile (`~/.dictaku/models/`), never bundled in the installer
