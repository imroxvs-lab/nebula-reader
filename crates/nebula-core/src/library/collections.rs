//! Colecciones (estanterías) creadas por el usuario.

use rusqlite::params;

use super::Library;
use crate::domain::{BookId, Collection, CollectionId, Timestamp};
use crate::error::{CoreError, CoreResult};

/// Longitud máxima del nombre de una colección.
pub const MAX_NAME_CHARS: usize = 80;

/// Normaliza un nombre: espacios colapsados, sin vacíos y acotado.
fn clean_name(name: &str) -> CoreResult<String> {
    let name: String = crate::xml::collapse_whitespace(name)
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    if name.is_empty() {
        return Err(CoreError::InvalidData(
            "el nombre no puede estar vacío".to_owned(),
        ));
    }
    Ok(name)
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(e, _)
        if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE)
}

impl Library {
    /// Colecciones con su número de libros, por nombre.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn collections(&self) -> CoreResult<Vec<Collection>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT c.id, c.name, COUNT(cb.book_id) FROM collections c
             LEFT JOIN collection_books cb ON cb.collection_id = c.id
             GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
        )?;
        let collections = stmt
            .query_map([], |row| {
                Ok(Collection {
                    id: CollectionId::new(row.get(0)?),
                    name: row.get(1)?,
                    count: u64::try_from(row.get::<_, i64>(2)?).unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(collections)
    }

    /// Crea una colección.
    ///
    /// # Errors
    /// - [`CoreError::InvalidData`] si el nombre está vacío.
    /// - [`CoreError::AlreadyExists`] si ya hay una con ese nombre.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn create_collection(&self, name: &str, at: Timestamp) -> CoreResult<Collection> {
        let name = clean_name(name)?;
        let result = self
            .conn
            .prepare_cached(
                "INSERT INTO collections (name, created_at) VALUES (?1, ?2) RETURNING id",
            )?
            .query_row(params![name, at.as_unix_seconds()], |row| {
                row.get::<_, i64>(0)
            });
        match result {
            Ok(id) => Ok(Collection {
                id: CollectionId::new(id),
                name,
                count: 0,
            }),
            Err(e) if is_unique_violation(&e) => Err(CoreError::AlreadyExists(format!(
                "Ya existe una colección llamada «{name}»."
            ))),
            Err(e) => Err(e.into()),
        }
    }

    /// Cambia el nombre de una colección (si no existe, no hace nada).
    ///
    /// # Errors
    /// - [`CoreError::InvalidData`] si el nombre está vacío.
    /// - [`CoreError::AlreadyExists`] si ya hay otra con ese nombre.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn rename_collection(&self, id: CollectionId, name: &str) -> CoreResult<()> {
        let name = clean_name(name)?;
        let result = self
            .conn
            .prepare_cached("UPDATE collections SET name = ?2 WHERE id = ?1")?
            .execute(params![id.get(), name]);
        match result {
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => Err(CoreError::AlreadyExists(format!(
                "Ya existe una colección llamada «{name}»."
            ))),
            Err(e) => Err(e.into()),
        }
    }

    /// Borra una colección (los libros siguen en la biblioteca).
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn delete_collection(&self, id: CollectionId) -> CoreResult<()> {
        self.conn
            .prepare_cached("DELETE FROM collections WHERE id = ?1")?
            .execute([id.get()])?;
        Ok(())
    }

    /// Añade libros a una colección; los que ya estaban se ignoran.
    ///
    /// # Errors
    /// - [`CoreError::InvalidData`] si la colección o algún libro no existe.
    /// - [`CoreError::Db`] si SQLite falla.
    pub fn add_to_collection(
        &self,
        id: CollectionId,
        books: &[BookId],
        at: Timestamp,
    ) -> CoreResult<()> {
        let tx = self.transaction()?;
        {
            let mut stmt = self.conn.prepare_cached(
                "INSERT INTO collection_books (collection_id, book_id, added_at)
                 VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
            )?;
            for book in books {
                match stmt.execute(params![id.get(), book.get(), at.as_unix_seconds()]) {
                    Ok(_) => {}
                    Err(rusqlite::Error::SqliteFailure(e, _))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        return Err(CoreError::InvalidData(
                            "la colección o el libro no existen".to_owned(),
                        ));
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Quita libros de una colección.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn remove_from_collection(&self, id: CollectionId, books: &[BookId]) -> CoreResult<()> {
        let tx = self.transaction()?;
        {
            let mut stmt = self.conn.prepare_cached(
                "DELETE FROM collection_books WHERE collection_id = ?1 AND book_id = ?2",
            )?;
            for book in books {
                stmt.execute(params![id.get(), book.get()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Colecciones a las que pertenece un libro.
    ///
    /// # Errors
    /// [`CoreError::Db`] si SQLite falla.
    pub fn collections_of(&self, book: BookId) -> CoreResult<Vec<CollectionId>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT collection_id FROM collection_books WHERE book_id = ?1")?;
        let ids = stmt
            .query_map([book.get()], |row| Ok(CollectionId::new(row.get(0)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use super::*;
    use crate::domain::{LibraryQuery, NewBook};
    use crate::library::InsertOutcome;

    fn at() -> Timestamp {
        Timestamp::from_unix_seconds(1)
    }

    fn library_with(paths: &[&str]) -> (Library, Vec<BookId>) {
        let lib = Library::open_in_memory().expect("biblioteca");
        let ids = paths
            .iter()
            .map(
                |p| match lib.insert(&NewBook::from_path(*p, 1).expect("libro"), at()) {
                    Ok(InsertOutcome::Inserted(id)) => id,
                    other => panic!("{other:?}"),
                },
            )
            .collect();
        (lib, ids)
    }

    #[test]
    fn collection_counts_its_books() {
        let (lib, ids) = library_with(&["C:/a.cbz", "C:/b.cbz"]);
        let shelf = lib.create_collection("  Favoritos  ", at()).expect("crear");
        lib.add_to_collection(shelf.id, &ids, at()).expect("añadir");
        lib.add_to_collection(shelf.id, &ids[..1], at())
            .expect("repetir");
        assert_eq!(
            lib.collections().expect("listar"),
            [Collection {
                id: shelf.id,
                name: "Favoritos".to_owned(),
                count: 2
            }]
        );
    }

    #[test]
    fn duplicate_name_is_rejected_ignoring_case() {
        let (lib, _) = library_with(&[]);
        lib.create_collection("Manga", at()).expect("crear");
        assert!(matches!(
            lib.create_collection("MANGA", at()),
            Err(CoreError::AlreadyExists(_))
        ));
    }

    #[test]
    fn empty_name_is_rejected() {
        let (lib, _) = library_with(&[]);
        assert!(matches!(
            lib.create_collection("   ", at()),
            Err(CoreError::InvalidData(_))
        ));
    }

    #[test]
    fn list_filters_by_collection() {
        let (lib, ids) = library_with(&["C:/a.cbz", "C:/b.cbz"]);
        let shelf = lib.create_collection("Uno", at()).expect("crear");
        lib.add_to_collection(shelf.id, &ids[1..], at())
            .expect("añadir");
        let query = LibraryQuery {
            collection: Some(shelf.id),
            ..LibraryQuery::default()
        };
        let titles: Vec<String> = lib
            .list(&query)
            .expect("listar")
            .into_iter()
            .map(|b| b.title)
            .collect();
        assert_eq!(titles, ["b"]);
    }

    #[test]
    fn deleting_collection_keeps_books() {
        let (lib, ids) = library_with(&["C:/a.cbz"]);
        let shelf = lib.create_collection("Temporal", at()).expect("crear");
        lib.add_to_collection(shelf.id, &ids, at()).expect("añadir");
        lib.delete_collection(shelf.id).expect("borrar");
        assert_eq!(
            (
                lib.count().expect("contar"),
                lib.collections_of(ids[0]).expect("de")
            ),
            (1, vec![])
        );
    }

    #[test]
    fn removing_book_from_collection() {
        let (lib, ids) = library_with(&["C:/a.cbz"]);
        let shelf = lib.create_collection("X", at()).expect("crear");
        lib.add_to_collection(shelf.id, &ids, at()).expect("añadir");
        lib.remove_from_collection(shelf.id, &ids).expect("quitar");
        assert!(lib.collections_of(ids[0]).expect("de").is_empty());
    }

    #[test]
    fn rename_to_existing_name_is_rejected() {
        let (lib, _) = library_with(&[]);
        lib.create_collection("A", at()).expect("crear");
        let b = lib.create_collection("B", at()).expect("crear");
        assert!(matches!(
            lib.rename_collection(b.id, "a"),
            Err(CoreError::AlreadyExists(_))
        ));
    }

    #[test]
    fn adding_to_missing_collection_fails() {
        let (lib, ids) = library_with(&["C:/a.cbz"]);
        assert!(matches!(
            lib.add_to_collection(CollectionId::new(7), &ids, at()),
            Err(CoreError::InvalidData(_))
        ));
    }
}
