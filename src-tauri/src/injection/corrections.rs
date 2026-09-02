use directories::ProjectDirs;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Dictionnaire de corrections persisté dans `~/.dictaku/corrections.json`.
///
/// Chaque entrée est une paire (texte_sr → texte_validé) enregistrée quand
/// l'agent choisit la version Whisper ou corrige manuellement la transcription.
/// Le filtre est appliqué sur chaque transcription avant injection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Corrections {
    /// Clé : texte tel que produit par Windows SR (lowercase pour matching souple).
    /// Valeur : texte validé par l'agent.
    entries: HashMap<String, CorrectionEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorrectionEntry {
    /// Texte corrigé validé par l'agent.
    pub corrected: String,
    /// Nombre de fois que cette correction a été appliquée.
    pub count: u32,
    /// Source de la correction.
    pub source: CorrectionSource,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CorrectionSource {
    /// L'agent a choisi la version Whisper plutôt que Windows SR.
    WhisperChosen,
    /// L'agent a corrigé manuellement le texte.
    Manual,
}

impl Corrections {
    /// Chemin du fichier de corrections.
    pub fn default_path() -> PathBuf {
        ProjectDirs::from("com", "dictaku", "dictaku")
            .map(|p| p.config_dir().join("corrections.json"))
            .unwrap_or_else(|| PathBuf::from("corrections.json"))
    }

    /// Charge le dictionnaire depuis le fichier JSON.
    pub fn load() -> Self {
        let path = Self::default_path();
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                warn!("Corrections JSON invalide ({e}) — réinitialisation");
                Self::default()
            }),
            Err(e) => {
                warn!("Lecture corrections.json : {e}");
                Self::default()
            }
        }
    }

    /// Sauvegarde le dictionnaire.
    pub fn save(&self) {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    warn!("Sauvegarde corrections.json : {e}");
                }
            }
            Err(e) => warn!("Sérialisation corrections : {e}"),
        }
    }

    /// Enregistre une correction (SR → corrigé).
    pub fn add(&mut self, sr_text: &str, corrected: &str, source: CorrectionSource) {
        let key = sr_text.to_lowercase();
        let entry = self.entries.entry(key).or_insert(CorrectionEntry {
            corrected: corrected.to_string(),
            count: 0,
            source,
        });
        entry.corrected = corrected.to_string();
        entry.count += 1;
        info!("Correction enregistrée : {:?} → {:?} (×{})", sr_text, corrected, entry.count);
        self.save();
    }

    /// Applique le filtre de corrections sur un texte.
    ///
    /// Remplace les occurrences connues (matching insensible à la casse)
    /// par leurs versions corrigées. Retourne le texte filtré.
    pub fn apply(&self, text: &str) -> String {
        if self.entries.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();
        // Trier par longueur décroissante pour éviter les remplacements partiels.
        let mut sorted: Vec<(&String, &CorrectionEntry)> = self.entries.iter().collect();
        sorted.sort_by_key(|a| Reverse(a.0.len()));

        for (key, entry) in &sorted {
            // Remplacement insensible à la casse, mot entier uniquement.
            let lower = result.to_lowercase();
            if let Some(pos) = lower.find(key.as_str()) {
                let original = &result[pos..pos + key.len()];
                // Préserve la casse de début si le mot corrigé commence par majuscule.
                let replacement = if original.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && entry.corrected.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
                {
                    let mut chars = entry.corrected.chars();
                    match chars.next() {
                        None => entry.corrected.clone(),
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                } else {
                    entry.corrected.clone()
                };
                result = result[..pos].to_string() + &replacement + &result[pos + key.len()..];
                debug!("Correction appliquée : {:?} → {:?}", key, replacement);
            }
        }

        result
    }

    /// Retourne les N termes les plus fréquemment corrigés pour enrichir le prompt Whisper.
    pub fn top_terms(&self, n: usize) -> Vec<String> {
        let mut sorted: Vec<&CorrectionEntry> = self.entries.values().collect();
        sorted.sort_by_key(|a| Reverse(a.count));
        sorted.iter().take(n).map(|e| e.corrected.clone()).collect()
    }
}
