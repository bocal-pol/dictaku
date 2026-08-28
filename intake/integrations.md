# Catalogue des intégrations — dictaku v0.1

Tous les composants sont locaux. Aucun appel réseau en production.

---

## 1. whisper.cpp — Moteur de transcription

| Attribut | Valeur |
|---|---|
| Dépôt | `https://github.com/ggerganov/whisper.cpp` |
| Intégration | Submodule Git OU binaire précompilé `whisper.dll` / `libwhisper.a` |
| Interface Rust | Crate `whisper-rs` (`https://crates.io/crates/whisper-rs`) — bindings safe autour de whisper.cpp |
| Modèles | Fichiers GGML `.bin` (téléchargés séparément depuis HuggingFace `ggerganov/whisper.cpp`) |
| API principale | `WhisperContext::new(model_path)` → `WhisperState` → `state.full(params, audio_data)` |
| Paramètres clés | `language: Option<&str>` (`None` = auto-détect), `translate: false`, `no_timestamps: false` |
| Thread safety | Un `WhisperState` par thread — ne pas partager entre threads |
| Latence observée | tiny: ~0.3 s · small: ~1.5 s · medium: ~4 s (CPU i7-10th gen, mono 5 s d'audio) |
| Secrets | Aucun |

---

## 2. API Windows Win32 — Raccourci global

| Attribut | Valeur |
|---|---|
| Abstraction Tauri | `tauri-plugin-global-shortcut` v2 |
| API Win32 sous-jacente | `RegisterHotKey` / `UnregisterHotKey` (user32.dll) |
| Alternative si conflit | Hook clavier bas niveau via `SetWindowsHookEx(WH_KEYBOARD_LL)` (plus permissif, plus complexe) |
| Limitation connue | `RegisterHotKey` échoue silencieusement si le raccourci est déjà pris par une autre app |
| Gestion de conflit | Au démarrage : tentative d'enregistrement → si échec → alerte tray + suggestion de raccourci alternatif |
| Secrets | Aucun |

**Code d'enregistrement (Tauri v2) :**
```rust
app.global_shortcut().register("Ctrl+Alt+D", || {
    // toggle dictation state
})?;
```

---

## 3. API Windows Win32 — Injection clavier (SendInput)

| Attribut | Valeur |
|---|---|
| Abstraction Rust | Crate `enigo` v0.2+ (`https://crates.io/crates/enigo`) |
| API Win32 sous-jacente | `SendInput` (user32.dll) avec `INPUT_KEYBOARD` / `KEYEVENTF_UNICODE` |
| Méthode d'injection | `enigo.text(transcribed_text)` — injection caractère par caractère via événements virtuels |
| Limitation UAC | `SendInput` est bloqué pour les fenêtres dont le niveau d'intégrité est > celui du processus appelant |
| Alternative haute intégrité | Service Windows séparé (SYSTEM level) — hors périmètre v0.1 |
| Caractères spéciaux | Les accents FR/NL nécessitent `KEYEVENTF_UNICODE` (géré nativement par `enigo`) |
| Secrets | Aucun |

**Usage minimal :**
```rust
use enigo::{Enigo, Keyboard, Settings};
let mut enigo = Enigo::new(&Settings::default()).unwrap();
enigo.text(&transcribed_text).unwrap();
```

---

## 4. cpal — Capture audio

| Attribut | Valeur |
|---|---|
| Crate | `cpal` v0.15+ (`https://crates.io/crates/cpal`) |
| Rôle | Accès cross-platform aux périphériques audio — utilisé ici pour capture microphone Windows (WASAPI) |
| Backend Windows | WASAPI (exclusif ou partagé) |
| Format imposé | 16 kHz, mono, f32 — converti si nécessaire depuis le format natif du device |
| Buffer | Ring buffer `ringbuf` — le thread audio pousse des samples, le thread Whisper consomme par chunks |
| Permission micro | Déclarée dans le manifeste Windows (`app.manifest`) — Windows demande la permission à l'utilisateur au premier accès |
| Secrets | Aucun |

**Flux de données :**
```
Microphone → cpal stream (f32 samples) → ring buffer → Whisper thread → texte transcrit
```

---

## 5. Tauri v2 — Framework desktop

| Attribut | Valeur |
|---|---|
| Version | Tauri 2.x (stable) |
| Frontend | HTML/CSS/JS vanilla (pas de framework JS requis pour v0.1) |
| IPC | `tauri::command` + `invoke()` côté frontend |
| Plugins utilisés | `tauri-plugin-global-shortcut`, `tauri-plugin-notification`, `tauri-plugin-shell` (optionnel) |
| Tray | `tauri::tray::TrayIconBuilder` — menu contextuel natif Windows |
| Config | `src-tauri/tauri.conf.json` — permissions, bundle, identifier |
| Bundle | `.msi` (via `cargo-tauri build`) ou `.exe` NSIS |
| Secrets | Aucun |

---

## 6. tauri-plugin-notification — Feedback utilisateur

| Attribut | Valeur |
|---|---|
| Plugin | `tauri-plugin-notification` v2 |
| Usage | Notifications tray discrètes pour : erreur micro, modèle manquant, premier lancement |
| Fallback | Changement d'icône tray (états visuels) si les notifications système sont désactivées |
| Secrets | Aucun |

---

## Matrice des dépendances Rust (`Cargo.toml` anticipé)

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-notification = "2"
whisper-rs = "0.12"      # bindings whisper.cpp
cpal = "0.15"
ringbuf = "0.3"
enigo = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

---

## Points de vigilance

| Intégration | Risque | Mitigation |
|---|---|---|
| whisper.cpp build | Compilation C++ depuis le submodule peut échouer si MSVC / clang non installé | Fournir un binaire précompilé `whisper.dll` en fallback dans `resources/` |
| enigo + UAC | Injection silencieusement ignorée dans apps élevées | Détecter et afficher un message d'erreur explicite (voir error-journal.md) |
| cpal WASAPI | Conflit si une autre app a le micro en mode exclusif | Utiliser le mode partagé WASAPI ; documenter dans le README |
| RegisterHotKey | Conflit avec apps existantes (Teams, OBS, etc.) | Permettre la reconfiguration du hotkey (F13 — Should) |
