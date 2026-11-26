//! Comprobación de archivos desaparecidos, movidos y nuevos.
//!
//! Se hace en dos pasos para no bloquear la biblioteca mientras se recorre el
//! disco:
//! 1. [`plan_rescan`] comprueba rutas y recorre las carpetas (sin SQLite).
//! 2. [`Library::apply_rescan`] aplica el resultado en una transacción.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::params;

use super::Library;
use crate::domain::{BookFormat, BookId, RescanReport, Timestamp};
use crate::error::{CoreError, CoreResult};
use crate::import::{ScanOutcome, collect_paths, enrich};

/// Libro conocido por la biblioteca, para comprobar su archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFile {
    /// Identificador.
    pub id: BookId,
    /// Ruta guardada.
    pub path: PathBuf,
    /// Tamaño guardado.
    pub file_size: u64,
    /// Si ya estaba marcado como desaparecido.
    pub missing: bool,
}

/// Resultado de recorrer el disco, listo para aplicar.
#[derive(Debug, Clone, Default)]
pub struct RescanPlan {
    known: Vec<KnownFile>,
    /// Para cada libro conocido, si su archivo existe.
    exists: Vec<bool>,
    /// Archivos de las carpetas que no están en la biblioteca.
    candidates: ScanOutcome,
}

/// Clave para reconocer un archivo movido: nombre (sin distinguir mayúsculas) y tamaño.
fn identity(path: &Path, size: u64) -> Option<(String, u64)> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    Some((name, size))
}

/// Clave de ruta sin distinguir mayúsculas (como en Windows y en la tabla `books`).
fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Comprueba qué archivos existen y busca libros en las carpetas.
///
/// Solo lee metadatos de los archivos que no estaban en la biblioteca.
pub fn plan_rescan<P: AsRef<Path>>(known: Vec<KnownFile>, folders: &[P]) -> RescanPlan {
    let exists = known.iter().map(|book| book.path.is_file()).collect();
    let known_paths: HashSet<String> = known.iter().map(|book| path_key(&book.path)).collect();
    let mut candidates = collect_paths(folders);
    candidates
        .books
        .retain(|book| !known_paths.contains(&path_key(&book.path)));
    RescanPlan {
        known,
        exists,
        candidates,
    }
}

impl Library {
    /// Libros con su ruta, para [`plan_rescan`].
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn known_files(&self) -> CoreResult<Vec<KnownFile>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id, path, file_size, missing FROM books")?;
        let files = stmt
            .query_map([], |row| {
                Ok(KnownFile {
                    id: BookId::new(row.get(0)?),
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    file_size: u64::try_from(row.get::<_, i64>(2)?).unwrap_or_default(),
                    missing: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(files)
    }

    /// Aplica el resultado de [`plan_rescan`]:
    /// - marca o desmarca los libros desaparecidos;
    /// - un archivo nuevo con el mismo nombre y tamaño que un libro desaparecido
    ///   se considera el mismo libro movido (conserva progreso, marcadores…);
    /// - el resto de archivos nuevos se añaden.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla (se deshace todo).
    pub fn apply_rescan(&self, plan: RescanPlan, at: Timestamp) -> CoreResult<RescanReport> {
        let mut report = RescanReport {
            checked: plan.known.len(),
            failures: plan.candidates.failures,
            ..RescanReport::default()
        };
        let tx = self.transaction()?;

        // Libros desaparecidos ahora, indexados por nombre y tamaño.
        let mut gone: HashMap<(String, u64), BookId> = HashMap::new();
        for (book, exists) in plan.known.iter().zip(&plan.exists) {
            match (book.missing, *exists) {
                (false, false) => report.missing += 1,
                (true, true) => report.restored += 1,
                _ => {}
            }
            if book.missing == *exists {
                self.set_missing(book.id, !exists)?;
            }
            if !exists && let Some(key) = identity(&book.path, book.file_size) {
                gone.insert(key, book.id);
            }
        }

        for candidate in plan.candidates.books {
            let moved =
                identity(&candidate.path, candidate.file_size).and_then(|key| gone.remove(&key));
            match moved {
                Some(id) => {
                    self.relocate(id, &candidate.path, candidate.file_size)?;
                    report.relocated += 1;
                    // Ya no falta: no cuenta como desaparecido en este informe.
                    report.missing = report.missing.saturating_sub(1);
                }
                None => {
                    if matches!(
                        self.insert(&enrich(candidate), at)?,
                        super::InsertOutcome::Inserted(_)
                    ) {
                        report.added += 1;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// Marca o desmarca un libro como desaparecido.
    ///
    /// # Errors
    /// - [`CoreError::NotFound`] si no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn set_missing(&self, id: BookId, missing: bool) -> CoreResult<()> {
        let updated = self
            .conn
            .prepare_cached("UPDATE books SET missing = ?2 WHERE id = ?1")?
            .execute(params![id.get(), missing])?;
        if updated == 0 {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }

    /// Apunta un libro a una nueva ruta (archivo movido o renombrado).
    ///
    /// # Errors
    /// - [`CoreError::InvalidData`] si el archivo es de otro formato.
    /// - [`CoreError::AlreadyExists`] si esa ruta ya está en la biblioteca.
    /// - [`CoreError::NotFound`] si el libro no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn relocate(&self, id: BookId, path: &Path, file_size: u64) -> CoreResult<()> {
        let current = self.get(id)?;
        if BookFormat::from_path(path) != Some(current.format) {
            return Err(CoreError::InvalidData(format!(
                "el archivo debe ser de tipo {}",
                current.format.as_str().to_uppercase()
            )));
        }
        let text = path
            .to_str()
            .ok_or_else(|| CoreError::NonUnicodePath(path.to_path_buf()))?;
        let result = self
            .conn
            .prepare_cached(
                "UPDATE books SET path = ?2, file_size = ?3, missing = 0 WHERE id = ?1",
            )?
            .execute(params![
                id.get(),
                text,
                i64::try_from(file_size).unwrap_or(i64::MAX)
            ]);
        match result {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(CoreError::AlreadyExists(
                    "Ese archivo ya está en la biblioteca.".to_owned(),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use std::fs;

    use super::*;
    use crate::domain::LibraryQuery;

    fn touch(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().expect("padre")).expect("carpeta");
        fs::write(path, bytes).expect("escribir");
    }

    fn rescan(lib: &Library, folders: &[&Path]) -> RescanReport {
        let plan = plan_rescan(lib.known_files().expect("conocidos"), folders);
        lib.apply_rescan(plan, Timestamp::from_unix_seconds(5))
            .expect("aplicar")
    }

    fn import(lib: &Library, path: &Path) {
        lib.import_paths(&[path], Timestamp::from_unix_seconds(1))
            .expect("importar");
    }

    #[test]
    fn deleted_file_is_marked_missing_and_restored_when_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.cbz");
        touch(&file, b"x");
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &file);

        fs::remove_file(&file).expect("borrar");
        let gone = rescan(&lib, &[]);
        let flagged = lib.list(&LibraryQuery::default()).expect("listar")[0].missing;
        touch(&file, b"x");
        let back = rescan(&lib, &[]);
        assert_eq!((gone.missing, flagged, back.restored), (1, true, 1));
    }

    #[test]
    fn moved_file_keeps_its_progress() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = dir.path().join("viejo").join("Saga.cbz");
        let new = dir.path().join("nuevo").join("Saga.cbz");
        touch(&old, b"contenido");
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &old);
        let id = lib.list(&LibraryQuery::default()).expect("listar")[0].id;
        lib.save_progress(
            id,
            crate::Locator::Page { index: 3 },
            0.3,
            Timestamp::from_unix_seconds(2),
        )
        .expect("progreso");

        fs::rename(&old, {
            fs::create_dir_all(new.parent().expect("padre")).expect("carpeta");
            &new
        })
        .expect("mover");
        let report = rescan(&lib, &[dir.path()]);
        let book = lib.get(id).expect("libro");
        assert_eq!(
            (
                report.relocated,
                report.added,
                report.missing,
                book.path,
                book.progress,
                book.missing
            ),
            (1, 0, 0, new, Some(0.3), false)
        );
    }

    #[test]
    fn new_files_in_folders_are_added() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(&dir.path().join("uno.epub"), b"x");
        touch(&dir.path().join("dos.cbz"), b"y");
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &dir.path().join("uno.epub"));
        let report = rescan(&lib, &[dir.path()]);
        assert_eq!((report.checked, report.added), (1, 1));
    }

    #[test]
    fn same_name_with_different_size_is_a_new_book() {
        let dir = tempfile::tempdir().expect("tempdir");
        let old = dir.path().join("a").join("x.cbz");
        touch(&old, b"1");
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &old);
        fs::remove_file(&old).expect("borrar");
        touch(&dir.path().join("b").join("x.cbz"), b"22");
        let report = rescan(&lib, &[dir.path()]);
        assert_eq!((report.relocated, report.added, report.missing), (0, 1, 1));
    }

    #[test]
    fn relocate_rejects_other_format() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.cbz");
        touch(&file, b"x");
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &file);
        let id = lib.list(&LibraryQuery::default()).expect("listar")[0].id;
        assert!(matches!(
            lib.relocate(id, &dir.path().join("a.pdf"), 1),
            Err(CoreError::InvalidData(_))
        ));
    }

    #[test]
    fn relocate_to_a_path_already_in_library_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (a, b) = (dir.path().join("a.cbz"), dir.path().join("b.cbz"));
        touch(&a, b"x");
        touch(&b, b"y");
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &a);
        import(&lib, &b);
        let id = lib
            .list(&LibraryQuery::default())
            .expect("listar")
            .into_iter()
            .find(|book| book.title == "a")
            .expect("a")
            .id;
        assert!(matches!(
            lib.relocate(id, &b, 1),
            Err(CoreError::AlreadyExists(_))
        ));
    }
}
