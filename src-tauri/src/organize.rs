//! Comandos de organización: estado de lectura, marcadores, colecciones,
//! filtros y reescaneo de la biblioteca.

use std::path::PathBuf;

use nebula_core::{
    BookId, Bookmark, BookmarkId, Collection, CollectionId, LibraryFacets, Locator, ReadStatus,
    RescanReport, Timestamp, plan_rescan,
};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::settings::{LIBRARY_FOLDERS, parse_folders};
use crate::state::AppState;

/// Libros como máximo por operación con colecciones.
const MAX_BATCH: usize = 10_000;
/// Valores distintos como máximo en cada filtro.
const MAX_FACETS: u32 = 2_000;

fn check_batch(books: &[BookId]) -> CommandResult<()> {
    if books.len() > MAX_BATCH {
        return Err(CommandError::invalid_input("demasiados libros a la vez"));
    }
    Ok(())
}

/// Marca un libro como leído, no leído o en curso.
#[tauri::command]
pub async fn book_set_status(
    state: State<'_, AppState>,
    id: BookId,
    status: ReadStatus,
) -> CommandResult<()> {
    state
        .with_library(move |lib| lib.set_status(id, status, Timestamp::now()))
        .await
}

/// Autores y series de la biblioteca, para los filtros.
#[tauri::command]
pub async fn library_facets(state: State<'_, AppState>) -> CommandResult<LibraryFacets> {
    state.with_library(|lib| lib.facets(MAX_FACETS)).await
}

/// Comprueba archivos desaparecidos o movidos y añade los nuevos de las
/// carpetas de la biblioteca. El disco se recorre sin bloquear la biblioteca.
#[tauri::command]
pub async fn library_rescan(state: State<'_, AppState>) -> CommandResult<RescanReport> {
    let (known, folders) = state
        .with_library(|lib| {
            let folders = lib
                .setting(LIBRARY_FOLDERS)?
                .and_then(|value| parse_folders(&value))
                .unwrap_or_default();
            Ok((lib.known_files()?, folders))
        })
        .await?;
    let plan = tauri::async_runtime::spawn_blocking(move || plan_rescan(known, &folders)).await?;
    let report = state
        .with_library(move |lib| lib.apply_rescan(plan, Timestamp::now()))
        .await?;
    // Los libros movidos cambian de ruta: se cierran los documentos abiertos.
    if report.relocated > 0 {
        state.blocking(|state| state.documents().clear()).await?;
    }
    tracing::info!(?report, "biblioteca reescaneada");
    Ok(report)
}

/// Indica la nueva ubicación de un libro cuyo archivo se ha movido.
#[tauri::command]
pub async fn book_relocate(
    state: State<'_, AppState>,
    id: BookId,
    path: PathBuf,
) -> CommandResult<()> {
    state
        .blocking(move |state| {
            let metadata = std::fs::metadata(&path)
                .map_err(|e| CommandError::from(nebula_core::CoreError::io(&path, e)))?;
            if !metadata.is_file() {
                return Err(CommandError::invalid_input("no es un archivo"));
            }
            state.library_blocking(|lib| lib.relocate(id, &path, metadata.len()))?;
            state.documents().remove(id)
        })
        .await
}

/// Marcadores de un libro.
#[tauri::command]
pub async fn bookmarks_list(
    state: State<'_, AppState>,
    id: BookId,
) -> CommandResult<Vec<Bookmark>> {
    state.with_library(move |lib| lib.bookmarks(id)).await
}

/// Añade un marcador.
#[tauri::command]
pub async fn bookmark_add(
    state: State<'_, AppState>,
    id: BookId,
    locator: Locator,
    percent: f64,
    label: String,
) -> CommandResult<Bookmark> {
    if !percent.is_finite() || label.len() > 4096 {
        return Err(CommandError::invalid_input("marcador no válido"));
    }
    state
        .with_library(move |lib| lib.add_bookmark(id, locator, percent, &label, Timestamp::now()))
        .await
}

/// Borra un marcador.
#[tauri::command]
pub async fn bookmark_remove(state: State<'_, AppState>, id: BookmarkId) -> CommandResult<()> {
    state.with_library(move |lib| lib.remove_bookmark(id)).await
}

/// Colecciones con su número de libros.
#[tauri::command]
pub async fn collections_list(state: State<'_, AppState>) -> CommandResult<Vec<Collection>> {
    state.with_library(nebula_core::Library::collections).await
}

/// Crea una colección.
#[tauri::command]
pub async fn collection_create(
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<Collection> {
    state
        .with_library(move |lib| lib.create_collection(&name, Timestamp::now()))
        .await
}

/// Cambia el nombre de una colección.
#[tauri::command]
pub async fn collection_rename(
    state: State<'_, AppState>,
    id: CollectionId,
    name: String,
) -> CommandResult<()> {
    state
        .with_library(move |lib| lib.rename_collection(id, &name))
        .await
}

/// Borra una colección (no sus libros).
#[tauri::command]
pub async fn collection_delete(state: State<'_, AppState>, id: CollectionId) -> CommandResult<()> {
    state
        .with_library(move |lib| lib.delete_collection(id))
        .await
}

/// Añade libros a una colección.
#[tauri::command]
pub async fn collection_add_books(
    state: State<'_, AppState>,
    id: CollectionId,
    books: Vec<BookId>,
) -> CommandResult<()> {
    check_batch(&books)?;
    state
        .with_library(move |lib| lib.add_to_collection(id, &books, Timestamp::now()))
        .await
}

/// Quita libros de una colección.
#[tauri::command]
pub async fn collection_remove_books(
    state: State<'_, AppState>,
    id: CollectionId,
    books: Vec<BookId>,
) -> CommandResult<()> {
    check_batch(&books)?;
    state
        .with_library(move |lib| lib.remove_from_collection(id, &books))
        .await
}

/// Colecciones a las que pertenece un libro.
#[tauri::command]
pub async fn book_collections(
    state: State<'_, AppState>,
    id: BookId,
) -> CommandResult<Vec<CollectionId>> {
    state.with_library(move |lib| lib.collections_of(id)).await
}
