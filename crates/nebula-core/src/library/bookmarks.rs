//! Marcadores dentro de los libros.

use rusqlite::params;

use super::Library;
use crate::domain::{BookId, Bookmark, BookmarkId, Locator, Timestamp};
use crate::error::{CoreError, CoreResult};

/// Longitud máxima de la etiqueta de un marcador.
pub const MAX_LABEL_CHARS: usize = 200;

impl Library {
    /// Marcadores de un libro, en orden de lectura.
    ///
    /// Los que tienen un localizador ilegible (de una versión futura) se omiten.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn bookmarks(&self, book: BookId) -> CoreResult<Vec<Bookmark>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, locator, percent, label, created_at FROM bookmarks
             WHERE book_id = ?1 ORDER BY percent, id",
        )?;
        let rows = stmt
            .query_map([book.get()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, locator, percent, label, created)| {
                Some(Bookmark {
                    id: BookmarkId::new(id),
                    locator: serde_json::from_str(&locator).ok()?,
                    percent,
                    label,
                    created_at: Timestamp::from_unix_seconds(created),
                })
            })
            .collect())
    }

    /// Añade un marcador. La etiqueta se recorta a [`MAX_LABEL_CHARS`].
    ///
    /// # Errors
    /// - [`CoreError::NotFound`] si el libro no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn add_bookmark(
        &self,
        book: BookId,
        locator: Locator,
        percent: f64,
        label: &str,
        at: Timestamp,
    ) -> CoreResult<Bookmark> {
        let percent = if percent.is_finite() {
            percent.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let label: String = crate::xml::collapse_whitespace(label)
            .chars()
            .take(MAX_LABEL_CHARS)
            .collect();
        let encoded =
            serde_json::to_string(&locator).map_err(|e| CoreError::InvalidData(e.to_string()))?;
        let result = self
            .conn
            .prepare_cached(
                "INSERT INTO bookmarks (book_id, locator, percent, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
            )?
            .query_row(
                params![book.get(), encoded, percent, label, at.as_unix_seconds()],
                |row| row.get::<_, i64>(0),
            );
        match result {
            Ok(id) => Ok(Bookmark {
                id: BookmarkId::new(id),
                locator,
                percent,
                label,
                created_at: at,
            }),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(CoreError::NotFound(book))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Borra un marcador. Borrar uno inexistente no es un error.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn remove_bookmark(&self, id: BookmarkId) -> CoreResult<()> {
        self.conn
            .prepare_cached("DELETE FROM bookmarks WHERE id = ?1")?
            .execute([id.get()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use super::*;
    use crate::domain::NewBook;
    use crate::library::InsertOutcome;

    fn setup() -> (Library, BookId) {
        let lib = Library::open_in_memory().expect("biblioteca");
        let book = NewBook::from_path("C:/a.cbz", 1).expect("libro");
        match lib.insert(&book, Timestamp::from_unix_seconds(1)) {
            Ok(InsertOutcome::Inserted(id)) => (lib, id),
            other => panic!("{other:?}"),
        }
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_unix_seconds(seconds)
    }

    #[test]
    fn bookmarks_are_listed_in_reading_order() {
        let (lib, id) = setup();
        lib.add_bookmark(id, Locator::Page { index: 9 }, 0.5, "Página 10", at(1))
            .expect("añadir");
        lib.add_bookmark(id, Locator::Page { index: 1 }, 0.1, "Página 2", at(2))
            .expect("añadir");
        let labels: Vec<String> = lib
            .bookmarks(id)
            .expect("listar")
            .into_iter()
            .map(|b| b.label)
            .collect();
        assert_eq!(labels, ["Página 2", "Página 10"]);
    }

    #[test]
    fn bookmark_label_is_trimmed_and_bounded() {
        let (lib, id) = setup();
        let long = format!("  {}  ", "x".repeat(500));
        let bookmark = lib
            .add_bookmark(id, Locator::Page { index: 0 }, 0.0, &long, at(1))
            .expect("añadir");
        assert_eq!(bookmark.label.chars().count(), MAX_LABEL_CHARS);
    }

    #[test]
    fn removed_bookmark_disappears() {
        let (lib, id) = setup();
        let bookmark = lib
            .add_bookmark(id, Locator::Page { index: 0 }, 0.0, "a", at(1))
            .expect("añadir");
        lib.remove_bookmark(bookmark.id).expect("borrar");
        assert!(lib.bookmarks(id).expect("listar").is_empty());
    }

    #[test]
    fn bookmarks_are_deleted_with_their_book() {
        let (lib, id) = setup();
        lib.add_bookmark(id, Locator::Page { index: 0 }, 0.0, "a", at(1))
            .expect("añadir");
        lib.remove(id).expect("quitar libro");
        assert!(lib.bookmarks(id).expect("listar").is_empty());
    }

    #[test]
    fn bookmark_for_missing_book_is_not_found() {
        let (lib, _) = setup();
        assert!(matches!(
            lib.add_bookmark(BookId::new(99), Locator::Page { index: 0 }, 0.0, "a", at(1)),
            Err(CoreError::NotFound(_))
        ));
    }
}
