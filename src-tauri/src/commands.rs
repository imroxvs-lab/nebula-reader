//! Comandos IPC expuestos a la interfaz.
//!
//! Todos son asíncronos: en Tauri 2 los comandos síncronos se ejecutan en el
//! hilo principal y bloquearían la ventana.

use std::path::PathBuf;

use nebula_core::cover::cover_path;
use nebula_core::{
    Book, BookId, ImportReport, LibraryQuery, LibraryStats, Locator, ReadingProgress, Timestamp,
    scan_paths,
};
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Claves de ajustes que la interfaz puede leer y escribir.
const SETTING_KEYS: &[&str] = &["theme", "readerFit"];
/// Longitud máxima de un valor de ajuste.
const MAX_SETTING_LEN: usize = 1024;
/// Longitud máxima del texto de búsqueda.
const MAX_SEARCH_LEN: usize = 256;
/// Número máximo de rutas por importación.
const MAX_IMPORT_PATHS: usize = 10_000;
/// Elementos máximos de «Continuar leyendo».
const MAX_CONTINUE_READING: u32 = 50;

/// Libro listo para leer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedBook {
    /// Datos del libro (ya con `last_opened_at` y `page_count` actualizados).
    pub book: Book,
    /// Número de páginas.
    pub page_count: u32,
    /// Dónde se quedó el usuario, si había empezado.
    pub progress: Option<ReadingProgress>,
}

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

/// Quita un libro de la biblioteca (el archivo no se borra), cierra el
/// documento si estaba abierto y elimina su miniatura.
#[tauri::command]
pub async fn library_remove(state: State<'_, AppState>, id: BookId) -> CommandResult<()> {
    state
        .blocking(move |state| {
            state.library_blocking(|lib| lib.remove(id))?;
            state.documents().remove(id)?;
            let thumb = cover_path(state.thumbs_dir(), id);
            if let Err(error) = std::fs::remove_file(&thumb)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(path = %thumb.display(), %error, "no se pudo borrar la miniatura");
            }
            Ok(())
        })
        .await
}

/// Libros empezados y no terminados, abiertos más recientemente primero.
#[tauri::command]
pub async fn library_continue_reading(
    state: State<'_, AppState>,
    limit: u32,
) -> CommandResult<Vec<Book>> {
    let limit = limit.min(MAX_CONTINUE_READING);
    state
        .with_library(move |lib| lib.continue_reading(limit))
        .await
}

/// Abre un libro para leer: indexa el archivo, registra la apertura y devuelve
/// el progreso guardado.
#[tauri::command]
pub async fn book_open(state: State<'_, AppState>, id: BookId) -> CommandResult<OpenedBook> {
    state
        .blocking(move |state| {
            let (book, archive) = state.comic(id)?;
            let page_count = archive
                .lock()
                .map_err(|_| CommandError::internal("documento en estado inválido"))?
                .page_count();
            let page_count = u32::try_from(page_count).unwrap_or(u32::MAX);
            state.library_blocking(|lib| {
                if book.page_count != Some(page_count) {
                    lib.set_page_count(id, page_count)?;
                }
                lib.mark_opened(id, Timestamp::now())?;
                Ok(OpenedBook {
                    book: lib.get(id)?,
                    page_count,
                    progress: lib.progress(id)?,
                })
            })
        })
        .await
}

/// Cierra un libro y libera su índice en memoria.
#[tauri::command]
pub async fn book_close(state: State<'_, AppState>, id: BookId) -> CommandResult<()> {
    state
        .blocking(move |state| state.documents().remove(id))
        .await
}

/// Guarda la posición de lectura.
#[tauri::command]
pub async fn progress_save(
    state: State<'_, AppState>,
    id: BookId,
    locator: Locator,
    percent: f64,
) -> CommandResult<()> {
    if !percent.is_finite() {
        return Err(CommandError::invalid_input("porcentaje no válido"));
    }
    state
        .with_library(move |lib| {
            let Locator::Page { index } = locator;
            if let Some(count) = lib.get(id)?.page_count
                && index >= count
            {
                return Err(nebula_core::CoreError::PageOutOfRange {
                    index: index as usize,
                    count: count as usize,
                });
            }
            lib.save_progress(id, locator, percent, Timestamp::now())
        })
        .await
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
