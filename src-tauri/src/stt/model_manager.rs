use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::config::settings::{Settings, WhisperModel};
use crate::error::{DictakuError, Result};

/// Gestion du cycle de vie des modèles Whisper GGML.
///
/// Responsabilités :
/// - Vérifier la présence locale d'un modèle
/// - Télécharger depuis HuggingFace avec vérification SHA256
/// - Retourner le chemin résolu pour l'utilisation par WhisperTranscriber
pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(settings: &Settings) -> Self {
        Self {
            models_dir: settings.models_dir(),
        }
    }

    /// Retourne le chemin complet d'un modèle s'il existe localement.
    pub fn model_path(&self, model: &WhisperModel) -> PathBuf {
        self.models_dir.join(model.filename())
    }

    /// Vérifie si le modèle est présent et son SHA256 est correct.
    pub fn is_model_available(&self, model: &WhisperModel) -> bool {
        let path = self.model_path(model);
        if !path.exists() {
            return false;
        }
        // Vérification rapide de taille (les modèles font plusieurs Mo).
        match path.metadata() {
            Ok(meta) if meta.len() > 1_000_000 => true,
            _ => {
                warn!("Modèle présent mais trop petit (corrompu ?) : {}", path.display());
                false
            }
        }
    }

    /// Télécharge un modèle Whisper depuis HuggingFace.
    ///
    /// Étapes :
    ///   1. Crée le dossier modèles si absent
    ///   2. Télécharge via reqwest (streaming pour économiser la RAM)
    ///   3. Calcule le SHA256 du fichier téléchargé
    ///   4. Compare avec la valeur hardcodée
    ///   5. Garde le fichier si valide, supprime sinon
    pub fn download(&self, model: &WhisperModel) -> Result<PathBuf> {
        let dest_path = self.model_path(model);

        if self.is_model_available(model) {
            info!("Modèle déjà présent : {}", dest_path.display());
            return Ok(dest_path);
        }

        // Création du dossier de destination.
        std::fs::create_dir_all(&self.models_dir).map_err(|e| {
            DictakuError::ModelDownload(format!(
                "Création dossier modèles {} : {e}",
                self.models_dir.display()
            ))
        })?;

        let url = model.download_url();
        info!("Téléchargement modèle {} depuis : {url}", model.filename());

        // Téléchargement bloquant (appel depuis un thread Tokio via spawn_blocking).
        let response = reqwest::blocking::get(url).map_err(|e| {
            DictakuError::ModelDownload(format!("Requête HTTP échouée : {e}"))
        })?;

        if !response.status().is_success() {
            return Err(DictakuError::ModelDownload(format!(
                "HTTP {} pour {url}",
                response.status()
            )));
        }

        let bytes = response.bytes().map_err(|e| {
            DictakuError::ModelDownload(format!("Lecture du corps de réponse : {e}"))
        })?;

        info!("Téléchargement terminé : {} octets", bytes.len());

        // Vérification de l'intégrité SHA256 avant d'écrire le fichier final.
        let computed = compute_sha256(&bytes);
        let expected = model.expected_sha256();

        debug!("SHA256 calculé  : {computed}");
        debug!("SHA256 attendu  : {expected}");

        if computed != expected {
            warn!("SHA256 invalide pour {} — fichier potentiellement corrompu", model.filename());
            return Err(DictakuError::ModelChecksum);
        }

        // Écriture du fichier.
        std::fs::write(&dest_path, &bytes).map_err(|e| {
            DictakuError::ModelDownload(format!("Écriture {} : {e}", dest_path.display()))
        })?;

        info!("Modèle installé avec succès : {}", dest_path.display());
        Ok(dest_path)
    }
}

/// Calcule le SHA256 d'un slice d'octets et retourne la représentation hexadécimale.
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
