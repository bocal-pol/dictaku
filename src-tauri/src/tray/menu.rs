use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tracing::{debug, info};

use crate::error::DictakuError;

/// SVG du tray icon en état Idle (microphone gris).
/// Embedé en base64 pour éviter un accès disque au démarrage.
#[allow(dead_code)]
const ICON_IDLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#9ca3af"><path d="M12 15a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.93V21h2v-2.07A7 7 0 0 0 19 12h-2z"/></svg>"##;

/// SVG du tray icon en état Listening (microphone vert animé).
#[allow(dead_code)]
const ICON_LISTENING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#22c55e"><path d="M12 15a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.93V21h2v-2.07A7 7 0 0 0 19 12h-2z"/></svg>"##;

/// SVG du tray icon en état Transcribing/Injecting (microphone orange).
#[allow(dead_code)]
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
    // Lit le raccourci et le modèle actif depuis la config pour l'afficher dans le menu.
    let (hotkey_display, active_model) = {
        let state = handle.state::<crate::state::app_state::AppState>();
        let config = state.config.blocking_lock();
        (format_hotkey_display(&config.hotkey), config.model.clone())
    };

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

    // Sous-menu Modèle — le ● marque le modèle actif lu depuis la config.
    use crate::config::settings::WhisperModel;
    let dot = |m: &WhisperModel, target: WhisperModel| if *m == target { " ●" } else { "" };
    let model_tiny = MenuItem::with_id(
        handle, "model_tiny",
        format!("Tiny (~39 MB){}", dot(&active_model, WhisperModel::Tiny)),
        true, None::<&str>,
    ).map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let model_base = MenuItem::with_id(
        handle, "model_base",
        format!("Base (~74 MB){}", dot(&active_model, WhisperModel::Base)),
        true, None::<&str>,
    ).map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;
    let model_small = MenuItem::with_id(
        handle, "model_small",
        format!("Small (~244 MB){}", dot(&active_model, WhisperModel::Small)),
        true, None::<&str>,
    ).map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    let model_submenu = Submenu::with_items(
        handle,
        "Modèle Whisper",
        true,
        &[&model_tiny, &model_base, &model_small],
    )
    .map_err(|e| DictakuError::HotkeyRegistration(e.to_string()))?;

    // Items principaux.
    let toggle_label = format!("Démarrer dictée  ({})", hotkey_display);
    let toggle_item = MenuItem::with_id(
        handle,
        "toggle",
        &toggle_label,
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
        .tooltip(format!("Dictaku — Dictée vocale ({})", hotkey_display))
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

/// Formate un raccourci "ctrl+shift+f12" en "Ctrl+Shift+F12" pour l'affichage.
fn format_hotkey_display(hotkey: &str) -> String {
    hotkey
        .split('+')
        .map(|part| {
            let mut chars = part.trim().chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join("+")
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
            use crate::config::settings::Language;
            use crate::state::app_state::AppState;

            let lang = match id {
                "lang_fr"   => Language::Fr,
                "lang_en"   => Language::En,
                "lang_nl"   => Language::Nl,
                "lang_auto" => Language::Auto,
                _           => return,
            };
            info!("Menu : langue → {lang}");
            let state = app.state::<AppState>();
            let mut config = state.config.blocking_lock();
            config.language = lang;
            if let Err(e) = config.save() {
                tracing::error!("Sauvegarde config langue : {e}");
            }
        }
        id if id.starts_with("model_") => {
            use crate::config::settings::WhisperModel;
            use crate::state::app_state::AppState;

            let model = match id {
                "model_tiny"  => WhisperModel::Tiny,
                "model_base"  => WhisperModel::Base,
                "model_small" => WhisperModel::Small,
                _             => return,
            };
            info!("Menu : modèle → {model:?}");
            let state = app.state::<AppState>();
            let mut config = state.config.blocking_lock();

            // Vérifie que le fichier modèle est présent avant de switcher.
            let model_path = config.models_dir().join(model.filename());
            if !model_path.exists() {
                tracing::warn!(
                    "Modèle {} absent — ouvrir la fenêtre setup pour le télécharger",
                    model.filename()
                );
                drop(config);
                if let Some(win) = app.get_webview_window("setup") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                return;
            }

            config.model = model;
            if let Err(e) = config.save() {
                tracing::error!("Sauvegarde config modèle : {e}");
            }
        }
        _ => {
            debug!("Menu : event inconnu — {event_id}");
        }
    }
}
