use tauri::{Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracing::{info, warn};

use crate::error::DictakuError;
use crate::state::app_state::{AppState, DictationState};

/// Raccourci global par défaut.
///
/// Format Tauri : modificateurs+touche en minuscules, séparés par `+`.
const DEFAULT_SHORTCUT: &str = "ctrl+alt+d";

/// Enregistre le raccourci global Ctrl+Alt+D.
///
/// Comportement toggle :
///   - Si Idle → déclenche le passage en Listening (début de dictée)
///   - Si Listening ou Transcribing → retour à Idle (annulation)
///
/// Le raccourci est lu depuis la configuration utilisateur au premier lancement,
/// puis depuis la valeur stockée dans `AppState.config`.
pub fn register_global_shortcut<R: Runtime>(app: &tauri::App<R>) -> Result<(), DictakuError> {

    let shortcut: Shortcut = DEFAULT_SHORTCUT
        .parse()
        .map_err(|_| DictakuError::HotkeyRegistration(format!("Raccourci invalide : {DEFAULT_SHORTCUT}")))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            // On ne réagit qu'à la pression (KeyPressed), pas au relâchement.
            if event.state() != ShortcutState::Pressed {
                return;
            }

            let state = _app.state::<AppState>();

            // Toggle dictée — exécuté dans un tokio::spawn pour ne pas bloquer
            // le thread du gestionnaire de raccourcis OS.
            let state_arc = state.state.clone();
            tauri::async_runtime::spawn(async move {
                let current = state_arc.lock().await.clone();
                match current {
                    DictationState::Idle => {
                        info!("Hotkey : déclenchement dictée");
                        // TODO: émettre un événement Tauri vers la WebView pour afficher la popup
                        // et lancer le pipeline audio via ipc::commands::toggle_dictation
                    }
                    DictationState::Listening | DictationState::Transcribing => {
                        info!("Hotkey : annulation dictée");
                        // TODO: annuler le pipeline en cours et retourner à Idle
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

/// Désenregistre le raccourci global (appelé lors du changement de raccourci dans les settings).
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
