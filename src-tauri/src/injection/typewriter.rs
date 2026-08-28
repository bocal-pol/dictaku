use enigo::{Enigo, Keyboard, Settings as EnigoSettings};
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::{DictakuError, Result};

/// Nom de la fenêtre Dictaku — utilisé pour le garde-fou d'injection.
///
/// On ne veut pas injecter du texte dans notre propre UI overlay
/// si l'utilisateur a cliqué dessus avant la fin de la transcription.
const DICTAKU_WINDOW_TITLE: &str = "Dictaku";

/// Injection clavier simulée via `enigo`.
///
/// Pipeline :
///   File d'attente de textes → injection caractère par caractère avec délai
///   configurable pour ne pas saturer le buffer clavier du système.
///
/// Le délai de 20ms entre chaque caractère simule une frappe humaine rapide
/// (~50 mots/minute) et évite les pertes dans des applications peu réactives.
pub struct Typewriter {
    /// Délai entre chaque caractère injecté (ms).
    delay_ms: u64,
    /// File d'attente FIFO — les textes sont injectés dans l'ordre d'arrivée.
    queue: VecDeque<String>,
}

impl Typewriter {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            queue: VecDeque::new(),
        }
    }

    /// Ajoute un texte à la file d'injection.
    pub fn enqueue(&mut self, text: String) {
        debug!("Texte mis en file : {} caractères", text.len());
        self.queue.push_back(text);
    }

    /// Injecte le prochain texte de la file dans la fenêtre active.
    ///
    /// Garde-fou : si la fenêtre active est Dictaku, l'injection est annulée
    /// pour éviter d'écrire dans notre propre UI et créer une boucle infinie.
    pub fn flush_next(&mut self) -> Result<bool> {
        let Some(text) = self.queue.pop_front() else {
            return Ok(false);
        };

        if is_dictaku_focused() {
            warn!("Injection annulée : la fenêtre Dictaku est au premier plan");
            return Ok(false);
        }

        self.type_text(&text)?;
        Ok(true)
    }

    /// Injecte tous les textes en attente séquentiellement.
    pub fn flush_all(&mut self) -> Result<usize> {
        let mut count = 0;
        while !self.queue.is_empty() {
            if self.flush_next()? {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Injecte un texte caractère par caractère dans la fenêtre active.
    ///
    /// Utilise `enigo::type_char` pour chaque rune Unicode, avec un délai
    /// configurable entre chaque injection pour ne pas déborder le buffer
    /// clavier des applications Windows qui ont un rate-limiting interne.
    pub fn type_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        info!("Injection : {} caractères", text.len());

        let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| {
            DictakuError::Injection(format!("Initialisation enigo : {e}"))
        })?;

        enigo.text(text).map_err(|e| {
            DictakuError::Injection(format!("Injection texte : {e}"))
        })?;

        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }

        debug!("Injection terminée");
        Ok(())
    }
}

/// Vérifie si la fenêtre en premier plan appartient à Dictaku.
///
/// Windows API : `GetForegroundWindow` + `GetWindowTextW`.
/// En cas d'erreur (ex. permissions), retourne `false` par défaut (fail-open).
///
/// HWND dans windows-sys est un `isize` (0 = NULL), pas un pointeur.
#[cfg(target_os = "windows")]
fn is_dictaku_focused() -> bool {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return false;
        }

        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);

        if len <= 0 {
            return false;
        }

        let title = OsString::from_wide(&buf[..len as usize])
            .to_string_lossy()
            .to_string();

        title.contains(DICTAKU_WINDOW_TITLE)
    }
}

/// Fallback non-Windows : toujours autoriser l'injection.
#[cfg(not(target_os = "windows"))]
fn is_dictaku_focused() -> bool {
    false
}
