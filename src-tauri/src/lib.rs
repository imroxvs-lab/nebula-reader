//! Capa de aplicación Tauri de Nebula Reader.
//!
//! Se mantiene fina: la lógica vive en `nebula-core`.

mod commands;
mod documents;
mod error;
mod protocol;
mod state;

use nebula_core::Library;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

use crate::state::AppState;

/// Nombre del archivo de la biblioteca dentro de la carpeta local de datos
/// (`%LOCALAPPDATA%\com.nebulareader.reader`).
const LIBRARY_FILE: &str = "library.db";
/// Carpeta de miniaturas de portada dentro de la carpeta local de datos.
const THUMBS_DIR: &str = "thumbs";

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
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, protocol::handle)
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            let db_path = data_dir.join(LIBRARY_FILE);
            tracing::info!(path = %db_path.display(), "abriendo biblioteca");
            let library = Library::open(&db_path)?;
            let pdf = load_pdf_engine(app.handle());
            app.manage(AppState::new(library, data_dir.join(THUMBS_DIR), pdf));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::library_list,
            commands::library_stats,
            commands::library_import,
            commands::library_remove,
            commands::library_continue_reading,
            commands::book_open,
            commands::book_close,
            commands::book_options_set,
            commands::progress_save,
            commands::settings_get,
            commands::settings_set,
        ])
        .run(tauri::generate_context!())
}

/// Carga PDFium desde los recursos de la app. Si falta, los PDF se deshabilitan
/// pero la app funciona con normalidad.
fn load_pdf_engine(app: &tauri::AppHandle) -> Option<nebula_core::pdf::PdfEngine> {
    let library = match app.path().resource_dir() {
        Ok(dir) => dir.join("pdfium").join("pdfium.dll"),
        Err(error) => {
            tracing::warn!(%error, "sin carpeta de recursos; PDF deshabilitado");
            return None;
        }
    };
    match nebula_core::pdf::PdfEngine::load(&library) {
        Ok(engine) => {
            tracing::info!(path = %library.display(), "motor PDF cargado");
            Some(engine)
        }
        Err(error) => {
            tracing::warn!(path = %library.display(), %error, "motor PDF no disponible");
            None
        }
    }
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
