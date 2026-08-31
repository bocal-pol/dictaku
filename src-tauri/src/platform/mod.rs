//! Traits et utilitaires spécifiques à la plateforme.
//!
//! v0.1 : Windows uniquement — la couche platform prépare
//! l'extension cross-platform en v0.2+ (macOS CGEventPost, X11/Wayland).

/// Handle opaque vers une fenêtre OS — utilisé pour restaurer le focus avant injection.
#[derive(Debug, Clone, Copy)]
pub struct WindowHandle(
    #[cfg(target_os = "windows")] windows_sys::Win32::Foundation::HWND,
    #[cfg(not(target_os = "windows"))] (),
);

impl WindowHandle {
    /// Retourne le handle de la fenêtre actuellement au premier plan.
    pub fn foreground() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            let hwnd = unsafe { GetForegroundWindow() };
            if hwnd == 0 { None } else { Some(Self(hwnd)) }
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }

    /// Remet cette fenêtre au premier plan pour que `enigo` y injecte le texte.
    ///
    /// Un petit délai de 50ms après le focus est nécessaire — certaines apps
    /// (Notepad, Word) ont besoin d'un cycle message pour traiter le focus avant
    /// d'accepter les `SendInput`.
    pub fn refocus(&self) {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};
            unsafe {
                ShowWindow(self.0, SW_RESTORE);
                SetForegroundWindow(self.0);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// Retourne le nom de la fenêtre actuellement au premier plan.
///
/// Windows : `GetForegroundWindow` + `GetWindowTextW`
/// Autres  : retourne None (non implémenté)
pub fn foreground_window_title() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == 0 { return None; }
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            if len == 0 { return None; }
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
