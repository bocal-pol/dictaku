//! Traits et utilitaires spécifiques à la plateforme.
//!
//! v0.1 : Windows uniquement — la couche platform prépare
//! l'extension cross-platform en v0.2+ (macOS CGEventPost, X11/Wayland).

/// Retourne le nom de la fenêtre actuellement au premier plan.
///
/// Windows : `GetForegroundWindow` + `GetWindowTextW`
/// Autres  : retourne None (non implémenté)
pub fn foreground_window_title() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        windows_foreground_title()
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn windows_foreground_title() -> Option<String> {
    // TODO: implémenter via windows-sys crate
    // GetForegroundWindow() + GetWindowTextW()
    None
}
