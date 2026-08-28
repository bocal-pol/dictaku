use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, info};

use crate::config::settings::{Settings, WhisperModel};
use crate::state::app_state::{AppState, DictationState};
use crate::stt::model_manager::ModelManager;

/// Retourne l'état courant du pipeline de dictée.
///
/// Réponse : "idle" | "listening" | "transcribing" | "injecting"
#[tauri::command]
pub async fn get_state(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let current = state.current_state().await;
    debug!("IPC get_state → {current}");
    Ok(current.to_string())
}

/// Toggle le pipeline de dictée : Idle → Listening ou Listening/Transcribing → Idle.
///
/// Lance le pipeline complet en arrière-plan via tokio::spawn :
///   1. Transition Idle → Listening
///   2. Capture audio (AudioRecorder)
///   3. Transition Listening → Transcribing
///   4. Transcription (WhisperTranscriber)
///   5. Transition Transcribing → Injecting
///   6. Injection (Typewriter)
///   7. Transition Injecting → Idle
///
/// En cas d'annulation (appel toggle en cours de Listening/Transcribing),
/// le stop_signal est levé et on retourne à Idle.
#[tauri::command]
pub async fn toggle_dictation(
    state: tauri::State<'_, AppState>,
    _app: AppHandle,
) -> Result<(), String> {
    let current = state.current_state().await;

    match current {
        DictationState::Idle => {
            info!("IPC toggle_dictation : démarrage dictée");
            state
                .transition(DictationState::Listening)
                .await
                .map_err(|e| e.to_string())?;

            // TODO: lancer le pipeline complet en background (audio + STT + injection)
            // Émettre l'événement "dictaku://state-changed" vers la WebView.
        }
        DictationState::Listening | DictationState::Transcribing => {
            info!("IPC toggle_dictation : annulation dictée");
            state
                .transition(DictationState::Idle)
                .await
                .map_err(|e| e.to_string())?;

            // TODO: lever le stop_signal du pipeline en cours
        }
        DictationState::Injecting => {
            // L'injection est courte — on ignore les toggle pendant ce temps.
            debug!("IPC toggle_dictation ignoré : injection en cours");
        }
    }

    Ok(())
}

/// Retourne la configuration actuelle.
#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    let config = state.config.lock().await.clone();
    debug!("IPC get_settings");
    Ok(config)
}

/// Sauvegarde la configuration et met à jour l'AppState.
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("IPC save_settings : modèle={:?}, langue={}", settings.model, settings.language);

    settings.save().map_err(|e| e.to_string())?;

    let mut config = state.config.lock().await;
    *config = settings;

    Ok(())
}

/// Lance le téléchargement d'un modèle Whisper en tâche de fond.
///
/// Émet des événements `dictaku://download-progress` vers la WebView :
/// `{ percent: u8, downloaded_mb: f64, total_mb: f64 }`
#[tauri::command]
pub async fn download_model(
    model: String,
    state: tauri::State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let whisper_model = parse_model(&model)?;
    info!("IPC download_model : {model}");

    let config = state.config.lock().await.clone();
    let manager = ModelManager::new(&config);

    tokio::task::spawn_blocking(move || {
        manager.download_with_progress(&whisper_model, |percent, downloaded_mb, total_mb| {
            let _ = app.emit(
                "dictaku://download-progress",
                serde_json::json!({
                    "percent": percent,
                    "downloaded_mb": downloaded_mb,
                    "total_mb": total_mb,
                }),
            );
        })
    })
    .await
    .map_err(|e| format!("Erreur interne spawn_blocking : {e}"))?
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Ferme la fenêtre de setup initial et bascule en mode tray.
#[tauri::command]
pub async fn close_setup_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("setup") {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Vérifie si un modèle Whisper est disponible localement.
#[tauri::command]
pub async fn check_model_exists(
    model: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let whisper_model = parse_model(&model)?;
    let config = state.config.lock().await.clone();
    let manager = ModelManager::new(&config);
    let exists = manager.is_model_available(&whisper_model);
    debug!("IPC check_model_exists({model}) → {exists}");
    Ok(exists)
}

/// Parse une string de modèle ("tiny", "base", "small") en `WhisperModel`.
fn parse_model(s: &str) -> Result<WhisperModel, String> {
    match s.to_lowercase().as_str() {
        "tiny" => Ok(WhisperModel::Tiny),
        "base" => Ok(WhisperModel::Base),
        "small" => Ok(WhisperModel::Small),
        other => Err(format!("Modèle inconnu : '{other}' — valeurs valides : tiny, base, small")),
    }
}
