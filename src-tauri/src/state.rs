//! Estado compartido de la aplicación.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nebula_core::{Book, BookFormat, BookId, ComicArchive, CoreResult, Library};
use tokio::sync::Semaphore;

use crate::documents::{DocumentCache, SharedArchive};
use crate::error::{CommandError, CommandResult};

/// Documentos abiertos que se mantienen en memoria.
const OPEN_DOCUMENTS: usize = 3;
/// Portadas que se pueden generar a la vez. Decodificar una página grande usa
/// bastante CPU y memoria temporal; dos en paralelo es un buen equilibrio.
const PARALLEL_COVERS: usize = 2;

/// Estado gestionado por Tauri.
///
/// La biblioteca va en `Arc<Mutex<_>>` porque `rusqlite::Connection` no es
/// `Sync` y las operaciones se ejecutan en hilos bloqueantes
/// (`spawn_blocking`), que necesitan poseer una referencia `'static`.
#[derive(Debug, Clone)]
pub struct AppState {
    library: Arc<Mutex<Library>>,
    documents: Arc<DocumentCache>,
    thumbs_dir: Arc<PathBuf>,
    cover_permits: Arc<Semaphore>,
}

impl AppState {
    /// Crea el estado con una biblioteca abierta y la carpeta de miniaturas.
    pub fn new(library: Library, thumbs_dir: PathBuf) -> Self {
        Self {
            library: Arc::new(Mutex::new(library)),
            documents: Arc::new(DocumentCache::new(OPEN_DOCUMENTS)),
            thumbs_dir: Arc::new(thumbs_dir),
            cover_permits: Arc::new(Semaphore::new(PARALLEL_COVERS)),
        }
    }

    /// Carpeta de la caché de miniaturas.
    pub fn thumbs_dir(&self) -> &Path {
        &self.thumbs_dir
    }

    /// Limitador de generación de portadas.
    pub fn cover_permits(&self) -> &Arc<Semaphore> {
        &self.cover_permits
    }

    /// Caché de documentos abiertos.
    pub fn documents(&self) -> &DocumentCache {
        &self.documents
    }

    /// Ejecuta `f` con la biblioteca bloqueada en un hilo de E/S, sin bloquear
    /// el runtime asíncrono ni el hilo de la interfaz.
    pub async fn with_library<T, F>(&self, f: F) -> CommandResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Library) -> CoreResult<T> + Send + 'static,
    {
        let state = self.clone();
        tauri::async_runtime::spawn_blocking(move || state.library_blocking(f)).await?
    }

    /// Ejecuta `f` con la biblioteca bloqueada. Solo desde hilos bloqueantes.
    pub fn library_blocking<T, F>(&self, f: F) -> CommandResult<T>
    where
        F: FnOnce(&Library) -> CoreResult<T>,
    {
        let guard = self
            .library
            .lock()
            .map_err(|_| CommandError::internal("la biblioteca quedó en un estado inválido"))?;
        f(&guard).map_err(CommandError::from)
    }

    /// Ejecuta `f` en un hilo bloqueante con una copia del estado.
    pub async fn blocking<T, F>(&self, f: F) -> CommandResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Self) -> CommandResult<T> + Send + 'static,
    {
        let state = self.clone();
        tauri::async_runtime::spawn_blocking(move || f(&state)).await?
    }

    /// Devuelve el cómic abierto de un libro, abriéndolo si hace falta.
    /// Solo desde hilos bloqueantes.
    pub fn comic(&self, id: BookId) -> CommandResult<(Book, SharedArchive)> {
        let book = self.library_blocking(|lib| lib.get(id))?;
        ensure_readable(&book)?;
        let archive = self
            .documents
            .get_or_open(id, || ComicArchive::open(&book.path))?;
        Ok((book, archive))
    }
}

/// Comprueba que el formato ya tiene lector.
pub fn ensure_readable(book: &Book) -> CommandResult<()> {
    match book.format {
        BookFormat::Cbz => Ok(()),
        BookFormat::Cbr | BookFormat::Cb7 => Err(CommandError::unsupported(
            "Los cómics CBR y CB7 todavía no se pueden abrir; llegarán en una próxima versión.",
        )),
        BookFormat::Epub => Err(CommandError::unsupported(
            "Los libros EPUB todavía no se pueden abrir; llegarán en una próxima versión.",
        )),
        BookFormat::Pdf => Err(CommandError::unsupported(
            "Los PDF todavía no se pueden abrir; llegarán en una próxima versión.",
        )),
    }
}
