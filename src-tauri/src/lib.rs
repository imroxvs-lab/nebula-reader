//! Capa de aplicación Tauri de Nebula Reader.
//!
//! Se mantiene fina: la lógica vive en `nebula-core`.

mod commands;
mod error;
mod state;

use nebula_core::Library;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

use crate::state::AppState;

/// Nombre del archivo de la biblioteca dentro de la carpeta local de datos
/// (`%LOCALAPPDATA%\com.nebulareader.reader`).
const LIBRARY_FILE: &str = "library.db";

/// Arranca la aplicación.
///
/// # Errors
/// Devuelve el error de Tauri si la aplicación no puede inicializarse,
/// incluido el fallo al abrir la biblioteca.
pub fn run() -> tauri::Result<()> {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                // La configuración manda sobre las decoraciones (barra de título propia).
                .with_state_flags(StateFlags::all() - StateFlags::DECORATIONS)
                .build(),
        )
        .setup(|app| {
            let db_path = app.path().app_local_data_dir()?.join(LIBRARY_FILE);
            tracing::info!(path = %db_path.display(), "abriendo biblioteca");
            let library = Library::open(&db_path)?;
            app.manage(AppState::new(library));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::library_list,
            commands::library_stats,
            commands::library_import,
            commands::library_remove,
            commands::settings_get,
            commands::settings_set,
        ])
        .run(tauri::generate_context!())
}

fn init_tracing() {
    let max_level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    // Si ya hay un suscriptor (p. ej. en tests), no se reemplaza.
    let _ = tracing_subscriber::fmt()
        .with_max_level(max_level)
        .with_target(false)
        .try_init();
}
