use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::time::Duration;
use tempfile::NamedTempFile;
use tracing::{debug, info, warn};

use crate::config::settings::Language;
use crate::error::{DictakuError, Result};

/// Timeout pour l'exécution de whisper-cli.exe.
///
/// 120s pour couvrir le modèle small sur CPU sans GPU (~4-5x temps réel).
/// Un fragment de 30s audio peut prendre jusqu'à 2 minutes en small/CPU.
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 120;

/// Prompt d'amorçage injecté dans Whisper pour orienter la reconnaissance vers
/// le vocabulaire policier belge francophone. Améliore significativement la
/// précision sur les termes juridiques, sigles et procédures.
const POLICE_PROMPT: &str = "Rapport de police belge francophone. \
    Termes courants : procès-verbal, PV, prévenu, inculpé, plaignant, déclarant, \
    témoin, audition, instruction judiciaire, parquet, substitut, magistrat, \
    zone de police, commissariat, officier de police judiciaire, OPJ, \
    intervention, patrouille, réquisition, perquisition, saisie, \
    mise en état d'arrestation, garde à vue, liberté sous conditions, \
    procureur du Roi, juge d'instruction, tribunal correctionnel, \
    DGA, BRI, PJF, CGSU, DJSOC, OCAM.";

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
            POLICE_PROMPT,
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
        debug!("whisper-cli lignes raw : {:?}", stdout.lines().collect::<Vec<_>>());

        // Filtrage des lignes de méta-données et hallucinations Whisper.
        // Whisper peut produire une ligne avec [Musique] ou [BLANK_AUDIO] en suffixe —
        // on filtre les balises inline avec une regex simple avant le filtre ligne.
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
                    && !is_hallucination(l)
            })
            .map(|line| {
                // Retire les balises inline entre crochets : [Musique], [BLANK_AUDIO]…
                let mut out = String::new();
                let mut depth = 0usize;
                for ch in line.trim().chars() {
                    match ch {
                        '[' => { depth += 1; }
                        ']' => { depth = depth.saturating_sub(1); }
                        c if depth == 0 => out.push(c),
                        _ => {}
                    }
                }
                out
            })
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        // Supprime les points de suspension parasites en fin de transcription.
        // Whisper hallucine souvent des `…` (U+2026) ou `...` à la fin d'un
        // segment audio qui se termine brusquement.
        let trimmed = strip_trailing_ellipsis(text.trim());
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
    prompt: &str,
    timeout: Duration,
) -> Result<std::process::Output> {
    // whisper-cli produit le .txt dans le même dossier que le WAV d'entrée.
    // Flags utilisés :
    //   --model     : chemin du modèle GGML
    //   --language  : code langue ISO (auto, fr, en, nl)
    //   --file      : fichier WAV d'entrée
    //   --no-timestamps : supprime les horodatages sur chaque ligne
    //   --prompt    : amorce le contexte pour orienter la reconnaissance vocabulaire
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
            "--prompt",
            prompt,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        // Cache la fenêtre console sur Windows — sans ça whisper-cli.exe ouvre
        // une fenêtre noire visible pendant la transcription.
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
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

/// Supprime les séquences de points de suspension parasites en fin de texte.
///
/// Whisper ajoute souvent `…` (U+2026) ou `...` à la fin d'un segment audio
/// qui se coupe brusquement. Cette fonction les retire sans toucher au texte
/// réel qui précède.
fn strip_trailing_ellipsis(text: &str) -> String {
    let mut result = text.to_string();
    loop {
        let trimmed = result.trim_end();
        if trimmed.ends_with('…') || trimmed.ends_with('.') {
            // Retire le dernier caractère et recommence.
            let char_boundary = trimmed.char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            result = trimmed[..char_boundary].to_string();
        } else {
            break;
        }
    }
    // Supprime aussi les séquences mixtes en fin (ex: "... " ou "… …")
    result.trim_end_matches(['.', '…', ' ']).to_string()
}

/// Détecte les hallucinations Whisper typiques.
///
/// Whisper hallucine souvent sur du silence ou du bruit : caractères répétés
/// ("!!!!!!", "......"), phrases musicales, remerciements génériques.
fn is_hallucination(text: &str) -> bool {
    if text.len() < 3 {
        return false;
    }
    // Répétition excessive d'un même caractère (ex: "!!!!!!!", "......")
    let chars: Vec<char> = text.chars().collect();
    let repeated = chars.windows(4).any(|w| w[0] == w[1] && w[1] == w[2] && w[2] == w[3]);
    if repeated {
        return true;
    }
    // Phrases de remplissage génériques que Whisper génère sur du silence
    let fillers = [
        "sous-titres réalisés", "sous-titrage", "merci d'avoir regardé",
        "abonnez-vous", "transcription", "music", "♪",
    ];
    let lower = text.to_lowercase();
    fillers.iter().any(|f| lower.contains(f))
}
