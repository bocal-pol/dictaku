pub mod hybrid;
pub mod model_manager;
pub mod whisper;
pub mod windows_sr;

pub use hybrid::{compare_with_whisper, transcribe_hybrid, HybridParams, HybridResult};
pub use model_manager::ModelManager;
pub use whisper::WhisperTranscriber;
pub use windows_sr::{is_sr_available, start_sr_session, transcribe_windows_sr, SrSession};
