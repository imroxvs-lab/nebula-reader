//! Núcleo de Nebula Reader.
//!
//! Contiene el dominio, el acceso a formatos de libros y cómics y la biblioteca
//! persistente. No depende de Tauri, de modo que se puede probar de forma aislada.

#![forbid(unsafe_code)]

pub mod domain;
pub mod error;
pub mod library;

pub use domain::{
    Book, BookFormat, BookId, BookKind, ImportFailure, ImportReport, LibraryQuery, NewBook, SortBy,
    Timestamp, title_from_path,
};
pub use error::{CoreError, CoreResult};
pub use library::{InsertOutcome, Library};

/// Versión del núcleo, expuesta para diagnósticos en la interfaz.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
