//! Comandos IPC expuestos a la interfaz.
//!
//! Todos son asíncronos: en Tauri 2 los comandos síncronos se ejecutan en el
//! hilo principal y bloquearían la ventana.

use std::path::PathBuf;

use nebula_core::cover::cover_path;
use nebula_core::{
    Book, BookFormat, BookId, ImportReport, LibraryQuery, LibraryStats, Locator, OpenDocument,
    ReadingProgress, SpineItem, Timestamp, TocEntry, metadata, scan_paths,
};
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

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
    /// Páginas (cómic) o documentos del orden de lectura (EPUB).
    pub page_count: u32,
    /// Dónde se quedó el usuario, si había empezado.
    pub progress: Option<ReadingProgress>,
    /// Estructura específica del formato.
    pub content: ReaderContent,
    /// Opciones de lectura guardadas para este libro (JSON), si las hay.
    pub reader_options: Option<String>,
}

/// Tamaño máximo del JSON de opciones de lectura.
const MAX_READER_OPTIONS_LEN: usize = 2048;

/// Estructura del documento que necesita cada lector.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ReaderContent {
    /// Cómic: basta con el número de páginas.
    Comic,
    /// EPUB: orden de lectura e índice.
    Epub {
        /// Documentos en orden de lectura.
        spine: Vec<SpineItem>,
        /// Índice.
        toc: Vec<TocEntry>,
    },
    /// PDF: se renderiza página a página bajo demanda.
    Pdf {
        /// Proporción alto/ancho de la primera página (para reservar espacio).
        aspect_ratio: f64,
    },
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
            let book = state.library_blocking(|lib| lib.get(id))?;
            // Se comprueba aquí para marcar el libro como desaparecido (o
            // recuperado) y dar un mensaje claro.
            let exists = book.path.is_file();
            if exists == book.missing {
                state.library_blocking(|lib| lib.set_missing(id, !exists))?;
            }
            if !exists {
                return Err(CommandError::new(
                    crate::error::ErrorCode::Io,
                    "No se encuentra el archivo. ¿Se ha movido, borrado o está en un disco desconectado? \
                     Usa «Localizar archivo…» para indicar dónde está.",
                ));
            }
            let (units, content, fresh_metadata) = if book.format == BookFormat::Pdf {
                // Los PDF no se mantienen abiertos: cada página se renderiza aparte.
                let engine = state.pdf()?;
                let info = engine.info(&book.path)?;
                let fresh = if book.author.is_none() {
                    engine.metadata(&book.path).ok().filter(|m| !m.is_empty())
                } else {
                    None
                };
                (
                    info.page_count as usize,
                    ReaderContent::Pdf {
                        aspect_ratio: info.aspect_ratio,
                    },
                    fresh,
                )
            } else {
                let (_, document) = state.document(id)?;
                let document = document
                    .lock()
                    .map_err(|_| CommandError::internal("documento en estado inválido"))?;
                match &*document {
                    OpenDocument::Comic(_) => (document.unit_count(), ReaderContent::Comic, None),
                    OpenDocument::Epub(epub) => (
                        document.unit_count(),
                        ReaderContent::Epub {
                            spine: epub.spine().to_vec(),
                            toc: epub.toc().to_vec(),
                        },
                        // Libros importados antes de leer metadatos: se completan ahora.
                        book.author
                            .is_none()
                            .then(|| metadata::from_epub(epub))
                            .filter(|meta| !meta.is_empty()),
                    ),
                }
            };
            let page_count = u32::try_from(units).unwrap_or(u32::MAX);
            state.library_blocking(|lib| {
                if book.page_count != Some(page_count) {
                    lib.set_page_count(id, page_count)?;
                }
                if let Some(meta) = &fresh_metadata {
                    lib.update_metadata(id, meta)?;
                }
                lib.mark_opened(id, Timestamp::now())?;
                Ok(OpenedBook {
                    book: lib.get(id)?,
                    page_count,
                    progress: lib.progress(id)?,
                    content,
                    reader_options: lib.reader_options(id)?,
                })
            })
        })
        .await
}

/// Guarda las opciones de lectura de un libro (modo de página, sentido…).
#[tauri::command]
pub async fn book_options_set(
    state: State<'_, AppState>,
    id: BookId,
    options: String,
) -> CommandResult<()> {
    if options.len() > MAX_READER_OPTIONS_LEN
        || serde_json::from_str::<serde_json::Value>(&options).is_err()
    {
        return Err(CommandError::invalid_input(
            "opciones de lectura no válidas",
        ));
    }
    state
        .with_library(move |lib| lib.set_reader_options(id, Some(&options)))
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
            if let Locator::Epub { position, .. } = locator
                && !(0.0..=1.0).contains(&position)
            {
                return Err(nebula_core::CoreError::InvalidData(
                    "posición fuera de rango".to_owned(),
                ));
            }
            let index = locator.unit();
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
    crate::settings::validate_key(&key)?;
    state.with_library(move |lib| lib.setting(&key)).await
}

/// Guarda un ajuste.
#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> CommandResult<()> {
    crate::settings::validate(&key, &value)?;
    state
        .with_library(move |lib| lib.set_setting(&key, &value))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_content_fields_are_camel_case() {
        let json = serde_json::to_string(&ReaderContent::Pdf { aspect_ratio: 1.5 }).ok();
        assert_eq!(json.as_deref(), Some(r#"{"type":"pdf","aspectRatio":1.5}"#));
    }
}
