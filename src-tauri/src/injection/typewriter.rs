use std::collections::VecDeque;
use std::time::Duration;
use tracing::{debug, error, info};

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
    pub fn type_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        info!("Injection clipboard : {} caractères", text.len());

        // 1. Écrire dans le clipboard.
        match set_clipboard_text(text) {
            Ok(()) => debug!("Clipboard rempli OK"),
            Err(e) => {
                error!("Échec clipboard : {e}");
                return Err(e);
            }
        }

        // 2. Délai pour que le clipboard et le focus soient stabilisés.
        std::thread::sleep(Duration::from_millis(200));

        // 3. Ctrl+V via SendInput.
        match paste_via_sendinput() {
            Ok(()) => debug!("SendInput Ctrl+V OK"),
            Err(e) => {
                error!("Échec SendInput : {e}");
                return Err(e);
            }
        }

        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }

        debug!("Injection terminée");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn set_clipboard_text(text: &str) -> Result<()> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0u16)).collect();
    let byte_len = utf16.len() * 2;

    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if hmem.is_null() {
            let err = windows_sys::Win32::Foundation::GetLastError();
            return Err(DictakuError::Injection(format!("GlobalAlloc échoué (err={err})")));
        }

        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            let err = windows_sys::Win32::Foundation::GetLastError();
            return Err(DictakuError::Injection(format!("GlobalLock échoué (err={err})")));
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
        GlobalUnlock(hmem);

        // NULL HWND = clipboard appartient au thread courant.
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            let err = windows_sys::Win32::Foundation::GetLastError();
            return Err(DictakuError::Injection(format!("OpenClipboard échoué (err={err})")));
        }
        EmptyClipboard();
        let result = SetClipboardData(CF_UNICODETEXT as u32, hmem);
        CloseClipboard();

        if result.is_null() {
            let err = windows_sys::Win32::Foundation::GetLastError();
            return Err(DictakuError::Injection(format!("SetClipboardData échoué (err={err})")));
        }
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_clipboard_text(_text: &str) -> Result<()> {
    Err(DictakuError::Injection("Clipboard non supporté sur cette plateforme".into()))
}

#[cfg(target_os = "windows")]
fn paste_via_sendinput() -> Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    let make_key = |vk: u16, flags: u32| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };

    let inputs = [
        make_key(VK_CONTROL, 0),
        make_key(VK_V, 0),
        make_key(VK_V, KEYEVENTF_KEYUP),
        make_key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };

    if sent != inputs.len() as u32 {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        return Err(DictakuError::Injection(format!(
            "SendInput : {sent}/{} events envoyés (err={err})",
            inputs.len()
        )));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn paste_via_sendinput() -> Result<()> {
    Err(DictakuError::Injection("SendInput non supporté sur cette plateforme".into()))
}
