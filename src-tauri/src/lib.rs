pub mod audio;
pub mod config;
pub mod error;
pub mod hotkey;
pub mod injection;
pub mod ipc;
pub mod platform;
pub mod state;
pub mod stt;
pub mod tray;

use state::app_state::AppState;
use tauri::Manager;
use tracing::info;
use tracing_subscriber::prelude::*;

pub fn run() {
    // Dossier de log : %APPDATA%\dictaku\dictaku\
    let log_dir = directories::ProjectDirs::from("com", "dictaku", "dictaku")
        .map(|p| p.config_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _ = std::fs::create_dir_all(&log_dir);

    // Écriture dans dictaku.log (rotation : jamais — fichier unique, écrasé au démarrage).
    let log_path = log_dir.join("dictaku.log");
    let log_file = std::fs::File::create(&log_path)
        .expect("impossible de créer le fichier de log");

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("dictaku=debug"));

    // Layer fichier (debug complet) + layer stderr (info uniquement).
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log_file));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    info!("Dictaku v{} démarrage", env!("CARGO_PKG_VERSION"));

    // Chargement de la configuration — crée le fichier par défaut si absent.
    let settings = config::settings::Settings::load().unwrap_or_else(|err| {
        tracing::warn!("Impossible de charger la config ({err}), utilisation des defaults");
        config::settings::Settings::default()
    });

    let app_state = AppState::new(settings);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        // Empêche Tauri de quitter quand toutes les fenêtres sont fermées/cachées.
        // L'app vit dans le tray — seul le menu "Quitter" arrête le process.
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
            }
        })
        .setup(|app| {
            // Installation du tray icon et du menu contextuel.
            tray::menu::setup_tray(app.handle())
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            // Enregistrement du raccourci global Ctrl+Alt+D.
            hotkey::manager::register_global_shortcut(app)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            // Détection du modèle Whisper — ouvre la fenêtre de setup si absent.
            {
                let config = app.state::<AppState>().config.blocking_lock().clone();
                let manager = stt::model_manager::ModelManager::new(&config);
                if !manager.is_model_available(&config.model) {
                    info!("Modèle Whisper absent — ouverture de la fenêtre de setup");
                    if let Some(win) = app.get_webview_window("setup") {
                        win.show().ok();
                        win.set_focus().ok();
                    }
                }
            }

            info!("Setup Tauri terminé — application en attente dans le tray");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::get_state,
            ipc::commands::toggle_dictation,
            ipc::commands::get_settings,
            ipc::commands::save_settings,
            ipc::commands::download_model,
            ipc::commands::check_model_exists,
            ipc::commands::close_setup_window,
        ])
        .run(tauri::generate_context!())
        .expect("Erreur critique lors du démarrage de l'application Tauri");
}
