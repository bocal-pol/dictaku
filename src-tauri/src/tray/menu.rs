use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tracing::{debug, info};

use crate::error::DictakuError;

/// SVG du tray icon en état Idle (microphone gris).
/// Embedé en base64 pour éviter un accès disque au démarrage.
const ICON_IDLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#9ca3af"><path d="M12 15a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.93V21h2v-2.07A7 7 0 0 0 19 12h-2z"/></svg>"##;

/// SVG du tray icon en état Listening (microphone vert animé).
const ICON_LISTENING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#22c55e"><path d="M12 15a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.93V21h2v-2.07A7 7 0 0 0 19 12h-2z"/></svg>"##;

/// SVG du tray icon en état Transcribing/Injecting (microphone orange).
const ICON_PROCESSING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#f97316"><path d="M12 15a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.93V21h2v-2.07A7 7 0 0 0 19 12h-2z"/></svg>"##;

/// Initialise le tray icon et le menu contextuel au démarrage de l'app.
///
/// Structure du menu :
///   ► Démarrer dictée      (toggle)
///   Langue ▶               (sous-menu)
///     ● Français / English / Nederlands / Auto
///   Modèle ▶               (sous-menu)
///       Tiny (~39 MB) / Base (~74 MB) / Small (~244 MB)
///   Paramètres             (ouvre settings.html)
///   Quitter
/// Initialise le tray icon. Prend un `AppHandle` (non-générique) pour éviter
/// les contraintes de runtime sur `TrayIconBuilder::build`.
pub fn setup_tray(handle: &AppHandle) -> Result<(), DictakuError> {

    // Sous-menu Langue.
    let lang_fr = MenuItem::with_id(handle, "lang_fr", "🇫🇷 Français", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let lang_en = MenuItem::with_id(handle, "lang_en", "🇬🇧 English", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let lang_nl = MenuItem::with_id(handle, "lang_nl", "🇧🇪 Nederlands", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let lang_auto = MenuItem::with_id(handle, "lang_auto", "Auto-détection", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    let lang_submenu = Submenu::with_items(
        handle,
        "Langue",
        true,
        &[&lang_fr, &lang_en, &lang_nl, &lang_auto],
    )
    .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    // Sous-menu Modèle.
    let model_tiny = MenuItem::with_id(handle, "model_tiny", "Tiny (~39 MB)", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let model_base = MenuItem::with_id(handle, "model_base", "Base (~74 MB) ●", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let model_small = MenuItem::with_id(handle, "model_small", "Small (~244 MB)", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    let model_submenu = Submenu::with_items(
        handle,
        "Modèle Whisper",
        true,
        &[&model_tiny, &model_base, &model_small],
    )
    .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    // Items principaux.
    let toggle_item = MenuItem::with_id(
        handle,
        "toggle",
        "Démarrer dictée  (Ctrl+Alt+D)",
        true,
        None::<&str>,
    )
    .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    let settings_item = MenuItem::with_id(handle, "settings", "Paramètres", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    let quit_item = MenuItem::with_id(handle, "quit", "Quitter Dictaku", true, None::<&str>)
        .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    let menu = Menu::with_items(
        handle,
        &[
            &toggle_item,
            &lang_submenu,
            &model_submenu,
            &settings_item,
            &quit_item,
        ],
    )
    .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    // Construction du tray icon — handle (AppHandle<R>) impl Manager<R>.
    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Dictaku — Dictée vocale (Ctrl+Alt+D)")
        .on_menu_event(|app, event| {
            handle_menu_event(app, event.id().as_ref());
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                debug!("Tray : clic gauche — toggle dictée");
                // TODO: émettre l'événement toggle vers le pipeline
            }
        })
        .build(handle)
        .map_err(|e| DictakuError::HotkeyRegistration(format!("Tray build : {e}")))?;

    info!("Tray icon initialisé");
    Ok(())
}

/// Dispatch des events du menu contextuel.
fn handle_menu_event(app: &AppHandle, event_id: &str) {
    match event_id {
        "toggle" => {
            info!("Menu : toggle dictée");
            // TODO: appeler toggle_dictation
        }
        "settings" => {
            info!("Menu : ouverture paramètres");
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "quit" => {
            info!("Menu : quitter");
            app.exit(0);
        }
        id if id.starts_with("lang_") => {
            debug!("Menu : changement langue → {id}");
            // TODO: mettre à jour Settings.language et AppState
        }
        id if id.starts_with("model_") => {
            debug!("Menu : changement modèle → {id}");
            // TODO: mettre à jour Settings.model et AppState.model_path
        }
        _ => {
            debug!("Menu : event inconnu — {event_id}");
        }
    }
}
