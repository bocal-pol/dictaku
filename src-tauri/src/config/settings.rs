use directories::ProjectDirs;
use std::path::PathBuf;
use tracing::{debug, info};

use crate::error::{DictakuError, Result};

/// Langues supportées par Whisper.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Détection automatique par Whisper (coût CPU légèrement supérieur).
    Auto,
    #[default]
    Fr,
    En,
    Nl,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Auto => write!(f, "auto"),
            Language::Fr => write!(f, "fr"),
            Language::En => write!(f, "en"),
            Language::Nl => write!(f, "nl"),
        }
    }
}

/// Modèles Whisper GGML disponibles — compromis vitesse / précision.
///
/// Tiny  : ~39 MB, ~10x temps réel  — suffisant pour les commandes courtes
/// Base  : ~74 MB, ~7x temps réel   — bon équilibre (recommandé)
/// Small : ~244 MB, ~4x temps réel  — meilleure précision, accents difficiles
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhisperModel {
    Tiny,
    Base,
    #[default]
    Small,
}

impl WhisperModel {
    /// Nom de fichier GGML attendu dans le dossier modèles.
    pub fn filename(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => "ggml-tiny.bin",
            WhisperModel::Base => "ggml-base.bin",
            WhisperModel::Small => "ggml-small.bin",
        }
    }

    /// URL HuggingFace pour le téléchargement du modèle.
    pub fn download_url(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
            }
            WhisperModel::Base => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
            }
            WhisperModel::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
            }
        }
    }

    /// SHA256 hardcodé pour vérifier l'intégrité du modèle téléchargé.
    /// Ces valeurs correspondent aux checksum officiels publiés par ggerganov.
    pub fn expected_sha256(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => {
                "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21"
            }
            WhisperModel::Base => {
                "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe"
            }
            WhisperModel::Small => {
                "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"
            }
        }
    }
}

/// Configuration persistée dans `%APPDATA%\dictaku\config.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// Raccourci global. Format : "ctrl+alt+d".
    pub hotkey: String,

    /// Langue utilisée pour la transcription Whisper.
    pub language: Language,

    /// Modèle Whisper actif.
    pub model: WhisperModel,

    /// Dossier des modèles (None = `%APPDATA%\dictaku\models\`).
    pub model_dir: Option<String>,

    /// Délai entre chaque caractère injecté (ms). Défaut 20ms.
    pub injection_delay_ms: u64,

    /// Seuil RMS pour la VAD. Défaut 0.01.
    pub vad_threshold: f32,

    /// Durée de silence avant arrêt automatique de la capture (ms). Défaut 1500.
    pub vad_silence_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "ctrl+alt+d".to_string(),
            language: Language::default(),
            model: WhisperModel::default(),
            model_dir: None,
            injection_delay_ms: 20,
            vad_threshold: 0.01,
            vad_silence_ms: 2000,
        }
    }
}

impl Settings {
    /// Chemin du fichier de configuration selon les conventions OS.
    ///
    /// Windows : `%APPDATA%\dictaku\config.json`
    pub fn default_path() -> PathBuf {
        ProjectDirs::from("com", "dictaku", "dictaku")
            .map(|p| p.config_dir().join("config.json"))
            .unwrap_or_else(|| PathBuf::from("config.json"))
    }

    /// Dossier des modèles Whisper.
    ///
    /// Utilise `model_dir` si défini, sinon `%APPDATA%\dictaku\models\`.
    pub fn models_dir(&self) -> PathBuf {
        if let Some(dir) = &self.model_dir {
            PathBuf::from(dir)
        } else {
            ProjectDirs::from("com", "dictaku", "dictaku")
                .map(|p| p.data_dir().join("models"))
                .unwrap_or_else(|| PathBuf::from("models"))
        }
    }

    /// Charge la configuration depuis le fichier JSON.
    ///
    /// Si le fichier est absent, retourne les valeurs par défaut et crée
    /// le fichier pour les sessions suivantes.
    pub fn load() -> Result<Self> {
        let path = Self::default_path();
        debug!("Chargement config depuis : {}", path.display());

        if !path.exists() {
            info!(
                "Fichier de config absent — création avec les defaults : {}",
                path.display()
            );
            let defaults = Self::default();
            defaults.save()?;
            return Ok(defaults);
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| DictakuError::Config(format!("Lecture impossible : {e}")))?;

        // Retire le BOM UTF-8 (EF BB BF) si présent — produit par certains
        // outils Windows (PowerShell Set-Content, Notepad) qui écrivent en UTF-8 avec BOM.
        let content = content.strip_prefix('\u{FEFF}').unwrap_or(&content);

        let settings: Self = serde_json::from_str(content)
            .map_err(|e| DictakuError::Config(format!("JSON invalide : {e}")))?;

        info!("Config chargée : modèle={:?}, langue={}", settings.model, settings.language);
        Ok(settings)
    }

    /// Sauvegarde la configuration dans le fichier JSON.
    pub fn save(&self) -> Result<()> {
        let path = Self::default_path();

        // Crée le répertoire parent si absent.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DictakuError::Config(format!("Création dossier config : {e}")))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| DictakuError::Config(format!("Sérialisation JSON : {e}")))?;

        std::fs::write(&path, content)
            .map_err(|e| DictakuError::Config(format!("Écriture impossible : {e}")))?;

        debug!("Config sauvegardée : {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_valid() {
        let s = Settings::default();
        assert_eq!(s.hotkey, "ctrl+alt+d");
        assert_eq!(s.language, Language::Fr);
        assert_eq!(s.model, WhisperModel::Small);
        assert_eq!(s.injection_delay_ms, 20);
        assert!(s.vad_threshold > 0.0);
        assert!(s.vad_silence_ms > 0);
    }

    #[test]
    fn settings_round_trip_json() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).expect("serialisation OK");
        let s2: Settings = serde_json::from_str(&json).expect("désérialisation OK");
        assert_eq!(s.hotkey, s2.hotkey);
        assert_eq!(s.model, s2.model);
        assert_eq!(s.language, s2.language);
    }

    #[test]
    fn whisper_model_filenames_are_distinct() {
        assert_ne!(WhisperModel::Tiny.filename(), WhisperModel::Base.filename());
        assert_ne!(WhisperModel::Base.filename(), WhisperModel::Small.filename());
    }

    #[test]
    fn whisper_model_sha256_length() {
        // SHA256 hex = 64 caractères
        assert_eq!(WhisperModel::Tiny.expected_sha256().len(), 64);
        assert_eq!(WhisperModel::Base.expected_sha256().len(), 64);
        assert_eq!(WhisperModel::Small.expected_sha256().len(), 64);
    }
}
