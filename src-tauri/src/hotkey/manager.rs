use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracing::{error, info, warn};

use crate::audio::recorder::{AudioRecorder, VadConfig};
use crate::error::DictakuError;
use crate::injection::typewriter::Typewriter;
use crate::stt::{is_sr_available, start_sr_session, HybridParams};
use crate::state::app_state::{AppState, DictationState};

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

    let active_stop: Arc<Mutex<Option<Arc<Mutex<bool>>>>> = Arc::new(Mutex::new(None));
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

            if current_state != DictationState::Idle {
                warn!("Hotkey start ignoré : dictée déjà en cours ({current_state:?})");
                return;
            }

            info!("Hotkey : démarrage dictée");

            let state_arc    = app_state.state.clone();
            let config_arc   = app_state.config.clone();
            let active_stop_clone = active_stop.clone();
            let app_handle   = _app.clone();
            let stop_sc      = stop_shortcut_clone;

            tauri::async_runtime::spawn(async move {
                let target_window = crate::platform::WindowHandle::foreground();

                let config     = config_arc.lock().await.clone();
                let cli_path   = resolve_whisper_cli();
                let model_path = config.models_dir().join(config.model.filename());
                let models_dir = config.models_dir();

                if !cli_path.exists() {
                    error!("whisper-cli.exe introuvable : {}", cli_path.display());
                    return;
                }

                // Transition Idle → Listening.
                *state_arc.lock().await = DictationState::Listening;

                // — — — GESTIONNAIRE ESCAPE — — —
                let active_stop_esc = active_stop_clone.clone();
                let state_arc_esc   = state_arc.clone();
                let _ = app_handle.global_shortcut().on_shortcut(stop_sc, move |_app2, _sc, ev| {
                    if ev.state() != ShortcutState::Pressed { return; }
                    let s = tauri::async_runtime::block_on(async {
                        state_arc_esc.lock().await.clone()
                    });
                    match s {
                        DictationState::Listening => {
                            info!("Escape : arrêt capture");
                            if let Some(sig) = active_stop_esc.lock().unwrap().as_ref() {
                                *sig.lock().unwrap() = true;
                            }
                        }
                        DictationState::Transcribing => {
                            info!("Escape ignoré : transcription en cours");
                        }
                        _ => {}
                    }
                });

                // — — — AUDIO — — —
                let vad = VadConfig {
                    threshold:        config.vad_threshold,
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

                // — — — WINDOWS SR — — —
                let sr_available = is_sr_available();
                info!("Windows SR disponible : {sr_available}");

                // Texte SR accumulé en temps réel.
                let sr_text_arc: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
                let sr_text_for_inject             = sr_text_arc.clone();

                // Fenêtre cible pour la refocusse avant injection.
                let target_win_for_sr = target_window;

                // Injector clone pour le thread SR.
                let inj_delay = config.injection_delay_ms;

                let sr_session_opt = if sr_available {
                    let lang = config.language.clone();
                    match start_sr_session(&lang, move |fragment: &str| {
                        // Inject fragment into the active field immediately.
                        let fragment = fragment.to_string();
                        let mut acc = sr_text_for_inject.lock().unwrap();
                        // Ajoute un espace entre les fragments sauf si premier.
                        let text_to_inject = if acc.is_empty() {
                            fragment.clone()
                        } else {
                            format!(" {fragment}")
                        };
                        // Refocus + injection.
                        if let Some(win) = target_win_for_sr {
                            win.refocus();
                        }
                        let mut tw = Typewriter::new(inj_delay);
                        tw.enqueue(text_to_inject);
                        if let Err(e) = tw.flush_all() {
                            error!("SR injection fragment erreur : {e}");
                        } else {
                            if !acc.is_empty() { acc.push(' '); }
                            acc.push_str(&fragment);
                        }
                    }) {
                        Ok(sess) => {
                            info!("Windows SR : session démarrée");
                            Some(sess)
                        }
                        Err(e) => {
                            warn!("Windows SR indisponible ({e}) — mode Whisper seul");
                            None
                        }
                    }
                } else {
                    None
                };

                // — — — COLLECTE AUDIO — — —
                let state_for_stt    = state_arc.clone();
                let active_stop_clear = active_stop_clone.clone();
                let app_handle2      = app_handle.clone();

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

                    info!("Audio capturé : {} samples", all_samples.len());

                    let state_inj    = state_for_stt.clone();
                    let stop_clear2  = active_stop_clear.clone();
                    let app_handle3  = app_handle2.clone();
                    let lang         = config.language.clone();
                    let sr_text_snap = sr_text_arc.lock().unwrap().clone();

                    // Arrêter la session SR (bloquant — thread join).
                    let sr_injected = if let Some(sess) = sr_session_opt {
                        let final_sr = tokio::task::spawn_blocking(move || sess.stop())
                            .await
                            .unwrap_or_default();
                        // Préférer le snapshot accumulé (synchronisé via Mutex pendant la dictée).
                        if sr_text_snap.is_empty() { final_sr } else { sr_text_snap }
                    } else {
                        String::new()
                    };

                    info!("SR injecté total : {:?}", &sr_injected[..sr_injected.len().min(80)]);

                    // — — — WHISPER + COMPARAISON — — —
                    let hybrid_result = if !sr_injected.is_empty() {
                        // Mode SR actif : comparer SR déjà injecté vs Whisper.
                        let params = HybridParams {
                            samples:    all_samples,
                            language:   lang,
                            cli_path,
                            model_path,
                            models_dir,
                            sr_injected: sr_injected.clone(),
                        };
                        match tokio::task::spawn_blocking(move || {
                            crate::stt::compare_with_whisper(params)
                        }).await {
                            Ok(Ok(r))  => r,
                            Ok(Err(e)) => {
                                error!("Comparaison Whisper : {e}");
                                // SR a déjà injecté — on accepte tel quel.
                                crate::stt::HybridResult::FastOnly { text: sr_injected }
                            }
                            Err(e) => {
                                error!("spawn_blocking comparaison : {e}");
                                crate::stt::HybridResult::FastOnly { text: sr_injected }
                            }
                        }
                    } else {
                        // Mode fallback (SR indisponible) : Whisper tiny + small.
                        match tokio::task::spawn_blocking(move || {
                            crate::stt::transcribe_hybrid(all_samples, lang, cli_path, model_path, models_dir)
                        }).await {
                            Ok(Ok(r))  => r,
                            Ok(Err(e)) => {
                                error!("Transcription fallback : {e}");
                                *state_for_stt.lock().await = DictationState::Idle;
                                *active_stop_clear.lock().unwrap() = None;
                                let _ = app_handle2.global_shortcut().unregister(stop_sc);
                                return;
                            }
                            Err(e) => {
                                error!("spawn_blocking fallback : {e}");
                                *state_for_stt.lock().await = DictationState::Idle;
                                *active_stop_clear.lock().unwrap() = None;
                                let _ = app_handle2.global_shortcut().unregister(stop_sc);
                                return;
                            }
                        }
                    };

                    // Désinscription d'Escape.
                    let _ = app_handle3.global_shortcut().unregister(stop_sc);

                    use crate::stt::HybridResult;

                    match hybrid_result {
                        // — — — CONCORDANT — — —
                        // SR a déjà injecté et Whisper confirme : rien à faire.
                        HybridResult::Concordant { .. } => {
                            info!("Concordant — SR déjà injecté, rien à faire");
                        }

                        // — — — FAST ONLY — — —
                        // SR a déjà injecté (ou fallback tiny), Whisper absent.
                        HybridResult::FastOnly { text } => {
                            if !text.is_empty() && sr_injected.is_empty() {
                                // Fallback sans SR → injection directe.
                                let corrections = crate::injection::Corrections::load();
                                let final_text  = corrections.apply(&text);
                                inject_text(final_text, target_window, inj_delay).await;
                            }
                            // Si SR a déjà injecté : texte déjà dans le champ, rien à faire.
                        }

                        // — — — PRECISE ONLY — — —
                        // SR vide mais Whisper a un résultat → injection directe Whisper.
                        HybridResult::PreciseOnly { text } => {
                            let corrections = crate::injection::Corrections::load();
                            let final_text  = corrections.apply(&text);
                            inject_text(final_text, target_window, inj_delay).await;
                        }

                        // — — — DIVERGENT — — —
                        // SR a injecté X, Whisper dit Y → popup pour remplacer ou garder.
                        HybridResult::Divergent { sr_text, whisper_text } => {
                            let (tx, rx_oneshot) = tokio::sync::oneshot::channel::<String>();
                            {
                                let app_state = app_handle3.state::<crate::state::app_state::AppState>();
                                *app_state.compare_tx.lock().unwrap() = Some(tx);
                            }

                            if let Some(win) = app_handle3.get_webview_window("compare") {
                                let _ = win.show();
                                let _ = win.set_focus();
                                let _ = app_handle3.emit("dictaku://compare-ready", serde_json::json!({
                                    "sr_text":      sr_text,
                                    "whisper_text": whisper_text,
                                    "sr_was_injected": true,
                                }));
                            }

                            match tokio::time::timeout(
                                std::time::Duration::from_secs(60),
                                rx_oneshot,
                            ).await {
                                Ok(Ok(chosen)) if !chosen.is_empty() => {
                                    // L'utilisateur a choisi — remplacer ce que SR a injecté.
                                    let corrections = crate::injection::Corrections::load();
                                    let final_text  = corrections.apply(&chosen);
                                    // Ctrl+A pour sélectionner tout puis Ctrl+V pour remplacer.
                                    replace_injected_text(final_text, target_window, inj_delay).await;
                                }
                                _ => {
                                    info!("Popup timeout ou annulée — SR conservé tel quel");
                                }
                            }
                        }
                    }

                    *state_inj.lock().await  = DictationState::Idle;
                    *stop_clear2.lock().unwrap() = None;
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

/// Injecte un texte dans la fenêtre cible (focus + clipboard + Ctrl+V).
async fn inject_text(
    text: String,
    target_window: Option<crate::platform::WindowHandle>,
    delay_ms: u64,
) {
    if text.is_empty() { return; }
    info!("Injection : «{}»", &text[..text.len().min(60)]);
    if let Err(e) = tokio::task::spawn_blocking(move || {
        if let Some(win) = target_window { win.refocus(); }
        let mut tw = Typewriter::new(delay_ms);
        tw.enqueue(text);
        tw.flush_all()
    }).await {
        error!("inject_text spawn_blocking : {e}");
    }
}

/// Remplace le contenu du champ actif par `text` (Ctrl+A → Ctrl+V).
///
/// Utilisé quand SR a déjà injecté et que l'utilisateur choisit le texte Whisper.
async fn replace_injected_text(
    text: String,
    target_window: Option<crate::platform::WindowHandle>,
    delay_ms: u64,
) {
    if text.is_empty() { return; }
    info!("Remplacement SR → «{}»", &text[..text.len().min(60)]);
    if let Err(e) = tokio::task::spawn_blocking(move || {
        if let Some(win) = target_window { win.refocus(); }
        select_all_then_paste(text, delay_ms)
    }).await {
        error!("replace_injected_text spawn_blocking : {e}");
    }
}

/// Envoie Ctrl+A pour sélectionner tout, puis colle le texte via clipboard + Ctrl+V.
#[cfg(target_os = "windows")]
fn select_all_then_paste(text: String, delay_ms: u64) -> crate::error::Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
    };
    const VK_A: u16 = 0x41; // Virtual key code for 'A'

    let make_key = |vk: u16, flags: u32| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };

    let ctrl_a = [
        make_key(VK_CONTROL, 0),
        make_key(VK_A, 0),
        make_key(VK_A, KEYEVENTF_KEYUP),
        make_key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    unsafe {
        SendInput(ctrl_a.len() as u32, ctrl_a.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
    std::thread::sleep(std::time::Duration::from_millis(80));

    // Maintenant coller le nouveau texte.
    let mut tw = Typewriter::new(delay_ms);
    tw.enqueue(text);
    tw.flush_all().map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn select_all_then_paste(text: String, delay_ms: u64) -> crate::error::Result<()> {
    let mut tw = Typewriter::new(delay_ms);
    tw.enqueue(text);
    tw.flush_all().map(|_| ())
}

fn resolve_whisper_cli() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let same_dir = exe_dir.join("whisper-cli.exe");
    if same_dir.exists() { return same_dir; }

    let resources_subdir = exe_dir.join("resources").join("whisper-cli.exe");
    if resources_subdir.exists() { return resources_subdir; }

    if let Some(dev_path) = exe_dir.ancestors().find_map(|p| {
        let candidate = p.join("src-tauri").join("resources").join("whisper-cli.exe");
        if candidate.exists() { Some(candidate) } else { None }
    }) {
        return dev_path;
    }

    same_dir
}
