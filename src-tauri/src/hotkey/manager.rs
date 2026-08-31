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
const STOP_SHORTCUT: &str = "escape";

pub fn register_global_shortcut<R: Runtime>(app: &tauri::App<R>) -> Result<(), DictakuError> {
    let hotkey_str = {
        let state = app.state::<AppState>();
        let config = state.config.blocking_lock();
        if config.hotkey.is_empty() {
            DEFAULT_SHORTCUT.to_string()
        } else {
            config.hotkey.clone()
        }
    };

    let start_shortcut: Shortcut = hotkey_str
        .parse()
        .map_err(|_| DictakuError::HotkeyRegistration(format!("Raccourci invalide : {hotkey_str}")))?;

    let stop_shortcut: Shortcut = STOP_SHORTCUT
        .parse()
        .map_err(|_| DictakuError::HotkeyRegistration(format!("Raccourci stop invalide : {STOP_SHORTCUT}")))?;

    // stop_signal partagé entre le pipeline et le handler Escape.
    let active_stop: Arc<Mutex<Option<Arc<Mutex<bool>>>>> = Arc::new(Mutex::new(None));

    // Clone pour le handler Escape.
    let _active_stop_for_escape = active_stop.clone();
    let stop_shortcut_clone = stop_shortcut;

    app.global_shortcut()
        .on_shortcut(start_shortcut, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }

            let app_state = _app.state::<AppState>();
            let current_state = tauri::async_runtime::block_on(async {
                app_state.state.lock().await.clone()
            });

            // Raccourci démarrer : ignoré si une dictée est déjà en cours.
            if current_state != DictationState::Idle {
                warn!("Hotkey start ignoré : dictée déjà en cours ({current_state:?})");
                return;
            }

            info!("Hotkey : démarrage dictée");

            let state_arc = app_state.state.clone();
            let config_arc = app_state.config.clone();
            let active_stop_clone = active_stop.clone();
            let app_handle = _app.clone();
            let stop_sc = stop_shortcut_clone;

            tauri::async_runtime::spawn(async move {
                // Capture la fenêtre active AVANT tout traitement.
                let target_window = crate::platform::WindowHandle::foreground();

                let config = config_arc.lock().await.clone();
                let cli_path = resolve_whisper_cli();
                let model_path = config.models_dir().join(config.model.filename());

                if !cli_path.exists() {
                    error!("whisper-cli.exe introuvable : {}", cli_path.display());
                    return;
                }
                if !model_path.exists() {
                    error!("Modèle Whisper absent : {}", model_path.display());
                    return;
                }

                // Transition Idle → Listening.
                *state_arc.lock().await = DictationState::Listening;

                // Enregistre Escape comme raccourci d'arrêt dynamique.
                let active_stop_esc = active_stop_clone.clone();
                let state_arc_esc = state_arc.clone();
                let _ = app_handle.global_shortcut().on_shortcut(stop_sc, move |_app2, _sc, ev| {
                    if ev.state() != ShortcutState::Pressed {
                        return;
                    }
                    let s = tauri::async_runtime::block_on(async {
                        state_arc_esc.lock().await.clone()
                    });
                    match s {
                        DictationState::Listening => {
                            info!("Escape : arrêt capture — envoi audio pour transcription");
                            // Lève le stop_signal pour fermer le channel audio.
                            if let Some(sig) = active_stop_esc.lock().unwrap().as_ref() {
                                *sig.lock().unwrap() = true;
                            }
                        }
                        DictationState::Transcribing => {
                            info!("Escape ignoré : transcription en cours (attendre la fin)");
                        }
                        _ => {}
                    }
                });

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
                        *state_arc.lock().await = DictationState::Idle;
                        let _ = app_handle.global_shortcut().unregister(stop_sc);
                        return;
                    }
                };

                *active_stop_clone.lock().unwrap() = Some(stop_signal);

                // Collecte des chunks audio.
                let state_for_stt = state_arc.clone();
                let active_stop_clear = active_stop_clone.clone();
                let app_handle2 = app_handle.clone();

                tokio::spawn(async move {
                    let mut all_samples: Vec<f32> = Vec::new();
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
                        *state_for_stt.lock().await = DictationState::Idle;
                        *active_stop_clear.lock().unwrap() = None;
                        let _ = app_handle2.global_shortcut().unregister(stop_sc);
                        return;
                    }

                    info!("Transcription de {} samples", all_samples.len());

                    let transcriber = WhisperTranscriber::new(cli_path, model_path, config.language);
                    let state_inj = state_for_stt.clone();
                    let stop_clear2 = active_stop_clear.clone();
                    let app_handle3 = app_handle2.clone();

                    let text = match tokio::task::spawn_blocking(move || {
                        transcriber.transcribe(&all_samples)
                    }).await {
                        Ok(Ok(t)) => t,
                        Ok(Err(e)) => {
                            error!("Erreur transcription : {e}");
                            *state_for_stt.lock().await = DictationState::Idle;
                            *active_stop_clear.lock().unwrap() = None;
                            let _ = app_handle2.global_shortcut().unregister(stop_sc);
                            return;
                        }
                        Err(e) => {
                            error!("spawn_blocking transcription : {e}");
                            *state_for_stt.lock().await = DictationState::Idle;
                            *active_stop_clear.lock().unwrap() = None;
                            let _ = app_handle2.global_shortcut().unregister(stop_sc);
                            return;
                        }
                    };

                    // Désinscription d'Escape — la dictée est terminée.
                    let _ = app_handle3.global_shortcut().unregister(stop_sc);

                    if text.is_empty() {
                        info!("Transcription vide — retour Idle");
                        *state_inj.lock().await = DictationState::Idle;
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

                    let inj_delay = config.injection_delay_ms;
                    let state_done = state_inj.clone();
                    let stop_clear3 = stop_clear2.clone();
                    if let Err(e) = tokio::task::spawn_blocking(move || {
                        if let Some(win) = target_window {
                            win.refocus();
                        }
                        let mut tw = Typewriter::new(inj_delay);
                        tw.enqueue(text);
                        tw.flush_all()
                    }).await {
                        error!("spawn_blocking injection : {e}");
                    }

                    *state_done.lock().await = DictationState::Idle;
                    *stop_clear3.lock().unwrap() = None;
                });
            });
        })
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    info!("Raccourci démarrer : {hotkey_str} | arrêter : {STOP_SHORTCUT} (dynamique)");
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
fn resolve_whisper_cli() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let same_dir = exe_dir.join("whisper-cli.exe");
    if same_dir.exists() {
        return same_dir;
    }

    let resources_subdir = exe_dir.join("resources").join("whisper-cli.exe");
    if resources_subdir.exists() {
        return resources_subdir;
    }

    if let Some(dev_path) = exe_dir.ancestors().find_map(|p| {
        let candidate = p.join("src-tauri").join("resources").join("whisper-cli.exe");
        if candidate.exists() { Some(candidate) } else { None }
    }) {
        return dev_path;
    }

    same_dir
}
