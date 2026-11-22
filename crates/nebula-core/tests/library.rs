//! Tests de integración de la biblioteca sobre archivos reales en disco.

#![allow(clippy::expect_used)] // En tests, un fallo de infraestructura debe abortar con mensaje.

use nebula_core::library::{InsertOutcome, Library};
use nebula_core::{CoreError, CoverStatus, LibraryQuery, NewBook, Timestamp};

fn new_book(path: &str) -> NewBook {
    NewBook::from_path(path, 42).expect("formato soportado")
}

#[test]
fn books_and_settings_persist_after_reopening() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = dir.path().join("library.db");

    {
        let lib = Library::open(&db).expect("abrir biblioteca");
        let outcome = lib
            .insert(&new_book("C:/a.cbz"), Timestamp::from_unix_seconds(1))
            .expect("insertar");
        assert!(matches!(outcome, InsertOutcome::Inserted(_)));
        lib.set_setting("theme", "dark").expect("guardar ajuste");
    }

    let lib = Library::open(&db).expect("reabrir biblioteca");
    let books = lib.list(&LibraryQuery::default()).expect("listar");
    assert_eq!(books.len(), 1);
    assert_eq!(
        lib.setting("theme").expect("leer ajuste").as_deref(),
        Some("dark")
    );
}

#[test]
fn reopening_does_not_reapply_migrations() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = dir.path().join("library.db");

    let first = Library::open(&db)
        .expect("abrir")
        .schema_version()
        .expect("versión");
    // Si la migración 1 se reaplicara, `CREATE TABLE books` fallaría.
    let second = Library::open(&db)
        .expect("reabrir")
        .schema_version()
        .expect("versión");
    assert_eq!(first, second);
    assert!(first >= 1);
}

#[test]
fn open_creates_missing_parent_directories() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = dir
        .path()
        .join("NebulaReader")
        .join("data")
        .join("library.db");
    assert!(Library::open(&db).is_ok());
    assert!(db.exists());
}

#[test]
fn open_enables_wal_journal() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = dir.path().join("library.db");
    let _lib = Library::open(&db).expect("abrir");

    let conn = rusqlite::Connection::open(&db).expect("conexión de inspección");
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("leer journal_mode");
    assert_eq!(mode, "wal");
}

#[test]
fn database_from_newer_version_is_rejected() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = dir.path().join("library.db");
    rusqlite::Connection::open(&db)
        .expect("crear base de datos")
        .pragma_update(None, "user_version", 99)
        .expect("fijar versión futura");

    assert!(matches!(
        Library::open(&db),
        Err(CoreError::SchemaTooNew { found: 99, .. })
    ));
}

#[test]
fn version_1_database_is_migrated_without_data_loss() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = dir.path().join("library.db");
    // Esquema exacto publicado en v0.1.0 (migración 1).
    let conn = rusqlite::Connection::open(&db).expect("crear base de datos");
    conn.execute_batch(
        "CREATE TABLE books (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE COLLATE NOCASE,
            title TEXT NOT NULL,
            format TEXT NOT NULL CHECK (format IN ('cbz', 'cbr', 'cb7', 'epub', 'pdf')),
            file_size INTEGER NOT NULL CHECK (file_size >= 0),
            added_at INTEGER NOT NULL,
            last_opened_at INTEGER
        ) STRICT;
        CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
        INSERT INTO books (path, title, format, file_size, added_at)
            VALUES ('C:/saga.cbz', 'Saga', 'cbz', 10, 5);
        INSERT INTO settings VALUES ('theme', 'dark');
        PRAGMA user_version = 1;",
    )
    .expect("esquema v1");
    drop(conn);

    let lib = Library::open(&db).expect("migrar");
    let books = lib.list(&LibraryQuery::default()).expect("listar");
    assert_eq!(
        books
            .iter()
            .map(|b| (b.title.as_str(), b.page_count, b.cover, b.progress))
            .collect::<Vec<_>>(),
        [("Saga", None, CoverStatus::Pending, None)]
    );
    assert_eq!(
        lib.setting("theme").expect("ajuste").as_deref(),
        Some("dark")
    );
}
