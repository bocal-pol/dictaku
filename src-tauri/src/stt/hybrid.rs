use std::path::PathBuf;
use tracing::{info, warn};

use crate::config::settings::Language;
use crate::error::Result;
use crate::injection::corrections::{CorrectionSource, Corrections};
use crate::stt::whisper::WhisperTranscriber;
use crate::stt::windows_sr::{self, SrResult};

/// Seuil de confiance Windows SR en dessous duquel on force la comparaison avec Whisper.
const SR_CONFIDENCE_THRESHOLD: f32 = 0.85;

/// Résultat du moteur hybride.
#[derive(Debug, Clone)]
pub enum HybridResult {
    /// Les deux moteurs concordent — injection directe, pas de popup.
    Concordant { text: String },
    /// Les deux moteurs divergent — l'agent doit choisir.
    Divergent {
        sr_text: String,
        whisper_text: String,
    },
    /// Seul Whisper a produit un résultat (SR indisponible ou confiance trop faible).
    WhisperOnly { text: String },
    /// Seul SR a produit un résultat (Whisper désactivé ou timeout).
    SrOnly { text: String },
}

/// Transcrit en parallèle via Windows SR + Whisper, compare et retourne le résultat.
///
/// Pipeline :
///   1. Windows SR (thread bloquant) — rapide < 1s
///   2. Whisper (thread bloquant) — lent 3-8s, lancé en parallèle
///   3. Comparaison : si concordant → HybridResult::Concordant
///                   si divergent  → HybridResult::Divergent (popup agent)
///   4. Le filtre corrections.json est appliqué sur le résultat final validé.
pub fn transcribe_hybrid(
    samples: Vec<f32>,
    language: Language,
    cli_path: PathBuf,
    model_path: PathBuf,
) -> Result<HybridResult> {
    let samples_sr = samples.clone();
    let lang_sr = language.clone();

    // Lance Windows SR et Whisper en parallèle via std::thread.
    let sr_handle = std::thread::spawn(move || {
        windows_sr::transcribe_windows_sr(&samples_sr, &lang_sr)
    });

    let whisper_handle = std::thread::spawn(move || {
        if !cli_path.exists() || !model_path.exists() {
            return Ok(None);
        }
        let transcriber = WhisperTranscriber::new(cli_path, model_path, language);
        transcriber.transcribe(&samples).map(|t| {
            if t.is_empty() { None } else { Some(t) }
        })
    });

    let sr_result = sr_handle.join()
        .unwrap_or_else(|_| Ok(None))
        .unwrap_or_else(|e| { warn!("Windows SR erreur : {e}"); None });

    let whisper_result = whisper_handle.join()
        .unwrap_or_else(|_| Ok(None))
        .unwrap_or_else(|e| { warn!("Whisper erreur : {e}"); None });

    match (sr_result, whisper_result) {
        (Some(sr), Some(whisper)) => {
            let sr_norm = normalize(&sr.text);
            let whisper_norm = normalize(&whisper);

            if sr_norm == whisper_norm || sr.confidence >= SR_CONFIDENCE_THRESHOLD && similarity(&sr_norm, &whisper_norm) > 0.90 {
                info!("Hybride : concordance — SR choisi (confiance {:.2})", sr.confidence);
                Ok(HybridResult::Concordant { text: sr.text })
            } else {
                info!(
                    "Hybride : divergence — SR={:?} Whisper={:?} (confiance {:.2})",
                    &sr.text[..sr.text.len().min(40)],
                    &whisper[..whisper.len().min(40)],
                    sr.confidence
                );
                Ok(HybridResult::Divergent {
                    sr_text: sr.text,
                    whisper_text: whisper,
                })
            }
        }
        (Some(sr), None) => {
            info!("Hybride : SR only (confiance {:.2})", sr.confidence);
            Ok(HybridResult::SrOnly { text: sr.text })
        }
        (None, Some(whisper)) => {
            info!("Hybride : Whisper only");
            Ok(HybridResult::WhisperOnly { text: whisper })
        }
        (None, None) => {
            warn!("Hybride : aucun résultat des deux moteurs");
            Ok(HybridResult::Concordant { text: String::new() })
        }
    }
}

/// Enregistre le choix de l'agent (SR ou Whisper) et met à jour le dictionnaire.
pub fn record_choice(
    sr_text: &str,
    chosen_text: &str,
    corrections: &mut Corrections,
) {
    if sr_text != chosen_text {
        corrections.add(sr_text, chosen_text, CorrectionSource::WhisperChosen);
    }
}

/// Normalise un texte pour la comparaison (lowercase, trim, ponctuation).
fn normalize(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calcule une similarité simple entre deux textes normalisés (Jaccard sur bigrammes).
fn similarity(a: &str, b: &str) -> f32 {
    let bigrams_a: std::collections::HashSet<(char, char)> = bigrams(a);
    let bigrams_b: std::collections::HashSet<(char, char)> = bigrams(b);

    if bigrams_a.is_empty() && bigrams_b.is_empty() {
        return 1.0;
    }
    if bigrams_a.is_empty() || bigrams_b.is_empty() {
        return 0.0;
    }

    let intersection = bigrams_a.intersection(&bigrams_b).count();
    let union = bigrams_a.union(&bigrams_b).count();
    intersection as f32 / union as f32
}

fn bigrams(text: &str) -> std::collections::HashSet<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}
