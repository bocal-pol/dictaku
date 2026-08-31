use enigo::{Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{debug, info};

use crate::error::{DictakuError, Result};

pub struct Typewriter {
    delay_ms: u64,
    queue: VecDeque<String>,
}

impl Typewriter {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            queue: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, text: String) {
        debug!("Texte mis en file : {} caractères", text.len());
        self.queue.push_back(text);
    }

    pub fn flush_next(&mut self) -> Result<bool> {
        let Some(text) = self.queue.pop_front() else {
            return Ok(false);
        };
        self.type_text(&text)?;
        Ok(true)
    }

    pub fn flush_all(&mut self) -> Result<usize> {
        let mut count = 0;
        while !self.queue.is_empty() {
            if self.flush_next()? {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Injecte le texte via clipboard + Ctrl+V.
    ///
    /// Stratégie : écrire dans le presse-papier Windows puis simuler Ctrl+V.
    /// Beaucoup plus fiable qu'envoyer des SendInput caractère par caractère —
    /// fonctionne indépendamment du focus et du layout clavier.
    pub fn type_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        info!("Injection clipboard : {} caractères", text.len());

        // 1. Écrire le texte dans le presse-papier Windows.
        set_clipboard_text(text)?;

        // 2. Petit délai pour que le clipboard soit prêt.
        std::thread::sleep(Duration::from_millis(50));

        // 3. Simuler Ctrl+V dans la fenêtre active.
        let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| {
            DictakuError::Injection(format!("Initialisation enigo : {e}"))
        })?;

        enigo.key(Key::Control, enigo::Direction::Press).map_err(|e| {
            DictakuError::Injection(format!("Ctrl press : {e}"))
        })?;
        enigo.key(Key::Unicode('v'), enigo::Direction::Click).map_err(|e| {
            DictakuError::Injection(format!("V click : {e}"))
        })?;
        enigo.key(Key::Control, enigo::Direction::Release).map_err(|e| {
            DictakuError::Injection(format!("Ctrl release : {e}"))
        })?;

        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }

        debug!("Injection terminée");
        Ok(())
    }
}

/// Écrit du texte dans le presse-papier Windows via l'API Win32.
#[cfg(target_os = "windows")]
fn set_clipboard_text(text: &str) -> Result<()> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    // Encodage UTF-16 LE avec null-terminator.
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0u16)).collect();
    let byte_len = utf16.len() * 2;

    unsafe {
        // Alloue un bloc mémoire global déplaçable.
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if hmem.is_null() {
            return Err(DictakuError::Injection("GlobalAlloc échoué".into()));
        }

        // Verrouille pour écrire les données.
        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            return Err(DictakuError::Injection("GlobalLock échoué".into()));
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
        GlobalUnlock(hmem);

        // Ouvre le presse-papier, vide, puis insère.
        if OpenClipboard(0 as HANDLE) == 0 {
            return Err(DictakuError::Injection("OpenClipboard échoué".into()));
        }
        EmptyClipboard();
        SetClipboardData(CF_UNICODETEXT as u32, hmem as HANDLE);
        CloseClipboard();
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_clipboard_text(_text: &str) -> Result<()> {
    Err(DictakuError::Injection("Clipboard non supporté sur cette plateforme".into()))
}
