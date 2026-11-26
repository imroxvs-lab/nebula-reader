//! Importación de archivos y carpetas a la biblioteca.
//!
//! Se divide en dos pasos para no bloquear la biblioteca mientras se recorre el
//! disco:
//! 1. [`scan_paths`] recorre el sistema de archivos (lento, sin tocar SQLite).
//! 2. [`Library::add_scanned`] inserta el resultado en una sola transacción.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::domain::{BookFormat, ImportFailure, ImportReport, NewBook, Timestamp};
use crate::error::{CoreError, CoreResult};
use crate::library::{InsertOutcome, Library};
use crate::metadata::read_metadata;

/// Resultado de recorrer las rutas a importar.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanOutcome {
    /// Archivos con formato soportado, listos para insertar.
    pub books: Vec<NewBook>,
    /// Archivos seleccionados explícitamente con formato no soportado.
    ///
    /// Dentro de carpetas los archivos no soportados (imágenes, notas…) se
    /// ignoran sin contarlos, para no llenar el informe de ruido.
    pub unsupported: usize,
    /// Rutas que no se pudieron leer.
    pub failures: Vec<ImportFailure>,
}

/// Recorre archivos y carpetas (recursivamente) buscando libros y cómics.
///
/// Las rutas se convierten en absolutas sin resolver enlaces, y las carpetas se
/// recorren en orden alfabético sin seguir enlaces simbólicos. Nunca falla en
/// bloque: cada problema se anota en [`ScanOutcome::failures`].
pub fn scan_paths<P: AsRef<Path>>(paths: &[P]) -> ScanOutcome {
    let mut outcome = collect_paths(paths);
    outcome.books = outcome.books.into_iter().map(enrich).collect();
    outcome
}

/// Como [`scan_paths`] pero sin leer metadatos (más rápido: solo recorre el disco).
pub(crate) fn collect_paths<P: AsRef<Path>>(paths: &[P]) -> ScanOutcome {
    let mut outcome = ScanOutcome::default();
    for path in paths {
        scan_one(path.as_ref(), &mut outcome);
    }
    outcome
}

fn scan_one(path: &Path, outcome: &mut ScanOutcome) {
    let path = match std::path::absolute(path) {
        Ok(path) => path,
        Err(e) => return outcome.fail(path.to_path_buf(), &e),
    };
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(e) => return outcome.fail(path, &e),
    };

    if metadata.is_dir() {
        scan_dir(&path, outcome);
    } else if metadata.is_file() {
        match NewBook::from_path(path, metadata.len()) {
            Ok(book) => outcome.books.push(book),
            Err(_) => outcome.unsupported += 1,
        }
    } else {
        outcome.failures.push(ImportFailure {
            path,
            reason: "no es un archivo ni una carpeta".to_owned(),
        });
    }
}

fn scan_dir(dir: &Path, outcome: &mut ScanOutcome) {
    let entries = WalkDir::new(dir)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let path = e.path().unwrap_or(dir).to_path_buf();
                outcome.failures.push(ImportFailure {
                    path,
                    reason: e.to_string(),
                });
                continue;
            }
        };
        if !entry.file_type().is_file() || BookFormat::from_path(entry.path()).is_none() {
            continue;
        }
        let size = match entry.metadata() {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                outcome.failures.push(ImportFailure {
                    path: entry.path().to_path_buf(),
                    reason: e.to_string(),
                });
                continue;
            }
        };
        // El formato ya se ha comprobado, así que `from_path` no puede fallar aquí;
        // aun así se trata el error en vez de suponerlo.
        match NewBook::from_path(entry.into_path(), size) {
            Ok(book) => outcome.books.push(book),
            Err(_) => outcome.unsupported += 1,
        }
    }
}

/// Añade los metadatos del archivo. Un archivo ilegible se importa igualmente
/// (con el título del nombre): al abrirlo, el usuario verá el error concreto.
pub(crate) fn enrich(book: NewBook) -> NewBook {
    match read_metadata(&book.path, book.format) {
        Ok(metadata) => book.with_metadata(metadata),
        Err(error) => {
            tracing::debug!(path = %book.path.display(), %error, "sin metadatos");
            book
        }
    }
}

impl ScanOutcome {
    fn fail(&mut self, path: PathBuf, error: &std::io::Error) {
        self.failures.push(ImportFailure {
            path,
            reason: error.to_string(),
        });
    }
}

impl Library {
    /// Inserta en una transacción los libros encontrados por [`scan_paths`].
    ///
    /// Los duplicados se cuentan sin modificar nada. Las rutas que no se pueden
    /// almacenar se anotan como fallos. Cualquier otro error de base de datos
    /// deshace toda la importación.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn add_scanned(&self, scan: ScanOutcome, added_at: Timestamp) -> CoreResult<ImportReport> {
        let mut report = ImportReport {
            unsupported: scan.unsupported,
            failures: scan.failures,
            ..ImportReport::default()
        };

        let tx = self.transaction()?;
        for book in &scan.books {
            match self.insert(book, added_at) {
                Ok(InsertOutcome::Inserted(_)) => report.added += 1,
                Ok(InsertOutcome::Duplicate) => report.skipped_duplicates += 1,
                Err(CoreError::NonUnicodePath(path)) => report.failures.push(ImportFailure {
                    path,
                    reason: "la ruta contiene caracteres no válidos".to_owned(),
                }),
                Err(e) => return Err(e),
            }
        }
        tx.commit()?;

        tracing::info!(
            added = report.added,
            duplicates = report.skipped_duplicates,
            unsupported = report.unsupported,
            failures = report.failures.len(),
            "importación completada"
        );
        Ok(report)
    }

    /// Atajo que escanea e importa en el mismo hilo.
    ///
    /// La capa de aplicación prefiere llamar a [`scan_paths`] sin bloquear la
    /// biblioteca y luego a [`Library::add_scanned`].
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn import_paths<P: AsRef<Path>>(
        &self,
        paths: &[P],
        added_at: Timestamp,
    ) -> CoreResult<ImportReport> {
        self.add_scanned(scan_paths(paths), added_at)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use std::fs;

    use super::*;
    use crate::domain::LibraryQuery;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("crear carpeta");
        }
        fs::write(path, b"x").expect("crear archivo");
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let root = dir.path();
        touch(&root.join("Saga_01.cbz"));
        touch(&root.join("notas.txt"));
        touch(&root.join("portada.jpg"));
        touch(&root.join("sub").join("Dune.epub"));
        touch(&root.join("sub").join("deep").join("Manual.PDF"));
        dir
    }

    fn import(lib: &Library, paths: &[PathBuf]) -> ImportReport {
        lib.import_paths(paths, Timestamp::from_unix_seconds(1))
            .expect("importar")
    }

    #[test]
    fn folder_scan_finds_supported_files_recursively() {
        let dir = fixture();
        let mut titles: Vec<_> = scan_paths(&[dir.path()])
            .books
            .into_iter()
            .map(|b| b.title)
            .collect();
        titles.sort();
        assert_eq!(titles, ["Dune", "Manual", "Saga 01"]);
    }

    #[test]
    fn unsupported_files_inside_folders_are_not_counted() {
        let dir = fixture();
        assert_eq!(scan_paths(&[dir.path()]).unsupported, 0);
    }

    #[test]
    fn explicitly_selected_unsupported_file_is_counted() {
        let dir = fixture();
        let scan = scan_paths(&[dir.path().join("notas.txt")]);
        assert_eq!((scan.books.len(), scan.unsupported), (0, 1));
    }

    #[test]
    fn missing_path_is_reported_as_failure() {
        let dir = fixture();
        let missing = dir.path().join("no-existe.cbz");
        let scan = scan_paths(std::slice::from_ref(&missing));
        assert_eq!(
            scan.failures.iter().map(|f| &f.path).collect::<Vec<_>>(),
            [&missing]
        );
    }

    #[test]
    fn scanned_paths_are_absolute() {
        let dir = fixture();
        let scan = scan_paths(&[dir.path()]);
        assert!(scan.books.iter().all(|b| b.path.is_absolute()));
    }

    #[test]
    fn scan_records_file_size() {
        let dir = fixture();
        let scan = scan_paths(&[dir.path().join("Saga_01.cbz")]);
        assert_eq!(scan.books.first().map(|b| b.file_size), Some(1));
    }

    #[test]
    fn import_reports_added_and_unsupported() {
        let dir = fixture();
        let lib = Library::open_in_memory().expect("biblioteca");
        let report = import(
            &lib,
            &[dir.path().to_path_buf(), dir.path().join("notas.txt")],
        );
        assert_eq!(
            (report.added, report.skipped_duplicates, report.unsupported),
            (3, 0, 1)
        );
    }

    #[test]
    fn importing_twice_counts_duplicates() {
        let dir = fixture();
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &[dir.path().to_path_buf()]);
        let report = import(&lib, &[dir.path().to_path_buf()]);
        assert_eq!((report.added, report.skipped_duplicates), (0, 3));
    }

    #[test]
    fn same_file_selected_twice_is_added_once() {
        let dir = fixture();
        let file = dir.path().join("Saga_01.cbz");
        let lib = Library::open_in_memory().expect("biblioteca");
        let report = import(&lib, &[file.clone(), file]);
        assert_eq!((report.added, report.skipped_duplicates), (1, 1));
    }

    #[test]
    fn comic_info_metadata_is_imported() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().expect("directorio temporal");
        let path = dir.path().join("saga_012.cbz");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).expect("crear"));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("ComicInfo.xml", options).expect("entrada");
        zip.write_all(b"<ComicInfo><Series>Saga</Series><Writer>Vaughan</Writer></ComicInfo>")
            .expect("escribir");
        zip.start_file("01.jpg", options).expect("entrada");
        zip.finish().expect("cerrar");

        let scan = scan_paths(&[&path]);
        let meta = scan.books.first().map(|b| b.metadata.clone());
        assert_eq!(
            meta.map(|m| (m.series, m.author)),
            Some((Some("Saga".to_owned()), Some("Vaughan".to_owned())))
        );
    }

    #[test]
    fn unreadable_file_is_imported_with_filename_title() {
        let dir = fixture();
        let scan = scan_paths(&[dir.path().join("sub").join("Dune.epub")]);
        assert_eq!(scan.books.first().map(|b| b.title.as_str()), Some("Dune"));
    }

    #[test]
    fn imported_books_are_listed() {
        let dir = fixture();
        let lib = Library::open_in_memory().expect("biblioteca");
        import(&lib, &[dir.path().to_path_buf()]);
        let books = lib.list(&LibraryQuery::default()).expect("listar");
        assert_eq!(books.len(), 3);
    }
}
