use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleRate, Stream, StreamConfig};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Wrapper pour rendre `cpal::Stream` envoyable entre threads.
///
/// SAFETY : `cpal::Stream` est !Send par précaution conservative, mais les
/// opérations effectuées ici (drop) sont thread-safe sur Windows (WASAPI).
/// Ce wrapper est uniquement utilisé pour transférer le stream vers un thread
/// de gardiennage dont le seul rôle est de le maintenir en vie puis de le dropper.
struct SendableStream(Stream);
unsafe impl Send for SendableStream {}

use crate::error::{DictakuError, Result};

/// Format audio requis par Whisper.cpp — 16kHz mono f32.
const TARGET_SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;

/// Taille du chunk audio envoyé sur le channel (en frames).
/// 512 frames @ 16kHz = 32ms de latence par chunk.
const CHUNK_SIZE: usize = 512;

/// Configuration du VAD (Voice Activity Detection) basé sur l'énergie RMS.
pub struct VadConfig {
    /// Seuil RMS en dessous duquel on considère le signal comme silence.
    pub threshold: f32,
    /// Durée de silence consécutif avant arrêt automatique.
    pub silence_duration: Duration,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.01,
            silence_duration: Duration::from_millis(1500),
        }
    }
}

/// Calcule l'énergie RMS d'un chunk de samples.
///
/// RMS = sqrt(mean(x²)) — mesure l'énergie sonore perçue.
/// Plus rapide et suffisant pour la VAD comparé à la FFT.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Capture audio en temps réel depuis le microphone par défaut.
///
/// Pipeline :
///   cpal callback → buffer partagé → thread VAD → mpsc channel
///
/// Le thread VAD lit le buffer, détecte les silences > `silence_duration`,
/// et retourne les samples PCM 16kHz mono f32 via le channel.
pub struct AudioRecorder {
    vad: VadConfig,
}

impl AudioRecorder {
    pub fn new(vad: VadConfig) -> Self {
        Self { vad }
    }

    /// Lance la capture et retourne un channel recevant les chunks audio.
    ///
    /// Le sender se ferme automatiquement quand :
    /// - le silence dépasse `vad.silence_duration`
    /// - `stop_signal` est activé (via le `Arc<Mutex<bool>>` retourné)
    pub fn start_recording(
        &self,
    ) -> Result<(
        mpsc::Receiver<Vec<f32>>,
        Arc<Mutex<bool>>, // stop_signal
    )> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| DictakuError::AudioCapture("Aucun microphone disponible".into()))?;

        info!("Microphone sélectionné : {}", device.name().unwrap_or_default());

        let (config, native_rate) = build_stream_config(&device)?;
        let native_channels = config.channels;
        debug!("Config audio : {:?} (native {}Hz, target {}Hz)", config, native_rate, TARGET_SAMPLE_RATE);

        let (tx, rx) = mpsc::channel::<Vec<f32>>(32);
        let stop_signal = Arc::new(Mutex::new(false));

        // Buffer partagé entre le callback cpal (thread RT) et le thread VAD.
        // On l'accède via Mutex standard (non-async) car le callback cpal
        // est synchrone et doit rester léger.
        let shared_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

        let buffer_clone = shared_buffer.clone();
        let stop_clone = stop_signal.clone();
        let vad_threshold = self.vad.threshold;
        let silence_duration = self.vad.silence_duration;

        // Callback cpal — exécuté dans un thread temps-réel OS.
        // Doit être minimal : pas d'allocation, pas de mutex contention longue.
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _info| {
                    // Resampling inline si nécessaire (native_rate → 16kHz).
                    let resampled = if native_rate != TARGET_SAMPLE_RATE || native_channels > 1 {
                        resample_to_16k(data, native_rate, native_channels)
                    } else {
                        data.to_vec()
                    };
                    if let Ok(mut buf) = buffer_clone.lock() {
                        buf.extend_from_slice(&resampled);
                    }
                },
                move |err| {
                    tracing::error!("Erreur flux audio cpal : {err}");
                },
                None,
            )
            .map_err(|e| DictakuError::AudioCapture(e.to_string()))?;

        stream.play().map_err(|e| DictakuError::AudioCapture(e.to_string()))?;

        // `cpal::Stream` n'impl pas `Send` — on l'enveloppe dans `SendableStream`
        // pour le transférer vers un thread gardien dont le seul rôle est de le
        // maintenir en vie jusqu'à la fin du thread VAD, puis de le dropper.
        let sendable_stream = SendableStream(stream);
        let (stream_drop_tx, stream_drop_rx) = std::sync::mpsc::sync_channel::<()>(0);
        std::thread::spawn(move || {
            let _ = stream_drop_rx.recv();
            drop(sendable_stream);
        });

        // Thread VAD — lit le buffer, envoie les chunks, détecte le silence.
        let stop_vad = stop_signal.clone();
        std::thread::spawn(move || {
            // `last_voice_at` reste None tant qu'aucune voix n'a été détectée.
            // Le timer de silence ne démarre qu'après le premier sample de voix.
            let mut last_voice_at: Option<Instant> = None;
            let mut chunk_acc: Vec<f32> = Vec::with_capacity(CHUNK_SIZE);

            loop {
                // Vérification du signal d'arrêt externe (annulation par l'utilisateur).
                if *stop_vad.lock().unwrap() {
                    debug!("VAD : signal d'arrêt reçu");
                    break;
                }

                // Récupération des samples depuis le buffer partagé.
                let new_samples: Vec<f32> = {
                    let mut buf = shared_buffer.lock().unwrap();
                    let drained: Vec<f32> = buf.drain(..).collect();
                    drained
                };

                if !new_samples.is_empty() {
                    let energy = rms(&new_samples);

                    if energy > vad_threshold {
                        // Voix détectée — mise à jour du timestamp.
                        last_voice_at = Some(Instant::now());
                    } else if let Some(last) = last_voice_at {
                        if last.elapsed() > silence_duration {
                            // Silence prolongé après voix — fin de la dictée.
                            info!("VAD : silence détecté après voix — arrêt de la capture");
                            if !chunk_acc.is_empty() {
                                let _ = tx.blocking_send(chunk_acc.clone());
                            }
                            break;
                        }
                    }

                    chunk_acc.extend_from_slice(&new_samples);

                    // Envoi par chunks de taille fixe pour pipeline fluide.
                    while chunk_acc.len() >= CHUNK_SIZE {
                        let chunk: Vec<f32> = chunk_acc.drain(..CHUNK_SIZE).collect();
                        if tx.blocking_send(chunk).is_err() {
                            // Le receiver est fermé — arrêt propre.
                            debug!("VAD : receiver fermé, arrêt");
                            // Signal au gardien de stream de dropper.
                            let _ = stream_drop_tx.send(());
                            return;
                        }
                    }
                }

                // Pause courte pour ne pas saturer le CPU.
                // 10ms = latence max acceptable pour la VAD temps réel.
                std::thread::sleep(Duration::from_millis(10));
            }
            // Signal au gardien de stream de dropper proprement.
            let _ = stream_drop_tx.send(());
        });

        Ok((rx, stop_signal))
    }
}

/// Construit une `StreamConfig` au taux natif du device.
///
/// On capture au taux supporté par le device puis on rééchantillonne à 16kHz
/// pour Whisper. Cela évite l'erreur "stream configuration not supported".
fn build_stream_config(device: &Device) -> Result<(StreamConfig, u32)> {
    let supported: Vec<_> = device
        .supported_input_configs()
        .map_err(|e| DictakuError::AudioCapture(format!("Configs supportées : {e}")))?
        .collect();

    // Priorité 1 : 16kHz mono exact
    for range in &supported {
        if range.channels() == CHANNELS
            && range.min_sample_rate().0 <= TARGET_SAMPLE_RATE
            && range.max_sample_rate().0 >= TARGET_SAMPLE_RATE
        {
            let native_rate = TARGET_SAMPLE_RATE;
            return Ok((StreamConfig {
                channels: CHANNELS,
                sample_rate: SampleRate(native_rate),
                buffer_size: cpal::BufferSize::Default,
            }, native_rate));
        }
    }

    // Priorité 2 : mono à n'importe quel taux (on resamplre ensuite)
    for range in &supported {
        if range.channels() == CHANNELS {
            let native_rate = range.max_sample_rate().0;
            info!("16kHz non supporté — capture à {}Hz + resampling", native_rate);
            return Ok((StreamConfig {
                channels: CHANNELS,
                sample_rate: SampleRate(native_rate),
                buffer_size: cpal::BufferSize::Default,
            }, native_rate));
        }
    }

    // Priorité 3 : stéréo (on mixe les canaux)
    for range in &supported {
        let native_rate = range.max_sample_rate().0;
        let channels = range.channels();
        info!("Capture stéréo {}ch à {}Hz + downmix + resampling", channels, native_rate);
        return Ok((StreamConfig {
            channels,
            sample_rate: SampleRate(native_rate),
            buffer_size: cpal::BufferSize::Default,
        }, native_rate));
    }

    Err(DictakuError::AudioCapture("Aucune configuration audio supportée".into()))
}

/// Rééchantillonne des samples f32 de `from_rate` Hz vers 16kHz mono.
fn resample_to_16k(samples: &[f32], from_rate: u32, channels: u16) -> Vec<f32> {
    // Downmix stéréo → mono
    let mono: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples.to_vec()
    };

    if from_rate == TARGET_SAMPLE_RATE {
        return mono;
    }

    let params = SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 64,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = TARGET_SAMPLE_RATE as f64 / from_rate as f64;
    let chunk_size = mono.len().max(1);

    let mut resampler = match SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1) {
        Ok(r) => r,
        Err(e) => {
            warn!("Resampler init échoué ({e}) — retour mono sans resampling");
            return mono;
        }
    };

    match resampler.process(&[&mono], None) {
        Ok(out) => out.into_iter().next().unwrap_or_default(),
        Err(e) => {
            warn!("Resampling échoué ({e}) — retour mono sans resampling");
            mono
        }
    }
}
