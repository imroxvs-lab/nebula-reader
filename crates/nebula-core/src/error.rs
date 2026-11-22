//! Errores del núcleo.

use std::path::PathBuf;

use crate::domain::BookId;

/// Error de cualquier operación de `nebula-core`.
///
/// Las variantes de base de datos y de formatos se añadirán en las tareas que
/// introducen esas capacidades.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoreError {
    /// Fallo de entrada/salida sobre una ruta concreta.
    #[error("error de E/S en {path}: {source}")]
    Io {
        /// Ruta implicada.
        path: PathBuf,
        /// Error original.
        #[source]
        source: std::io::Error,
    },

    /// El archivo no es de ningún formato soportado.
    #[error("formato no soportado: {path}")]
    UnsupportedFormat {
        /// Ruta del archivo rechazado.
        path: PathBuf,
    },

    /// Un valor almacenado no corresponde a ningún formato conocido.
    #[error("formato desconocido: {0:?}")]
    UnknownFormat(String),

    /// No existe ningún libro con ese identificador.
    #[error("no existe el libro {0}")]
    NotFound(BookId),
}

impl CoreError {
    /// Construye un [`CoreError::Io`] asociando el error a su ruta.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// Atajo de `Result` con [`CoreError`].
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_message_includes_path() {
        let err = CoreError::io(
            "C:/libros/a.cbz",
            std::io::Error::new(std::io::ErrorKind::NotFound, "no existe"),
        );
        assert!(err.to_string().contains("C:/libros/a.cbz"));
    }

    #[test]
    fn not_found_message_includes_id() {
        assert_eq!(
            CoreError::NotFound(BookId::new(42)).to_string(),
            "no existe el libro 42"
        );
    }
}
