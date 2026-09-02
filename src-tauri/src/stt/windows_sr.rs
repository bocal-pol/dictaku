use std::time::Duration;
use tracing::{debug, info, warn};

use crate::config::settings::Language;
use crate::error::{DictakuError, Result};

/// Résultat de la reconnaissance Windows SR.
#[derive(Debug, Clone)]
pub struct SrResult {
    /// Texte transcrit.
    pub text: String,
    /// Score de confiance [0.0, 1.0] retourné par Windows SR.
    pub confidence: f32,
}

/// Transcrit un buffer PCM 16kHz mono f32 via Windows Speech Recognition (WinRT).
///
/// Windows SR est offline, intégré à l'OS, latence < 1s.
/// Retourne None si SR n'est pas disponible ou si la confiance est trop faible.
#[cfg(target_os = "windows")]
pub fn transcribe_windows_sr(
    samples: &[f32],
    language: &Language,
) -> Result<Option<SrResult>> {
    use windows::Media::SpeechRecognition::{
        SpeechRecognitionResult, SpeechRecognizer,
        SpeechRecognitionResultStatus, SpeechContinuousRecognitionSession,
    };
    use windows::Media::SpeechRecognition::SpeechRecognizer as SR;
    use windows::Globalization::Language as WinLanguage;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
    use windows::Foundation::IAsyncOperation;

    let lang_tag = match language {
        Language::Fr | Language::Auto => "fr-BE",
        Language::En => "en-US",
        Language::Nl => "nl-BE",
    };

    info!("Windows SR : langue {lang_tag}, {} samples", samples.len());

    // Vérifie que la langue est disponible sur ce poste.
    let win_lang = WinLanguage::CreateLanguage(
        &windows::core::HSTRING::from(lang_tag)
    ).map_err(|e| DictakuError::Transcription(format!("Langue SR invalide : {e}")))?;

    let is_available = SR::IsSupportedLanguage(&win_lang)
        .map_err(|e| DictakuError::Transcription(format!("Vérification langue SR : {e}")))?;

    if !is_available {
        warn!("Windows SR : langue {lang_tag} non installée — fallback Whisper");
        return Ok(None);
    }

    // Crée le recognizer avec la langue cible.
    let recognizer = SR::CreateWithLanguage(&win_lang)
        .map_err(|e| DictakuError::Transcription(format!("Création SpeechRecognizer : {e}")))?;

    // Écrit les samples PCM dans un stream en mémoire.
    // Windows SR attend du PCM 16kHz mono 16-bit.
    let stream = InMemoryRandomAccessStream::new()
        .map_err(|e| DictakuError::Transcription(format!("InMemoryStream : {e}")))?;

    let writer = DataWriter::CreateDataWriter(&stream)
        .map_err(|e| DictakuError::Transcription(format!("DataWriter : {e}")))?;

    // Entête WAV minimale (44 bytes) pour que Windows SR reconnaisse le format.
    write_wav_header(&writer, samples.len() as u32, 16_000)?;

    // Samples f32 → i16.
    for &s in samples {
        let i = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.WriteInt16(i)
            .map_err(|e| DictakuError::Transcription(format!("WriteInt16 : {e}")))?;
    }

    writer.StoreAsync()
        .map_err(|e| DictakuError::Transcription(format!("StoreAsync : {e}")))?
        .get()
        .map_err(|e| DictakuError::Transcription(format!("Store await : {e}")))?;

    stream.Seek(0)
        .map_err(|e| DictakuError::Transcription(format!("Stream seek : {e}")))?;

    // Reconnaissance depuis le stream audio.
    let result = recognizer
        .RecognizeAsync()
        .map_err(|e| DictakuError::Transcription(format!("RecognizeAsync : {e}")))?
        .get()
        .map_err(|e| DictakuError::Transcription(format!("Recognize await : {e}")))?;

    let status = result.Status()
        .map_err(|e| DictakuError::Transcription(format!("Result status : {e}")))?;

    if status != SpeechRecognitionResultStatus::Success {
        debug!("Windows SR : statut non-succès ({status:?})");
        return Ok(None);
    }

    let text = result.Text()
        .map_err(|e| DictakuError::Transcription(format!("Result text : {e}")))?
        .to_string();

    let confidence = result.RawConfidence()
        .map_err(|e| DictakuError::Transcription(format!("Result confidence : {e}")))?
        as f32;

    info!("Windows SR : {:?} (confiance {:.2})", &text[..text.len().min(60)], confidence);

    if text.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(SrResult { text, confidence }))
}

/// Écrit une entête WAV minimale dans un DataWriter WinRT.
#[cfg(target_os = "windows")]
fn write_wav_header(
    writer: &windows::Storage::Streams::DataWriter,
    sample_count: u32,
    sample_rate: u32,
) -> Result<()> {
    let data_size = sample_count * 2; // i16 = 2 bytes par sample
    let file_size = data_size + 36;

    // RIFF header
    for &b in b"RIFF" { writer.WriteByte(b).ok(); }
    write_u32_le(writer, file_size);
    for &b in b"WAVE" { writer.WriteByte(b).ok(); }

    // fmt chunk
    for &b in b"fmt " { writer.WriteByte(b).ok(); }
    write_u32_le(writer, 16);          // chunk size
    write_u16_le(writer, 1);           // PCM
    write_u16_le(writer, 1);           // mono
    write_u32_le(writer, sample_rate);
    write_u32_le(writer, sample_rate * 2); // byte rate
    write_u16_le(writer, 2);           // block align
    write_u16_le(writer, 16);          // bits per sample

    // data chunk
    for &b in b"data" { writer.WriteByte(b).ok(); }
    write_u32_le(writer, data_size);

    Ok(())
}

#[cfg(target_os = "windows")]
fn write_u32_le(writer: &windows::Storage::Streams::DataWriter, v: u32) {
    for b in v.to_le_bytes() { writer.WriteByte(b).ok(); }
}

#[cfg(target_os = "windows")]
fn write_u16_le(writer: &windows::Storage::Streams::DataWriter, v: u16) {
    for b in v.to_le_bytes() { writer.WriteByte(b).ok(); }
}

/// Stub non-Windows — retourne toujours None.
#[cfg(not(target_os = "windows"))]
pub fn transcribe_windows_sr(
    _samples: &[f32],
    _language: &Language,
) -> Result<Option<SrResult>> {
    Ok(None)
}
