use std::path::PathBuf;
use tracing::{info, warn};

use crate::config::settings::Language;
use crate::error::Result;
use crate::injection::corrections::{CorrectionSource, Corrections};
use crate::stt::whisper::WhisperTranscriber;
use crate::stt::windows_sr;

/// Seuil de similarité Jaccard au-dessus duquel les deux moteurs sont considérés concordants.
const SIMILARITY_THRESHOLD: f32 = 0.90;

/// Résultat du moteur hybride.
#[derive(Debug, Clone)]
pub enum HybridResult {
    /// Les deux moteurs concordent — injection directe, pas de popup.
    Concordant { text: String },
    /// Les deux moteurs divergent — l'agent doit choisir.
    Divergent { fast_text: String, precise_text: String },
    /// Seul Whisper small a produit un résultat (tiny absent).
    PreciseOnly { text: String },
    /// Seul tiny a produit un résultat (small absent ou timeout).
    FastOnly { text: String },
}

/// Transcrit en parallèle via Whisper tiny (rapide) + Whisper small (précis).
///
/// Pipeline :
///   1. Tiny  — rapide (~1-2s), moins précis
///   2. Small — lent  (~5-8s), vocabulaire policier
///   3. Si concordant (Jaccard > 90%) → injection directe
///   4. Si divergent → popup de comparaison
pub fn transcribe_hybrid(
    samples: Vec<f32>,
    language: Language,
    cli_path: PathBuf,
    model_path: PathBuf,
    models_dir: PathBuf,
) -> Result<HybridResult> {
    let samples_fast = samples.clone();
    let lang_fast = language.clone();
    let cli_fast = cli_path.clone();
    let mdir = models_dir.clone();

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
            let sim = similarity(&fast_norm, &precise_norm);

            if sim >= SIMILARITY_THRESHOLD {
                info!("Hybride : concordance (sim={:.2}) — tiny choisi", sim);
                Ok(HybridResult::Concordant { text: fast.text })
            } else {
                info!(
                    "Hybride : divergence (sim={:.2}) — tiny={:?} small={:?}",
                    sim,
                    &fast.text[..fast.text.len().min(40)],
                    &precise[..precise.len().min(40)],
                );
                Ok(HybridResult::Divergent {
                    fast_text: fast.text,
                    precise_text: precise,
                })
            }
        }
        (Some(fast), None) => {
            info!("Hybride : tiny only");
            Ok(HybridResult::FastOnly { text: fast.text })
        }
        (None, Some(precise)) => {
            info!("Hybride : small only");
            Ok(HybridResult::PreciseOnly { text: precise })
        }
        (None, None) => {
            warn!("Hybride : aucun résultat");
            Ok(HybridResult::Concordant { text: String::new() })
        }
    }
}

/// Enregistre le choix de l'agent dans le dictionnaire de corrections.
pub fn record_choice(fast_text: &str, chosen_text: &str, corrections: &mut Corrections) {
    if fast_text != chosen_text {
        corrections.add(fast_text, chosen_text, CorrectionSource::WhisperChosen);
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
    let union = ba.union(&bb).count();
    inter as f32 / union as f32
}

fn bigrams(text: &str) -> std::collections::HashSet<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}
