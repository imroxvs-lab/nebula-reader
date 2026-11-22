//! Migraciones del esquema SQLite.
//!
//! La versión aplicada se guarda en `PRAGMA user_version`. Cada migración se
//! ejecuta en su propia transacción junto con la actualización de la versión, de
//! modo que un fallo deja la base de datos en la versión anterior.
//!
//! **Nunca se modifica una migración publicada**: los cambios van en una nueva
//! entrada al final de [`MIGRATIONS`].

use rusqlite::Connection;

use crate::error::{CoreError, CoreResult};

/// Migraciones en orden. La migración `i` lleva el esquema a la versión `i + 1`.
const MIGRATIONS: &[&str] = &[
    // 1: biblioteca básica y ajustes.
    //
    // `path` usa `COLLATE NOCASE` porque en Windows las rutas no distinguen
    // mayúsculas: `C:\A.cbz` y `c:\a.cbz` son el mismo archivo.
    r"
    CREATE TABLE books (
        id             INTEGER PRIMARY KEY,
        path           TEXT    NOT NULL UNIQUE COLLATE NOCASE,
        title          TEXT    NOT NULL,
        format         TEXT    NOT NULL CHECK (format IN ('cbz', 'cbr', 'cb7', 'epub', 'pdf')),
        file_size      INTEGER NOT NULL CHECK (file_size >= 0),
        added_at       INTEGER NOT NULL,
        last_opened_at INTEGER
    ) STRICT;

    CREATE INDEX books_title_idx       ON books (title COLLATE NOCASE);
    CREATE INDEX books_added_at_idx    ON books (added_at);
    CREATE INDEX books_last_opened_idx ON books (last_opened_at);

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) STRICT;
    ",
    // 2: lector de cómics. Número de páginas, estado de la portada y progreso.
    //
    // La ruta de la miniatura no se guarda: se deriva del id (`thumbs/<id>.jpg`)
    // para que mover la carpeta de datos no deje rutas obsoletas.
    r"
    ALTER TABLE books ADD COLUMN page_count INTEGER CHECK (page_count >= 0);
    ALTER TABLE books ADD COLUMN cover_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (cover_status IN ('pending', 'ready', 'failed'));

    CREATE TABLE progress (
        book_id    INTEGER PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
        locator    TEXT    NOT NULL,
        percent    REAL    NOT NULL CHECK (percent >= 0.0 AND percent <= 1.0),
        updated_at INTEGER NOT NULL
    ) STRICT;
    ",
];

/// Versión de esquema más reciente que conoce esta compilación.
pub(crate) fn latest_version() -> u32 {
    // Hay pocas migraciones; la conversión no puede desbordar en la práctica.
    u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX)
}

/// Versión de esquema de la base de datos.
pub(crate) fn current_version(conn: &Connection) -> CoreResult<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Aplica las migraciones pendientes.
///
/// # Errors
/// - [`CoreError::SchemaTooNew`] si la base de datos es de una versión futura.
/// - [`CoreError::Db`] si falla alguna migración (se revierte esa migración).
pub(crate) fn migrate(conn: &mut Connection) -> CoreResult<()> {
    let found = current_version(conn)?;
    let supported = latest_version();
    if found > supported {
        return Err(CoreError::SchemaTooNew { found, supported });
    }

    for (version, sql) in (1..=supported)
        .zip(MIGRATIONS)
        .skip_while(|(v, _)| *v <= found)
    {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        tracing::info!(version, "migración de la biblioteca aplicada");
    }
    Ok(())
}
