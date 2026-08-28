# Dictaku

Application desktop de dictée vocale Windows, offline, propulsée par [Whisper.cpp](https://github.com/ggerganov/whisper.cpp).

> Squelette v0.1 — documentation complète à venir.

## Stack

- **Tauri 2** (Rust + WebView)
- **Whisper.cpp** (binaire précompilé dans `resources/`)
- **cpal** (capture audio)
- **enigo** (injection clavier)

## Démarrage rapide

```powershell
# 1. Télécharger le modèle Whisper
.\scripts\download-model.ps1 -Model base

# 2. Lancer en développement
cargo tauri dev

# 3. Builder
cargo tauri build
```

## Raccourci

`Ctrl+Alt+D` — Toggle dictée

## Licence

MIT 2026 — Pascal Dengis
