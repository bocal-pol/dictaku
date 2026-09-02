/// Types d'erreur centralisés de Dictaku.
///
/// Chaque variante correspond à un domaine fonctionnel du pipeline
/// Hotkey → Audio → STT → Injection. Les erreurs sont sérialisables
/// pour être transmises à la WebView via les commandes Tauri IPC.
#[derive(Debug, thiserror::Error)]
pub enum DictakuError {
    #[error("Échec de la capture audio : {0}")]
    AudioCapture(String),

    #[error("Échec de la transcription : {0}")]
    Transcription(String),

    #[error("Erreur Windows Speech Recognition : {0}")]
    Stt(String),

    #[error("Délai d'attente de transcription dépassé ({0}s)")]
    TranscriptionTimeout(u64),

    #[error("Échec de l'injection clavier : {0}")]
    Injection(String),

    #[error("Erreur de configuration : {0}")]
    Config(String),

    #[error("Modèle Whisper introuvable : {path}")]
    ModelNotFound { path: String },

    #[error("Téléchargement du modèle échoué : {0}")]
    ModelDownload(String),

    #[error("Vérification SHA256 échouée — fichier corrompu ou différent du modèle attendu")]
    ModelChecksum,

    #[error("Enregistrement du raccourci global échoué : {0}")]
    HotkeyRegistration(String),

    #[error("Transition d'état invalide : {from:?} → {to:?}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Erreur système : {0}")]
    Io(#[from] std::io::Error),
}

/// Alias pratique utilisé dans les modules internes de Dictaku.
pub type Result<T> = std::result::Result<T, DictakuError>;

/// Implémentation de `serde::Serialize` pour renvoyer les erreurs
/// via les commandes Tauri IPC (Result<T, DictakuError>).
impl serde::Serialize for DictakuError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
