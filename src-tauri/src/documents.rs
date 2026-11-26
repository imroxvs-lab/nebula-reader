//! Caché LRU de documentos abiertos.
//!
//! Mantiene abiertos los últimos libros leídos para servir páginas sin volver a
//! indexar el archivo en cada petición. El índice de un CBZ ocupa poco (nombres
//! de entradas), pero se limita el número para acotar descriptores y memoria.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use nebula_core::{BookId, CoreResult, OpenDocument};

use crate::error::{CommandError, CommandResult};

/// Documento compartido entre peticiones; el `Mutex` serializa las lecturas
/// porque el lector ZIP necesita `&mut` para desplazarse por el archivo.
pub type SharedDocument = Arc<Mutex<OpenDocument>>;

/// Caché LRU de documentos abiertos.
#[derive(Debug)]
pub struct DocumentCache {
    capacity: usize,
    /// El más reciente al final.
    entries: Mutex<VecDeque<(BookId, SharedDocument)>>,
}

impl DocumentCache {
    /// Crea una caché que mantiene como máximo `capacity` documentos.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Devuelve el documento si está abierto, o lo abre con `open` y lo guarda.
    ///
    /// La apertura se hace sin mantener el bloqueo de la caché, para no frenar
    /// las peticiones de otros documentos mientras se indexa un archivo.
    pub fn get_or_open<F>(&self, id: BookId, open: F) -> CommandResult<SharedDocument>
    where
        F: FnOnce() -> CoreResult<OpenDocument>,
    {
        if let Some(found) = self.touch(id)? {
            return Ok(found);
        }
        let archive = Arc::new(Mutex::new(open()?));

        let mut entries = self.lock()?;
        // Otra petición pudo abrirlo mientras tanto: se conserva el existente.
        if let Some(position) = entries.iter().position(|(entry_id, _)| *entry_id == id) {
            return Ok(Arc::clone(&entries[position].1));
        }
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back((id, Arc::clone(&archive)));
        Ok(archive)
    }

    /// Cierra todos los documentos abiertos.
    pub fn clear(&self) -> CommandResult<()> {
        self.lock()?.clear();
        Ok(())
    }

    /// Cierra un documento si estaba abierto.
    pub fn remove(&self, id: BookId) -> CommandResult<()> {
        self.lock()?.retain(|(entry_id, _)| *entry_id != id);
        Ok(())
    }

    /// Busca un documento y lo marca como el más reciente.
    fn touch(&self, id: BookId) -> CommandResult<Option<SharedDocument>> {
        let mut entries = self.lock()?;
        let Some(position) = entries.iter().position(|(entry_id, _)| *entry_id == id) else {
            return Ok(None);
        };
        let entry = entries.remove(position);
        let archive = entry.as_ref().map(|(_, archive)| Arc::clone(archive));
        if let Some(entry) = entry {
            entries.push_back(entry);
        }
        Ok(archive)
    }

    fn lock(&self) -> CommandResult<std::sync::MutexGuard<'_, VecDeque<(BookId, SharedDocument)>>> {
        self.entries.lock().map_err(|_| {
            CommandError::internal("la caché de documentos quedó en un estado inválido")
        })
    }

    /// Identificadores abiertos, del menos al más reciente (para tests).
    #[cfg(test)]
    fn ids(&self) -> Vec<BookId> {
        self.entries
            .lock()
            .map(|entries| entries.iter().map(|(id, _)| *id).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use std::io::Write;
    use std::path::PathBuf;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn cbz(dir: &std::path::Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("crear"));
        zip.start_file("01.jpg", SimpleFileOptions::default())
            .expect("entrada");
        zip.write_all(b"x").expect("escribir");
        zip.finish().expect("cerrar");
        path
    }

    fn open_into(cache: &DocumentCache, id: i64, path: &std::path::Path) {
        cache
            .get_or_open(BookId::new(id), || {
                nebula_core::ComicArchive::open(path).map(OpenDocument::Comic)
            })
            .expect("abrir");
    }

    #[test]
    fn evicts_least_recently_used() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = cbz(dir.path(), "a.cbz");
        let cache = DocumentCache::new(2);
        open_into(&cache, 1, &path);
        open_into(&cache, 2, &path);
        open_into(&cache, 1, &path); // 1 pasa a ser el más reciente
        open_into(&cache, 3, &path); // expulsa al 2
        assert_eq!(cache.ids(), [BookId::new(1), BookId::new(3)]);
    }

    #[test]
    fn cached_document_is_not_reopened() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = cbz(dir.path(), "a.cbz");
        let cache = DocumentCache::new(2);
        open_into(&cache, 1, &path);
        let result = cache.get_or_open(BookId::new(1), || {
            Err(nebula_core::CoreError::NoPages(PathBuf::from(
                "no se debe llamar",
            )))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn remove_closes_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = cbz(dir.path(), "a.cbz");
        let cache = DocumentCache::new(2);
        open_into(&cache, 1, &path);
        cache.remove(BookId::new(1)).expect("cerrar");
        assert!(cache.ids().is_empty());
    }
}
