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
use tracing::info;

/// Point d'entrée de la bibliothèque — appelé depuis `main.rs`.
///
/// Initialise l'ensemble de la stack Tauri :
/// 1. Tracing (log vers stderr, filtre depuis RUST_LOG ou info par défaut)
/// 2. Configuration utilisateur (créée avec les defaults si absente)
/// 3. AppState partagé via `tauri::Manager`
/// 4. Plugins Tauri (global-shortcut, notification, shell)
/// 5. Tray icon + menu contextuel
/// 6. Commandes IPC exposées à la WebView
pub fn run() {
    // Initialisation du subscriber tracing. Utilise RUST_LOG si défini,
    // sinon `info` pour la release, `debug` pour le dev.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("dictaku=info")),
        )
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
        .setup(|app| {
            // Installation du tray icon et du menu contextuel.
            tray::menu::setup_tray(app.handle())
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            // Enregistrement du raccourci global Ctrl+Alt+D.
            hotkey::manager::register_global_shortcut(app)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

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
        ])
        .run(tauri::generate_context!())
        .expect("Erreur critique lors du démarrage de l'application Tauri");
}
