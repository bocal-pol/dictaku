use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::settings::Settings;
use crate::error::{DictakuError, Result};

/// Machine à états du pipeline de dictée.
///
/// Transitions valides :
///   Idle → Listening      (raccourci appuyé, mic ouvert)
///   Listening → Transcribing (VAD silence détecté ou raccourci relâché)
///   Transcribing → Injecting (transcription prête)
///   Injecting → Idle      (injection terminée)
///   Listening → Idle      (annulation par l'utilisateur)
///   Transcribing → Idle   (annulation ou timeout)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictationState {
    Idle,
    Listening,
    Transcribing,
    Injecting,
}

impl std::fmt::Display for DictationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DictationState::Idle => write!(f, "idle"),
            DictationState::Listening => write!(f, "listening"),
            DictationState::Transcribing => write!(f, "transcribing"),
            DictationState::Injecting => write!(f, "injecting"),
        }
    }
}

/// État global partagé entre tous les composants de l'application.
///
/// Wrappé dans `Arc<Mutex<...>>` pour être passé aux threads cpal,
/// aux commandes Tauri IPC et au handler de raccourci global.
pub struct AppState {
    pub state: Arc<Mutex<DictationState>>,
    pub config: Arc<Mutex<Settings>>,
    /// Chemin résolu du modèle Whisper actif (None = modèle absent ou non sélectionné).
    pub model_path: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new(config: Settings) -> Self {
        Self {
            state: Arc::new(Mutex::new(DictationState::Idle)),
            config: Arc::new(Mutex::new(config)),
            model_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Effectue une transition d'état avec vérification de cohérence.
    ///
    /// Retourne une erreur si la transition est invalide selon le graphe
    /// d'états documenté ci-dessus, afin d'éviter des états incohérents
    /// au niveau de l'UI et du pipeline audio.
    pub async fn transition(&self, new_state: DictationState) -> Result<()> {
        let mut current = self.state.lock().await;

        let valid = match (&*current, &new_state) {
            (DictationState::Idle, DictationState::Listening) => true,
            (DictationState::Listening, DictationState::Transcribing) => true,
            (DictationState::Listening, DictationState::Idle) => true,
            (DictationState::Transcribing, DictationState::Injecting) => true,
            (DictationState::Transcribing, DictationState::Idle) => true,
            (DictationState::Injecting, DictationState::Idle) => true,
            _ => false,
        };

        if !valid {
            warn!(
                "Transition d'état invalide : {} → {}",
                current, new_state
            );
            return Err(DictakuError::InvalidStateTransition {
                from: current.to_string(),
                to: new_state.to_string(),
            });
        }

        info!("Transition d'état : {} → {}", current, new_state);
        *current = new_state;
        Ok(())
    }

    /// Retourne l'état courant sans le modifier.
    pub async fn current_state(&self) -> DictationState {
        self.state.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Settings;

    fn make_state() -> AppState {
        AppState::new(Settings::default())
    }

    #[tokio::test]
    async fn transition_idle_to_listening_ok() {
        let s = make_state();
        assert!(s.transition(DictationState::Listening).await.is_ok());
        assert_eq!(s.current_state().await, DictationState::Listening);
    }

    #[tokio::test]
    async fn transition_invalid_idle_to_injecting_err() {
        let s = make_state();
        let result = s.transition(DictationState::Injecting).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn full_pipeline_transitions_ok() {
        let s = make_state();
        s.transition(DictationState::Listening).await.unwrap();
        s.transition(DictationState::Transcribing).await.unwrap();
        s.transition(DictationState::Injecting).await.unwrap();
        s.transition(DictationState::Idle).await.unwrap();
        assert_eq!(s.current_state().await, DictationState::Idle);
    }

    #[tokio::test]
    async fn cancel_from_listening_ok() {
        let s = make_state();
        s.transition(DictationState::Listening).await.unwrap();
        s.transition(DictationState::Idle).await.unwrap();
        assert_eq!(s.current_state().await, DictationState::Idle);
    }
}
