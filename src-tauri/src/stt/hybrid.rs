use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::config::settings::Language;
use crate::error::Result;
use crate::injection::corrections::{CorrectionSource, Corrections};
use crate::stt::whisper::WhisperTranscriber;
use crate::stt::windows_sr;

/// Seuil de similarité Jaccard au-dessus duquel les deux moteurs sont considérés concordants.
const SIMILARITY_THRESHOLD: f32 = 0.82;

/// Résultat du moteur hybride.
#[derive(Debug, Clone)]
pub enum HybridResult {
    /// Les deux moteurs concordent — injection déjà faite par SR, rien à faire.
    Concordant { text: String },
    /// Les deux moteurs divergent — popup pour proposer le remplacement.
    Divergent {
        /// Ce que SR a déjà injecté dans le champ.
        sr_text: String,
        /// Ce que Whisper small a transcrit (probablement plus précis).
        whisper_text: String,
    },
    /// Seul Whisper small a produit un résultat (SR indisponible ou vide).
    PreciseOnly { text: String },
    /// Seul SR a produit un résultat (Whisper absent ou timeout).
    FastOnly { text: String },
}

/// Paramètres pour lancer le moteur hybride SR + Whisper.
pub struct HybridParams {
    pub samples: Vec<f32>,
    pub language: Language,
    pub cli_path: PathBuf,
    pub model_path: PathBuf,
    pub models_dir: PathBuf,
    /// Texte déjà injecté par SR (accumulé pendant la dictée).
    pub sr_injected: String,
}

/// Transcrit via Whisper small et compare avec le texte SR déjà injecté.
///
/// SR a déjà écrit dans le champ actif pendant la dictée.
/// Cette fonction vérifie si Whisper confirme ou diverge.
pub fn compare_with_whisper(params: HybridParams) -> Result<HybridResult> {
    let sr_text = params.sr_injected.trim().to_string();

    // Lancer Whisper small sur les samples capturés.
    let whisper_result = {
        let cli = params.cli_path.clone();
        let model = params.model_path.clone();
        let lang = params.language.clone();
        let samples = params.samples.clone();

        if !cli.exists() || !model.exists() {
            warn!("Whisper small absent — mode FastOnly");
            return Ok(HybridResult::FastOnly { text: sr_text });
        }

        let transcriber = WhisperTranscriber::new(cli, model, lang);
        match transcriber.transcribe(&samples) {
            Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
            Ok(_) => {
                info!("Whisper small : résultat vide — mode FastOnly");
                return Ok(HybridResult::FastOnly { text: sr_text });
            }
            Err(e) => {
                warn!("Whisper small erreur : {e} — mode FastOnly");
                return Ok(HybridResult::FastOnly { text: sr_text });
            }
        }
    };

    // Si SR n'a rien produit mais Whisper oui → PreciseOnly.
    if sr_text.is_empty() {
        info!("SR vide, Whisper OK → PreciseOnly");
        return Ok(HybridResult::PreciseOnly { text: whisper_result });
    }

    let sr_norm      = normalize(&sr_text);
    let whisper_norm = normalize(&whisper_result);
    let sim          = similarity(&sr_norm, &whisper_norm);

    if sim >= SIMILARITY_THRESHOLD {
        info!("Hybride : concordance (sim={:.2}) — SR validé par Whisper", sim);
        Ok(HybridResult::Concordant { text: sr_text })
    } else {
        info!(
            "Hybride : divergence (sim={:.2})\n  SR      : {:?}\n  Whisper : {:?}",
            sim,
            &sr_text[..sr_text.len().min(60)],
            &whisper_result[..whisper_result.len().min(60)],
        );
        Ok(HybridResult::Divergent {
            sr_text,
            whisper_text: whisper_result,
        })
    }
}

/// Transcrit en parallèle via le moteur de fallback (Whisper tiny + small).
///
/// Utilisé quand Windows SR est indisponible.
pub fn transcribe_hybrid(
    samples: Vec<f32>,
    language: Language,
    cli_path: PathBuf,
    model_path: PathBuf,
    models_dir: PathBuf,
) -> Result<HybridResult> {
    let samples_fast = samples.clone();
    let lang_fast    = language.clone();
    let cli_fast     = cli_path.clone();
    let mdir         = models_dir.clone();

    // Tiny en parallèle avec Small.
    let fast_handle = std::thread::spawn(move || {
        windows_sr::transcribe_windows_sr(&samples_fast, &lang_fast, &cli_fast, &mdir)
    });

    let precise_handle = std::thread::spawn(move || {
        if !cli_path.exists() || !model_path.exists() {
            return Ok(None);
        }
        let transcriber = WhisperTranscriber::new(cli_path, model_path, language);
        transcriber.transcribe(&samples).map(|t| {
            if t.is_empty() { None } else { Some(t) }
        })
    });

    let fast_result = fast_handle.join()
        .unwrap_or_else(|_| Ok(None))
        .unwrap_or_else(|e| { warn!("Moteur rapide erreur : {e}"); None });

    let precise_result = precise_handle.join()
        .unwrap_or_else(|_| Ok(None))
        .unwrap_or_else(|e| { warn!("Whisper small erreur : {e}"); None });

    match (fast_result, precise_result) {
        (Some(fast), Some(precise)) => {
            let fast_norm    = normalize(&fast.text);
            let precise_norm = normalize(&precise);
            let sim          = similarity(&fast_norm, &precise_norm);

            if sim >= SIMILARITY_THRESHOLD {
                info!("Hybride fallback : concordance (sim={:.2})", sim);
                Ok(HybridResult::Concordant { text: fast.text })
            } else {
                info!("Hybride fallback : divergence (sim={:.2})", sim);
                Ok(HybridResult::Divergent {
                    sr_text: fast.text,
                    whisper_text: precise,
                })
            }
        }
        (Some(fast), None)     => Ok(HybridResult::FastOnly { text: fast.text }),
        (None, Some(precise))  => Ok(HybridResult::PreciseOnly { text: precise }),
        (None, None)           => {
            warn!("Hybride fallback : aucun résultat");
            Ok(HybridResult::Concordant { text: String::new() })
        }
    }
}

/// Enregistre le choix de l'agent dans le dictionnaire de corrections.
pub fn record_choice(sr_text: &str, chosen_text: &str, corrections: &mut Corrections) {
    if sr_text != chosen_text {
        corrections.add(sr_text, chosen_text, CorrectionSource::WhisperChosen);
    }
}

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

fn similarity(a: &str, b: &str) -> f32 {
    let ba: std::collections::HashSet<(char, char)> = bigrams(a);
    let bb: std::collections::HashSet<(char, char)> = bigrams(b);
    if ba.is_empty() && bb.is_empty() { return 1.0; }
    if ba.is_empty() || bb.is_empty() { return 0.0; }
    let inter = ba.intersection(&bb).count();
    let union  = ba.union(&bb).count();
    inter as f32 / union as f32
}

fn bigrams(text: &str) -> std::collections::HashSet<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}
