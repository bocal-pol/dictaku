use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tempfile::NamedTempFile;
use tracing::{debug, info, warn};

use crate::config::settings::Language;
use crate::error::{DictakuError, Result};

/// Timeout pour l'exécution de whisper-cli.exe.
///
/// 30s est suffisant pour les modèles tiny/base/small sur CPU moderne.
/// Les fragments audio en v0.1 ne dépassent pas 30s (VAD arrête avant).
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 30;

/// Wrapper autour du binaire `whisper-cli.exe` précompilé.
///
/// v0.1 : appel CLI avec fichier WAV temporaire.
/// v0.2 : remplacer par les bindings `whisper-rs` pour éviter le round-trip disque.
pub struct WhisperTranscriber {
    /// Chemin vers `whisper-cli.exe` — dans `resources/` de l'app Tauri.
    cli_path: PathBuf,
    /// Chemin du modèle GGML actif.
    model_path: PathBuf,
    /// Langue de transcription.
    language: Language,
}

impl WhisperTranscriber {
    pub fn new(cli_path: PathBuf, model_path: PathBuf, language: Language) -> Self {
        Self {
            cli_path,
            model_path,
            language,
        }
    }

    /// Transcrit un buffer PCM 16kHz mono f32 vers du texte.
    ///
    /// Pipeline :
    ///   1. Convertit le Vec<f32> en fichier WAV temporaire
    ///   2. Appelle whisper-cli.exe avec timeout
    ///   3. Lit le fichier .txt produit
    ///   4. Nettoie les fichiers temporaires (NamedTempFile via RAII)
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        // Crée un fichier WAV temporaire dans %TEMP%.
        // NamedTempFile supprime le fichier à la fin du scope (RAII).
        let wav_file = NamedTempFile::new()
            .map_err(|e| DictakuError::Transcription(format!("Fichier WAV temp : {e}")))?;
        let wav_path = wav_file.path().with_extension("wav");

        // Écrit les samples f32 en WAV 16kHz mono via hound.
        write_wav(&wav_path, samples)?;
        debug!("WAV temporaire écrit : {} ({} samples)", wav_path.display(), samples.len());

        // Lance whisper-cli.exe avec timeout — le texte est lu sur stdout.
        let output = run_whisper_cli(
            &self.cli_path,
            &self.model_path,
            &wav_path,
            &self.language,
            Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS),
        )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("whisper-cli stderr : {stderr}");
            return Err(DictakuError::Transcription(format!(
                "whisper-cli exit code {} : {stderr}",
                output.status.code().unwrap_or(-1)
            )));
        }

        // whisper-cli écrit la transcription sur stdout.
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("whisper-cli stdout brut : {stdout:?}");

        // Filtrage des lignes de méta-données et hallucinations Whisper.
        let text = stdout
            .lines()
            .filter(|line| {
                let l = line.trim();
                !l.is_empty()
                    && !l.starts_with('[')      // [00:00:00.000 --> ...] ou [BLANK_AUDIO]
                    && !l.starts_with('(')      // (inaudible), (bruit)
                    && !l.starts_with('*')      // *cris*, *rires*
                    && !l.starts_with("whisper")
                    && !l.starts_with("system_info")
                    && !l.starts_with("main:")
            })
            .collect::<Vec<_>>()
            .join(" ");

        let trimmed = text.trim().to_string();
        info!("Transcription : {:?} ({} caractères)", &trimmed[..trimmed.len().min(80)], trimmed.len());
        Ok(trimmed)
    }
}

/// Écrit les samples f32 dans un fichier WAV 16kHz mono.
fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| DictakuError::Transcription(format!("Création WAV : {e}")))?;

    for &sample in samples {
        // Conversion f32 [-1.0, 1.0] → i16 [-32768, 32767].
        let s = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer
            .write_sample(s)
            .map_err(|e| DictakuError::Transcription(format!("Écriture sample WAV : {e}")))?;
    }

    writer
        .finalize()
        .map_err(|e| DictakuError::Transcription(format!("Finalisation WAV : {e}")))?;

    Ok(())
}

/// Exécute whisper-cli.exe avec un timeout via `wait_timeout`.
fn run_whisper_cli(
    cli: &Path,
    model: &Path,
    wav: &Path,
    language: &Language,
    timeout: Duration,
) -> Result<std::process::Output> {
    // whisper-cli produit le .txt dans le même dossier que le WAV d'entrée.
    // Flags utilisés :
    //   --model     : chemin du modèle GGML
    //   --language  : code langue ISO (auto, fr, en, nl)
    //   --file      : fichier WAV d'entrée
    //   --output-txt : produit <fichier>.txt
    //   --no-prints : supprime les logs verbeux de whisper.cpp sur stdout
    let lang_str = language.to_string();

    let mut child = Command::new(cli)
        .args([
            "--model",
            &model.to_string_lossy(),
            "--language",
            &lang_str,
            "--file",
            &wav.to_string_lossy(),
            "--no-timestamps",
            "--no-speech-thold", "0.6",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            DictakuError::Transcription(format!(
                "Impossible de lancer whisper-cli ({}) : {e}",
                cli.display()
            ))
        })?;

    // Attend la fin du processus avec timeout pour éviter un blocage infini.
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(DictakuError::TranscriptionTimeout(timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(DictakuError::Transcription(format!("wait whisper-cli : {e}")))
            }
        }
    }

    child
        .wait_with_output()
        .map_err(|e| DictakuError::Transcription(format!("Lecture output whisper-cli : {e}")))
}
