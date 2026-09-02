use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error, info};

use crate::config::settings::Language;
use crate::error::{DictakuError, Result};

/// Résultat du moteur Windows SR.
#[derive(Debug, Clone)]
pub struct SrResult {
    pub text: String,
    pub confidence: f32,
}

/// Handle vers une session SR active.
/// Doit être droppé pour arrêter la reconnaissance.
pub struct SrSession {
    /// Texte accumulé par la session (partagé avec le thread SR).
    pub accumulated: Arc<Mutex<String>>,
    /// Signal d'arrêt — passer à true pour stopper proprement.
    pub stop_flag: Arc<Mutex<bool>>,
    /// Handle du thread SR (join pour attendre la fin).
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SrSession {
    /// Arrête la session et attend que le thread se termine.
    pub fn stop(mut self) -> String {
        *self.stop_flag.lock().unwrap() = true;
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
        self.accumulated.lock().unwrap().clone()
    }
}

impl Drop for SrSession {
    fn drop(&mut self) {
        *self.stop_flag.lock().unwrap() = true;
        // thread join non bloquant en drop (le thread verra le flag)
    }
}

/// Démarre une session Windows SR sur le microphone par défaut.
///
/// Retourne un `SrSession` permettant :
/// - de lire le texte accumulé en temps réel (`session.accumulated`)
/// - d'injecter les fragments dans le champ actif via `inject_fn`
/// - d'arrêter proprement avec `session.stop()`
///
/// `inject_fn` est appelé dans le thread SR à chaque résultat partiel.
pub fn start_sr_session<F>(
    language: &Language,
    inject_fn: F,
) -> Result<SrSession>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    let accumulated = Arc::new(Mutex::new(String::new()));
    let stop_flag = Arc::new(Mutex::new(false));

    let acc_clone = accumulated.clone();
    let stop_clone = stop_flag.clone();
    let lang_tag = language.bcp47().to_string();

    let thread = std::thread::spawn(move || {
        if let Err(e) = run_sr_session(lang_tag, acc_clone, stop_clone, inject_fn) {
            error!("SR session erreur : {e}");
        }
    });

    Ok(SrSession {
        accumulated,
        stop_flag,
        thread: Some(thread),
    })
}

/// Boucle principale de la session WinRT SR (tourne dans un thread dédié).
#[cfg(target_os = "windows")]
fn run_sr_session<F>(
    lang_tag: String,
    accumulated: Arc<Mutex<String>>,
    stop_flag: Arc<Mutex<bool>>,
    inject_fn: F,
) -> Result<()>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    use windows::Globalization::Language as WinLanguage;
    use windows::Media::SpeechRecognition::{
        SpeechContinuousRecognitionResultGeneratedEventArgs,
        SpeechContinuousRecognitionSession,
        SpeechRecognizer,
        SpeechRecognitionTopicConstraint,
        SpeechRecognitionScenario,
    };
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    // WinRT SpeechRecognizer requiert COM initialisé sur le thread appelant.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    info!("SR WinRT : initialisation langue={lang_tag}");

    // Essaie avec la langue demandée, puis fallback vers la langue système.
    let lang_hstr = windows::core::HSTRING::from(lang_tag.as_str());
    let recognizer = if let Ok(lang) = WinLanguage::CreateLanguage(&lang_hstr) {
        match SpeechRecognizer::Create(&lang) {
            Ok(r) => {
                info!("SR WinRT : recognizer créé pour {lang_tag}");
                r
            }
            Err(e) => {
                info!("SR WinRT : langue {lang_tag} non disponible ({e}) — fallback langue système");
                SpeechRecognizer::new()
                    .map_err(|e2| DictakuError::Stt(format!("SpeechRecognizer (système) : {e2}")))?
            }
        }
    } else {
        info!("SR WinRT : tag {lang_tag} invalide — fallback langue système");
        SpeechRecognizer::new()
            .map_err(|e| DictakuError::Stt(format!("SpeechRecognizer (système) : {e}")))?
    };

    // Ajouter une contrainte de dictée topic pour la reconnaissance libre.
    // Sans contrainte explicite, WinRT peut ne produire aucun résultat.
    let constraints = recognizer.Constraints()
        .map_err(|e| DictakuError::Stt(format!("Constraints() : {e}")))?;

    let topic_constraint = SpeechRecognitionTopicConstraint::Create(
        SpeechRecognitionScenario::Dictation,
        &windows::core::HSTRING::from("dictation"),
    ).map_err(|e| DictakuError::Stt(format!("TopicConstraint : {e}")))?;

    constraints.Append(&topic_constraint)
        .map_err(|e| DictakuError::Stt(format!("Append constraint : {e}")))?;

    // Compiler les contraintes.
    recognizer.CompileConstraintsAsync()
        .map_err(|e| DictakuError::Stt(format!("CompileConstraints : {e}")))?
        .get()
        .map_err(|e| DictakuError::Stt(format!("CompileConstraints await : {e}")))?;
    info!("SR WinRT : contraintes compilées (dictée libre)");

    // Obtenir la session continue.
    let session: SpeechContinuousRecognitionSession = recognizer.ContinuousRecognitionSession()
        .map_err(|e| DictakuError::Stt(format!("ContinuousRecognitionSession : {e}")))?;

    // Enregistrer le handler de résultat.
    let acc_for_result = accumulated.clone();

    type SrEventHandler = windows::Foundation::TypedEventHandler<
        SpeechContinuousRecognitionSession,
        SpeechContinuousRecognitionResultGeneratedEventArgs,
    >;

    let _token = session.ResultGenerated(&SrEventHandler::new(
        move |_session, args| {
            debug!("SR handler invoqué");
            if let Some(args) = &*args {
                if let Ok(result) = args.Result() {
                    if let Ok(text) = result.Text() {
                        let fragment = text.to_string();
                        debug!("SR fragment reçu : {fragment:?}");
                        if !fragment.trim().is_empty() {
                            info!("SR fragment : {fragment:?}");
                            inject_fn(&fragment);
                            let mut acc = acc_for_result.lock().unwrap();
                            if !acc.is_empty() { acc.push(' '); }
                            acc.push_str(&fragment);
                        }
                    }
                }
            }
            Ok(())
        }
    )).map_err(|e| DictakuError::Stt(format!("ResultGenerated handler : {e}")))?;

    // Démarrer la session.
    session.StartAsync()
        .map_err(|e| DictakuError::Stt(format!("StartAsync : {e}")))?
        .get()
        .map_err(|e| DictakuError::Stt(format!("StartAsync await : {e}")))?;

    info!("SR WinRT : session démarrée");

    // Attendre le signal d'arrêt.
    loop {
        std::thread::sleep(Duration::from_millis(100));
        if *stop_flag.lock().unwrap() {
            break;
        }
    }

    // Arrêter proprement.
    let _ = session.StopAsync()
        .and_then(|op| op.get());

    info!("SR WinRT : session arrêtée");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn run_sr_session<F>(
    _lang_tag: String,
    _accumulated: Arc<Mutex<String>>,
    _stop_flag: Arc<Mutex<bool>>,
    _inject_fn: F,
) -> Result<()>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    Err(DictakuError::Stt("Windows SR non disponible sur cette plateforme".into()))
}

/// Vérifie si la reconnaissance vocale Windows est disponible.
pub fn is_sr_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Essai de création d'un recognizer par défaut — si ça échoue, SR n'est pas dispo.
        use windows::Media::SpeechRecognition::SpeechRecognizer;
        SpeechRecognizer::new().is_ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// API de compatibilité pour l'ancien code qui appelle transcribe_windows_sr.
/// Utilisé uniquement dans les chemins de fallback (SR indisponible).
pub fn transcribe_windows_sr(
    samples: &[f32],
    language: &Language,
    cli_path: &std::path::Path,
    models_dir: &std::path::Path,
) -> Result<Option<SrResult>> {
    use crate::stt::whisper::WhisperTranscriber;

    let tiny_path = models_dir.join("ggml-tiny.bin");
    if !tiny_path.exists() {
        debug!("Fallback SR : ggml-tiny.bin absent");
        return Ok(None);
    }

    info!("Fallback SR (Whisper tiny) : {} samples", samples.len());

    let transcriber = WhisperTranscriber::new(
        cli_path.to_path_buf(),
        tiny_path,
        language.clone(),
    );

    match transcriber.transcribe(samples) {
        Ok(text) if !text.trim().is_empty() => {
            info!("Fallback SR : {:?}", &text[..text.len().min(60)]);
            Ok(Some(SrResult { text, confidence: 1.0 }))
        }
        Ok(_) => Ok(None),
        Err(e) => {
            debug!("Fallback SR erreur : {e}");
            Ok(None)
        }
    }
}
