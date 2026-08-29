use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracing::{error, info, warn};

use crate::audio::recorder::{AudioRecorder, VadConfig};
use crate::error::DictakuError;
use crate::injection::typewriter::Typewriter;
use crate::state::app_state::{AppState, DictationState};
use crate::stt::whisper::WhisperTranscriber;

const DEFAULT_SHORTCUT: &str = "ctrl+shift+f12";

pub fn register_global_shortcut<R: Runtime>(app: &tauri::App<R>) -> Result<(), DictakuError> {
    // Lit le raccourci depuis la config — fallback sur DEFAULT_SHORTCUT si absent.
    let hotkey_str = {
        let state = app.state::<AppState>();
        let config = state.config.blocking_lock();
        if config.hotkey.is_empty() {
            DEFAULT_SHORTCUT.to_string()
        } else {
            config.hotkey.clone()
        }
    };

    let shortcut: Shortcut = hotkey_str
        .parse()
        .map_err(|_| DictakuError::HotkeyRegistration(format!("Raccourci invalide : {hotkey_str}")))?;

    // stop_signal partagé entre le handler et le pipeline en cours.
    let active_stop: Arc<Mutex<Option<Arc<Mutex<bool>>>>> = Arc::new(Mutex::new(None));

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }

            let app_state = _app.state::<AppState>();
            let state_arc = app_state.state.clone();
            let config_arc = app_state.config.clone();
            let active_stop_clone = active_stop.clone();

            tauri::async_runtime::spawn(async move {
                let current = state_arc.lock().await.clone();

                match current {
                    // ── Idle → lancement du pipeline ─────────────────────────
                    DictationState::Idle => {
                        info!("Hotkey : démarrage dictée");

                        // Récupération de la config courante.
                        let config = config_arc.lock().await.clone();

                        // Résolution des chemins — whisper-cli.exe est dans resources/.
                        let cli_path = resolve_whisper_cli();
                        let model_path = config.models_dir().join(config.model.filename());

                        if !cli_path.exists() {
                            error!("whisper-cli.exe introuvable : {}", cli_path.display());
                            return;
                        }
                        if !model_path.exists() {
                            error!(
                                "Modèle Whisper absent : {} — télécharger via le script download-model.ps1",
                                model_path.display()
                            );
                            return;
                        }

                        // Transition Idle → Listening.
                        {
                            let mut s = state_arc.lock().await;
                            *s = DictationState::Listening;
                        }

                        // Démarrage de la capture audio.
                        let vad = VadConfig {
                            threshold: config.vad_threshold,
                            silence_duration: std::time::Duration::from_millis(config.vad_silence_ms),
                        };
                        let recorder = AudioRecorder::new(vad);
                        let (mut rx, stop_signal) = match recorder.start_recording() {
                            Ok(pair) => pair,
                            Err(e) => {
                                error!("Erreur démarrage audio : {e}");
                                let mut s = state_arc.lock().await;
                                *s = DictationState::Idle;
                                return;
                            }
                        };

                        // Mémorisation du stop_signal pour l'annulation.
                        *active_stop_clone.lock().unwrap() = Some(stop_signal.clone());

                        // Collecte des chunks audio dans une tâche async puis
                        // lance la transcription dans un thread bloquant dédié.
                        let state_for_stt = state_arc.clone();
                        let active_stop_clear = active_stop_clone.clone();
                        tokio::spawn(async move {
                            let mut all_samples: Vec<f32> = Vec::new();

                            // Lecture des chunks jusqu'à fermeture du channel (VAD silence ou stop).
                            while let Some(chunk) = rx.recv().await {
                                all_samples.extend_from_slice(&chunk);
                            }

                            // Transition Listening → Transcribing.
                            {
                                let mut s = state_for_stt.lock().await;
                                if *s == DictationState::Listening {
                                    *s = DictationState::Transcribing;
                                }
                            }

                            if all_samples.is_empty() {
                                info!("Aucun audio capturé — retour Idle");
                                let mut s = state_for_stt.lock().await;
                                *s = DictationState::Idle;
                                *active_stop_clear.lock().unwrap() = None;
                                return;
                            }

                            info!("Transcription de {} samples", all_samples.len());

                            // Transcription dans un thread bloquant (CPU-bound).
                            let transcriber = WhisperTranscriber::new(
                                cli_path,
                                model_path,
                                config.language,
                            );
                            let state_inj = state_for_stt.clone();
                            let stop_clear2 = active_stop_clear.clone();
                            let text = match tokio::task::spawn_blocking(move || {
                                transcriber.transcribe(&all_samples)
                            }).await {
                                Ok(Ok(t)) => t,
                                Ok(Err(e)) => {
                                    error!("Erreur transcription : {e}");
                                    let mut s = state_for_stt.lock().await;
                                    *s = DictationState::Idle;
                                    *active_stop_clear.lock().unwrap() = None;
                                    return;
                                }
                                Err(e) => {
                                    error!("spawn_blocking transcription : {e}");
                                    let mut s = state_for_stt.lock().await;
                                    *s = DictationState::Idle;
                                    *active_stop_clear.lock().unwrap() = None;
                                    return;
                                }
                            };

                            if text.is_empty() {
                                info!("Transcription vide — retour Idle");
                                let mut s = state_inj.lock().await;
                                *s = DictationState::Idle;
                                *stop_clear2.lock().unwrap() = None;
                                return;
                            }

                            // Transition Transcribing → Injecting.
                            {
                                let mut s = state_inj.lock().await;
                                if *s == DictationState::Transcribing {
                                    *s = DictationState::Injecting;
                                }
                            }

                            info!("Injection : «{}»", &text[..text.len().min(60)]);

                            // Injection dans un thread bloquant (enigo est synchrone).
                            let inj_delay = config.injection_delay_ms;
                            let state_done = state_inj.clone();
                            let stop_clear3 = stop_clear2.clone();
                            if let Err(e) = tokio::task::spawn_blocking(move || {
                                let mut tw = Typewriter::new(inj_delay);
                                tw.enqueue(text);
                                tw.flush_all()
                            }).await {
                                error!("spawn_blocking injection : {e}");
                            }

                            // Retour à Idle.
                            let mut s = state_done.lock().await;
                            *s = DictationState::Idle;
                            *stop_clear3.lock().unwrap() = None;
                        });
                    }

                    // ── Listening / Transcribing → annulation ─────────────────
                    DictationState::Listening | DictationState::Transcribing => {
                        info!("Hotkey : annulation dictée");

                        // Lève le stop_signal si un pipeline est actif.
                        if let Some(sig) = active_stop_clone.lock().unwrap().as_ref() {
                            *sig.lock().unwrap() = true;
                        }

                        let mut s = state_arc.lock().await;
                        *s = DictationState::Idle;
                    }

                    DictationState::Injecting => {
                        warn!("Hotkey ignoré : injection en cours");
                    }
                }
            });
        })
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    info!("Raccourci global enregistré : {DEFAULT_SHORTCUT}");
    Ok(())
}

pub fn unregister_global_shortcut<R: Runtime>(app: &tauri::App<R>) -> Result<(), DictakuError> {
    let shortcut: Shortcut = DEFAULT_SHORTCUT
        .parse()
        .map_err(|_| DictakuError::HotkeyRegistration(format!("Raccourci invalide : {DEFAULT_SHORTCUT}")))?;

    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    info!("Raccourci global désenregistré : {DEFAULT_SHORTCUT}");
    Ok(())
}

/// Résout le chemin de whisper-cli.exe.
///
/// Tauri 2 NSIS place les resources dans `<install_dir>/` (même niveau que l'exe).
/// En dev, elles sont dans `src-tauri/resources/`.
fn resolve_whisper_cli() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Même dossier que l'exe (prod NSIS)
    let same_dir = exe_dir.join("whisper-cli.exe");
    if same_dir.exists() {
        return same_dir;
    }

    // 2. Sous-dossier resources/ (certaines cibles Tauri)
    let resources_subdir = exe_dir.join("resources").join("whisper-cli.exe");
    if resources_subdir.exists() {
        return resources_subdir;
    }

    // 3. Dev : src-tauri/resources/whisper-cli.exe (remonte l'arborescence)
    if let Some(dev_path) = exe_dir.ancestors().find_map(|p| {
        let candidate = p.join("src-tauri").join("resources").join("whisper-cli.exe");
        if candidate.exists() { Some(candidate) } else { None }
    }) {
        return dev_path;
    }

    // Fallback : même dossier (le log d'erreur indiquera qu'il est absent)
    same_dir
}
