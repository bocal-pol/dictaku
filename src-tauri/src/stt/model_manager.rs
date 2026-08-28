use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::config::settings::{Settings, WhisperModel};
use crate::error::{DictakuError, Result};

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(settings: &Settings) -> Self {
        Self {
            models_dir: settings.models_dir(),
        }
    }

    pub fn model_path(&self, model: &WhisperModel) -> PathBuf {
        self.models_dir.join(model.filename())
    }

    pub fn is_model_available(&self, model: &WhisperModel) -> bool {
        let path = self.model_path(model);
        if !path.exists() {
            return false;
        }
        match path.metadata() {
            Ok(meta) if meta.len() > 1_000_000 => true,
            _ => {
                warn!("Modèle présent mais trop petit (corrompu ?) : {}", path.display());
                false
            }
        }
    }

    /// Télécharge un modèle avec suivi de progression.
    ///
    /// `on_progress(percent: u8, downloaded_mb: f64, total_mb: f64)` est appelé
    /// régulièrement pendant le téléchargement pour mettre à jour l'UI.
    pub fn download_with_progress<F>(
        &self,
        model: &WhisperModel,
        mut on_progress: F,
    ) -> Result<PathBuf>
    where
        F: FnMut(u8, f64, f64),
    {
        let dest_path = self.model_path(model);

        if self.is_model_available(model) {
            info!("Modèle déjà présent : {}", dest_path.display());
            on_progress(100, 0.0, 0.0);
            return Ok(dest_path);
        }

        std::fs::create_dir_all(&self.models_dir).map_err(|e| {
            DictakuError::ModelDownload(format!(
                "Création dossier modèles {} : {e}",
                self.models_dir.display()
            ))
        })?;

        let url = model.download_url();
        info!("Téléchargement modèle {} depuis : {url}", model.filename());

        let mut response = reqwest::blocking::get(url).map_err(|e| {
            DictakuError::ModelDownload(format!("Requête HTTP échouée : {e}"))
        })?;

        if !response.status().is_success() {
            return Err(DictakuError::ModelDownload(format!(
                "HTTP {} pour {url}",
                response.status()
            )));
        }

        let total = response.content_length().unwrap_or(0);
        let total_mb = total as f64 / 1_048_576.0;

        // Téléchargement streaming vers un fichier temporaire.
        let tmp_path = dest_path.with_extension("bin.tmp");
        let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
            DictakuError::ModelDownload(format!("Création fichier temporaire : {e}"))
        })?;

        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;
        let mut last_pct: u8 = 0;
        let mut buf = vec![0u8; 65_536]; // chunks de 64 KB

        loop {
            let n = std::io::Read::read(&mut response, &mut buf).map_err(|e| {
                DictakuError::ModelDownload(format!("Lecture flux HTTP : {e}"))
            })?;
            if n == 0 {
                break;
            }
            let chunk = &buf[..n];
            file.write_all(chunk).map_err(|e| {
                DictakuError::ModelDownload(format!("Écriture fichier : {e}"))
            })?;
            hasher.update(chunk);
            downloaded += n as u64;

            if total > 0 {
                let pct = ((downloaded * 100) / total) as u8;
                if pct != last_pct {
                    last_pct = pct;
                    let dl_mb = downloaded as f64 / 1_048_576.0;
                    on_progress(pct, dl_mb, total_mb);
                }
            }
        }

        file.flush().map_err(|e| {
            DictakuError::ModelDownload(format!("Flush fichier : {e}"))
        })?;
        drop(file);

        // Vérification SHA256.
        let computed = hex::encode(hasher.finalize());
        let expected = model.expected_sha256();
        debug!("SHA256 calculé : {computed}");
        debug!("SHA256 attendu : {expected}");

        if computed != expected {
            let _ = std::fs::remove_file(&tmp_path);
            warn!("SHA256 invalide pour {}", model.filename());
            return Err(DictakuError::ModelChecksum);
        }

        std::fs::rename(&tmp_path, &dest_path).map_err(|e| {
            DictakuError::ModelDownload(format!("Déplacement fichier final : {e}"))
        })?;

        on_progress(100, total_mb, total_mb);
        info!("Modèle installé : {}", dest_path.display());
        Ok(dest_path)
    }

    /// Version sans progression (compatibilité).
    pub fn download(&self, model: &WhisperModel) -> Result<PathBuf> {
        self.download_with_progress(model, |_, _, _| {})
    }
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
