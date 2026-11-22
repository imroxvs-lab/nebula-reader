//! Capa de aplicación Tauri de Nebula Reader.
//!
//! Se mantiene fina: la lógica vive en `nebula-core`.

/// Devuelve la versión del núcleo para mostrarla en la interfaz.
#[tauri::command]
fn app_version() -> &'static str {
    nebula_core::VERSION
}

/// Arranca la aplicación.
///
/// # Errors
/// Devuelve el error de Tauri si la aplicación no puede inicializarse.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
}
