//! Comandos IPC expuestos a la interfaz.
//!
//! Todos son asíncronos: en Tauri 2 los comandos síncronos se ejecutan en el
//! hilo principal y bloquearían la ventana.

use std::path::PathBuf;

use nebula_core::{Book, BookId, ImportReport, LibraryQuery, LibraryStats, Timestamp, scan_paths};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Claves de ajustes que la interfaz puede leer y escribir.
const SETTING_KEYS: &[&str] = &["theme"];
/// Longitud máxima de un valor de ajuste.
const MAX_SETTING_LEN: usize = 1024;
/// Longitud máxima del texto de búsqueda.
const MAX_SEARCH_LEN: usize = 256;
/// Número máximo de rutas por importación.
const MAX_IMPORT_PATHS: usize = 10_000;

/// Versión del núcleo.
#[tauri::command]
pub fn app_version() -> &'static str {
    nebula_core::VERSION
}

/// Lista la biblioteca según la consulta.
#[tauri::command]
pub async fn library_list(
    state: State<'_, AppState>,
    query: LibraryQuery,
) -> CommandResult<Vec<Book>> {
    if query
        .search
        .as_ref()
        .is_some_and(|s| s.len() > MAX_SEARCH_LEN)
    {
        return Err(CommandError::invalid_input(
            "la búsqueda es demasiado larga",
        ));
    }
    state.with_library(move |lib| lib.list(&query)).await
}

/// Recuento de libros y cómics.
#[tauri::command]
pub async fn library_stats(state: State<'_, AppState>) -> CommandResult<LibraryStats> {
    state.with_library(nebula_core::Library::stats).await
}

/// Importa archivos y carpetas.
///
/// El recorrido del disco se hace sin bloquear la biblioteca; solo la
/// inserción final la bloquea.
#[tauri::command]
pub async fn library_import(
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
) -> CommandResult<ImportReport> {
    if paths.len() > MAX_IMPORT_PATHS {
        return Err(CommandError::invalid_input("demasiados elementos a la vez"));
    }
    let scan = tauri::async_runtime::spawn_blocking(move || scan_paths(&paths)).await?;
    state
        .with_library(move |lib| lib.add_scanned(scan, Timestamp::now()))
        .await
}

/// Quita un libro de la biblioteca (el archivo no se borra).
#[tauri::command]
pub async fn library_remove(state: State<'_, AppState>, id: BookId) -> CommandResult<()> {
    state.with_library(move |lib| lib.remove(id)).await
}

/// Lee un ajuste.
#[tauri::command]
pub async fn settings_get(
    state: State<'_, AppState>,
    key: String,
) -> CommandResult<Option<String>> {
    validate_setting_key(&key)?;
    state.with_library(move |lib| lib.setting(&key)).await
}

/// Guarda un ajuste.
#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> CommandResult<()> {
    validate_setting_key(&key)?;
    if value.len() > MAX_SETTING_LEN {
        return Err(CommandError::invalid_input(
            "valor de ajuste demasiado largo",
        ));
    }
    state
        .with_library(move |lib| lib.set_setting(&key, &value))
        .await
}

fn validate_setting_key(key: &str) -> CommandResult<()> {
    if SETTING_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(CommandError::invalid_input(format!(
            "ajuste desconocido: {key}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_setting_key_is_accepted() {
        assert!(validate_setting_key("theme").is_ok());
    }

    #[test]
    fn unknown_setting_key_is_rejected() {
        assert!(validate_setting_key("../../etc").is_err());
    }
}
