use tracing::{debug, info};

use crate::config::settings::Language;
use crate::error::Result;

/// Résultat du moteur rapide.
#[derive(Debug, Clone)]
pub struct SrResult {
    pub text: String,
    /// Confiance simulée — toujours 1.0 pour Whisper tiny (pas de score natif).
    pub confidence: f32,
}

/// Moteur "rapide" : Whisper tiny lancé via whisper-cli.
///
/// Tiny (~39 MB, ~10x temps réel) est utilisé comme substitut de Windows SR :
/// rapide, offline, pas de dépendance WinRT. Retourne None si tiny est absent.
pub fn transcribe_windows_sr(
    samples: &[f32],
    language: &Language,
    cli_path: &std::path::Path,
    models_dir: &std::path::Path,
) -> Result<Option<SrResult>> {
    use crate::stt::whisper::WhisperTranscriber;

    let tiny_path = models_dir.join("ggml-tiny.bin");
    if !tiny_path.exists() {
        debug!("Moteur rapide : ggml-tiny.bin absent — moteur rapide désactivé");
        return Ok(None);
    }

    info!("Moteur rapide (tiny) : {} samples", samples.len());

    let transcriber = WhisperTranscriber::new(
        cli_path.to_path_buf(),
        tiny_path,
        language.clone(),
    );

    match transcriber.transcribe(samples) {
        Ok(text) if !text.trim().is_empty() => {
            info!("Moteur rapide : {:?}", &text[..text.len().min(60)]);
            Ok(Some(SrResult { text, confidence: 1.0 }))
        }
        Ok(_) => Ok(None),
        Err(e) => {
            debug!("Moteur rapide erreur : {e}");
            Ok(None)
        }
    }
}
