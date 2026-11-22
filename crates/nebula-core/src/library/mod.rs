//! Biblioteca persistente en SQLite.
//!
//! [`Library`] envuelve una única [`rusqlite::Connection`]. No es `Sync`: la capa
//! de aplicación la protege con un `Mutex` y la usa desde hilos bloqueantes.

mod migrations;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::domain::{
    Book, BookFormat, BookId, BookKind, LibraryQuery, LibraryStats, NewBook, SortBy, Timestamp,
};
use crate::error::{CoreError, CoreResult};

/// Tiempo máximo de espera si otro proceso tiene la base de datos bloqueada.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Columnas de `books` en el orden que espera [`book_from_row`].
const BOOK_COLUMNS: &str = "id, path, title, format, file_size, added_at, last_opened_at";

/// Resultado de intentar insertar un libro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// Se añadió con este identificador.
    Inserted(BookId),
    /// Ya existía un libro con la misma ruta; no se modificó nada.
    Duplicate,
}

/// Biblioteca de libros respaldada por SQLite.
#[derive(Debug)]
pub struct Library {
    conn: Connection,
}

impl Library {
    /// Abre (o crea) la biblioteca en `path` y aplica las migraciones pendientes.
    ///
    /// Crea el directorio padre si no existe y activa el modo WAL.
    ///
    /// # Errors
    /// - [`CoreError::Io`] si no se puede crear el directorio.
    /// - [`CoreError::SchemaTooNew`] si la base de datos es de una versión futura.
    /// - [`CoreError::Db`] si SQLite falla al abrir o migrar.
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let conn = Connection::open(path)?;
        // WAL: lecturas concurrentes con escrituras y menos fsync.
        // `synchronous = NORMAL` es seguro con WAL (solo puede perder la última
        // transacción ante un corte de luz, nunca corromper).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    /// Abre una biblioteca en memoria, útil para pruebas.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn open_in_memory() -> CoreResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> CoreResult<Self> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Abre una transacción sobre la conexión compartida. Los métodos de
    /// `Library` llamados mientras viva forman parte de ella; si se descarta sin
    /// `commit`, se deshace.
    pub(crate) fn transaction(&self) -> CoreResult<rusqlite::Transaction<'_>> {
        Ok(self.conn.unchecked_transaction()?)
    }

    /// Versión del esquema aplicada.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn schema_version(&self) -> CoreResult<u32> {
        migrations::current_version(&self.conn)
    }

    /// Inserta un libro con fecha de alta `added_at`. Si ya existe uno con la
    /// misma ruta (sin distinguir mayúsculas), no hace nada.
    ///
    /// # Errors
    /// - [`CoreError::NonUnicodePath`] si la ruta no es Unicode válido.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn insert(&self, book: &NewBook, added_at: Timestamp) -> CoreResult<InsertOutcome> {
        let path = path_to_str(&book.path)?;
        let file_size = i64::try_from(book.file_size).unwrap_or(i64::MAX);
        let id = self
            .conn
            .prepare_cached(
                "INSERT INTO books (path, title, format, file_size, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (path) DO NOTHING
                 RETURNING id",
            )?
            .query_row(
                params![
                    path,
                    book.title,
                    book.format,
                    file_size,
                    added_at.as_unix_seconds()
                ],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        Ok(id.map_or(InsertOutcome::Duplicate, |id| {
            InsertOutcome::Inserted(BookId::new(id))
        }))
    }

    /// Obtiene un libro por su identificador.
    ///
    /// # Errors
    /// - [`CoreError::NotFound`] si no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn get(&self, id: BookId) -> CoreResult<Book> {
        self.conn
            .prepare_cached(
                "SELECT id, path, title, format, file_size, added_at, last_opened_at
                 FROM books WHERE id = ?1",
            )?
            .query_row([id.get()], book_from_row)
            .optional()?
            .ok_or(CoreError::NotFound(id))
    }

    /// Lista los libros que cumplen la consulta.
    ///
    /// La búsqueda es por subcadena del título; distingue mayúsculas solo fuera
    /// del rango ASCII (limitación de `LIKE` en SQLite).
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn list(&self, query: &LibraryQuery) -> CoreResult<Vec<Book>> {
        let pattern = query.search_term().map(like_pattern);
        let sql = format!(
            "SELECT {BOOK_COLUMNS} FROM books
             WHERE (?1 IS NULL OR title LIKE ?1 ESCAPE '\\')
               AND {kind}
             ORDER BY {order}
             LIMIT coalesce(?2, -1)", // En SQLite, LIMIT -1 = sin límite.
            kind = kind_clause(query.kind),
            order = order_clause(query.sort),
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let books = stmt
            .query_map(params![pattern, query.limit], book_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(books)
    }

    /// Número total de libros.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn count(&self) -> CoreResult<u64> {
        let stats = self.stats()?;
        Ok(stats.books + stats.comics)
    }

    /// Recuento de la biblioteca por categoría.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn stats(&self) -> CoreResult<LibraryStats> {
        let sql = format!(
            "SELECT coalesce(sum({comic}), 0), coalesce(sum(NOT {comic}), 0) FROM books",
            comic = kind_clause(Some(BookKind::Comic)),
        );
        let (comics, books): (i64, i64) = self
            .conn
            .prepare_cached(&sql)?
            .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(LibraryStats {
            books: u64::try_from(books).unwrap_or_default(),
            comics: u64::try_from(comics).unwrap_or_default(),
        })
    }

    /// Quita un libro de la biblioteca (no borra el archivo).
    ///
    /// # Errors
    /// - [`CoreError::NotFound`] si no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn remove(&self, id: BookId) -> CoreResult<()> {
        let deleted = self
            .conn
            .prepare_cached("DELETE FROM books WHERE id = ?1")?
            .execute([id.get()])?;
        if deleted == 0 {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }

    /// Registra que el libro se abrió en `at`.
    ///
    /// # Errors
    /// - [`CoreError::NotFound`] si no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn mark_opened(&self, id: BookId, at: Timestamp) -> CoreResult<()> {
        let updated = self
            .conn
            .prepare_cached("UPDATE books SET last_opened_at = ?2 WHERE id = ?1")?
            .execute(params![id.get(), at.as_unix_seconds()])?;
        if updated == 0 {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }

    /// Valor de un ajuste, si existe.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn setting(&self, key: &str) -> CoreResult<Option<String>> {
        Ok(self
            .conn
            .prepare_cached("SELECT value FROM settings WHERE key = ?1")?
            .query_row([key], |row| row.get(0))
            .optional()?)
    }

    /// Guarda (o reemplaza) un ajuste.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn set_setting(&self, key: &str, value: &str) -> CoreResult<()> {
        self.conn
            .prepare_cached(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )?
            .execute([key, value])?;
        Ok(())
    }
}

fn path_to_str(path: &Path) -> CoreResult<&str> {
    path.to_str()
        .ok_or_else(|| CoreError::NonUnicodePath(path.to_path_buf()))
}

/// Patrón `LIKE` que busca `term` como subcadena literal.
fn like_pattern(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for c in term.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

/// Condición SQL para filtrar por categoría. Solo interpola los identificadores
/// constantes de [`BookFormat::as_str`], nunca datos del usuario.
fn kind_clause(kind: Option<BookKind>) -> String {
    let Some(kind) = kind else {
        return "1 = 1".to_owned();
    };
    let formats = BookFormat::ALL
        .into_iter()
        .filter(|f| f.kind() == kind)
        .map(|f| format!("'{}'", f.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("format IN ({formats})")
}

const fn order_clause(sort: SortBy) -> &'static str {
    match sort {
        SortBy::Title => "title COLLATE NOCASE ASC, id ASC",
        SortBy::AddedAt => "added_at DESC, id DESC",
        SortBy::LastOpened => {
            "last_opened_at IS NULL ASC, last_opened_at DESC, added_at DESC, id DESC"
        }
    }
}

fn book_from_row(row: &Row<'_>) -> rusqlite::Result<Book> {
    let file_size: i64 = row.get(4)?;
    Ok(Book {
        id: BookId::new(row.get(0)?),
        path: PathBuf::from(row.get::<_, String>(1)?),
        title: row.get(2)?,
        format: row.get(3)?,
        file_size: u64::try_from(file_size).unwrap_or_default(),
        added_at: Timestamp::from_unix_seconds(row.get(5)?),
        last_opened_at: row
            .get::<_, Option<i64>>(6)?
            .map(Timestamp::from_unix_seconds),
    })
}

impl ToSql for BookFormat {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for BookFormat {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()?
            .parse()
            .map_err(|e: CoreError| FromSqlError::Other(Box::new(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        #[allow(clippy::expect_used)] // Fallo de infraestructura de test.
        Library::open_in_memory().expect("biblioteca en memoria")
    }

    fn new_book(path: &str) -> NewBook {
        #[allow(clippy::expect_used)] // Rutas de test con extensión válida.
        NewBook::from_path(path, 100).expect("formato soportado")
    }

    fn insert(lib: &Library, path: &str, added_at: i64) -> BookId {
        match lib.insert(&new_book(path), Timestamp::from_unix_seconds(added_at)) {
            Ok(InsertOutcome::Inserted(id)) => id,
            other => panic!("inserción inesperada de {path}: {other:?}"),
        }
    }

    fn titles(books: &[Book]) -> Vec<&str> {
        books.iter().map(|b| b.title.as_str()).collect()
    }

    fn list(lib: &Library, query: &LibraryQuery) -> Vec<Book> {
        match lib.list(query) {
            Ok(books) => books,
            Err(e) => panic!("list falló: {e}"),
        }
    }

    #[test]
    fn new_library_is_at_latest_schema() {
        assert_eq!(
            library().schema_version().ok(),
            Some(migrations::latest_version())
        );
    }

    #[test]
    fn inserted_book_can_be_read_back() {
        let lib = library();
        let id = insert(&lib, "C:/comics/Saga_01.cbz", 1_000);
        let book = lib.get(id).ok();
        assert_eq!(
            book,
            Some(Book {
                id,
                path: PathBuf::from("C:/comics/Saga_01.cbz"),
                title: "Saga 01".to_owned(),
                format: BookFormat::Cbz,
                file_size: 100,
                added_at: Timestamp::from_unix_seconds(1_000),
                last_opened_at: None,
            })
        );
    }

    #[test]
    fn duplicate_path_is_ignored() {
        let lib = library();
        insert(&lib, "C:/a.epub", 1);
        let outcome = lib.insert(&new_book("C:/a.epub"), Timestamp::from_unix_seconds(2));
        assert_eq!(outcome.ok(), Some(InsertOutcome::Duplicate));
    }

    #[test]
    fn duplicate_detection_ignores_path_case() {
        let lib = library();
        insert(&lib, "C:/Libros/A.epub", 1);
        let outcome = lib.insert(
            &new_book("c:/libros/a.EPUB"),
            Timestamp::from_unix_seconds(2),
        );
        assert_eq!(outcome.ok(), Some(InsertOutcome::Duplicate));
    }

    #[test]
    fn get_missing_book_is_not_found() {
        assert!(matches!(
            library().get(BookId::new(99)),
            Err(CoreError::NotFound(id)) if id == BookId::new(99)
        ));
    }

    #[test]
    fn default_list_is_newest_first() {
        let lib = library();
        insert(&lib, "C:/old.pdf", 1);
        insert(&lib, "C:/new.pdf", 3);
        insert(&lib, "C:/mid.pdf", 2);
        assert_eq!(
            titles(&list(&lib, &LibraryQuery::default())),
            ["new", "mid", "old"]
        );
    }

    #[test]
    fn title_sort_is_case_insensitive() {
        let lib = library();
        insert(&lib, "C:/beta.pdf", 1);
        insert(&lib, "C:/Alpha.pdf", 2);
        insert(&lib, "C:/gamma.pdf", 3);
        let query = LibraryQuery {
            sort: SortBy::Title,
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["Alpha", "beta", "gamma"]);
    }

    #[test]
    fn last_opened_sort_puts_never_opened_last() {
        let lib = library();
        insert(&lib, "C:/never.pdf", 10);
        let early = insert(&lib, "C:/early.pdf", 1);
        let late = insert(&lib, "C:/late.pdf", 2);
        assert!(
            lib.mark_opened(early, Timestamp::from_unix_seconds(100))
                .is_ok()
        );
        assert!(
            lib.mark_opened(late, Timestamp::from_unix_seconds(200))
                .is_ok()
        );
        let query = LibraryQuery {
            sort: SortBy::LastOpened,
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["late", "early", "never"]);
    }

    #[test]
    fn kind_filter_keeps_only_comics() {
        let lib = library();
        insert(&lib, "C:/a.cbz", 1);
        insert(&lib, "C:/b.epub", 2);
        insert(&lib, "C:/c.cbr", 3);
        insert(&lib, "C:/d.pdf", 4);
        let query = LibraryQuery {
            kind: Some(BookKind::Comic),
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["c", "a"]);
    }

    #[test]
    fn kind_filter_keeps_only_books() {
        let lib = library();
        insert(&lib, "C:/a.cbz", 1);
        insert(&lib, "C:/b.epub", 2);
        insert(&lib, "C:/c.cb7", 3);
        insert(&lib, "C:/d.pdf", 4);
        let query = LibraryQuery {
            kind: Some(BookKind::Book),
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["d", "b"]);
    }

    #[test]
    fn search_matches_substring_ignoring_ascii_case() {
        let lib = library();
        insert(&lib, "C:/One_Piece_01.cbz", 1);
        insert(&lib, "C:/Naruto_01.cbz", 2);
        let query = LibraryQuery {
            search: Some("piece".to_owned()),
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["One Piece 01"]);
    }

    #[test]
    fn search_treats_wildcards_literally() {
        let lib = library();
        insert(&lib, "C:/100% Cómic.cbz", 1);
        insert(&lib, "C:/1000 Cómic.cbz", 2);
        let query = LibraryQuery {
            search: Some("100%".to_owned()),
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["100% Cómic"]);
    }

    #[test]
    fn search_and_kind_filters_combine() {
        let lib = library();
        insert(&lib, "C:/Saga 1.cbz", 1);
        insert(&lib, "C:/Saga guía.pdf", 2);
        let query = LibraryQuery {
            search: Some("saga".to_owned()),
            kind: Some(BookKind::Book),
            sort: SortBy::Title,
            limit: None,
        };
        assert_eq!(titles(&list(&lib, &query)), ["Saga guía"]);
    }

    #[test]
    fn limit_truncates_results() {
        let lib = library();
        insert(&lib, "C:/a.pdf", 1);
        insert(&lib, "C:/b.pdf", 2);
        insert(&lib, "C:/c.pdf", 3);
        let query = LibraryQuery {
            limit: Some(2),
            ..LibraryQuery::default()
        };
        assert_eq!(titles(&list(&lib, &query)), ["c", "b"]);
    }

    #[test]
    fn stats_count_books_and_comics() {
        let lib = library();
        insert(&lib, "C:/a.cbz", 1);
        insert(&lib, "C:/b.cb7", 2);
        insert(&lib, "C:/c.epub", 3);
        assert_eq!(
            lib.stats().ok(),
            Some(LibraryStats {
                books: 1,
                comics: 2
            })
        );
    }

    #[test]
    fn stats_of_empty_library_are_zero() {
        assert_eq!(library().stats().ok(), Some(LibraryStats::default()));
    }

    #[test]
    fn remove_deletes_book() {
        let lib = library();
        let id = insert(&lib, "C:/a.pdf", 1);
        assert!(lib.remove(id).is_ok());
        assert_eq!(lib.count().ok(), Some(0));
    }

    #[test]
    fn remove_missing_book_is_not_found() {
        assert!(matches!(
            library().remove(BookId::new(5)),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn mark_opened_missing_book_is_not_found() {
        assert!(matches!(
            library().mark_opened(BookId::new(5), Timestamp::from_unix_seconds(1)),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn missing_setting_is_none() {
        assert_eq!(library().setting("theme").ok(), Some(None));
    }

    #[test]
    fn setting_is_overwritten() {
        let lib = library();
        assert!(lib.set_setting("theme", "dark").is_ok());
        assert!(lib.set_setting("theme", "light").is_ok());
        assert_eq!(lib.setting("theme").ok(), Some(Some("light".to_owned())));
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern(r"a%b_c\d"), r"%a\%b\_c\\d%");
    }
}
