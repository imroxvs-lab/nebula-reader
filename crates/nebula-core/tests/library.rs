//! Tests de integración de la biblioteca sobre archivos reales en disco.

#![allow(clippy::expect_used)] // En tests, un fallo de infraestructura debe abortar con mensaje.

use nebula_core::library::{InsertOutcome, Library};
use nebula_core::{CoreError, LibraryQuery, NewBook, Timestamp};

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
    assert_eq!((first, second), (1, 1));
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
        Err(CoreError::SchemaTooNew {
            found: 99,
            supported: 1
        })
    ));
}
