//! Errores del núcleo.

use std::path::PathBuf;

use crate::domain::BookId;

/// Error de cualquier operación de `nebula-core`.
///
/// Las variantes de los lectores de formatos se añadirán en las tareas que
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

    /// La ruta no es Unicode válido y no se puede almacenar en la biblioteca.
    #[error("la ruta no es Unicode válido: {0:?}")]
    NonUnicodePath(PathBuf),

    /// El archivo comprimido está dañado o supera los límites de seguridad.
    #[error("no se puede leer {path}: {reason}")]
    Archive {
        /// Ruta del archivo.
        path: PathBuf,
        /// Motivo legible.
        reason: String,
    },

    /// El archivo no contiene ninguna página de imagen.
    #[error("{0} no contiene ninguna imagen")]
    NoPages(PathBuf),

    /// El recurso pedido no está dentro del archivo.
    #[error("el recurso {0} no existe en el libro")]
    NotFoundInArchive(String),

    /// Se pidió una página que no existe.
    #[error("la página {index} no existe (el libro tiene {count})")]
    PageOutOfRange {
        /// Índice pedido (desde 0).
        index: usize,
        /// Número de páginas.
        count: usize,
    },

    /// La página descomprimida supera el tamaño máximo permitido.
    #[error("la página {index} supera el límite de {limit} bytes")]
    PageTooLarge {
        /// Índice de la página (desde 0).
        index: usize,
        /// Límite aplicado.
        limit: u64,
    },

    /// No se pudo decodificar o codificar una imagen.
    #[error("imagen no válida: {0}")]
    Image(String),

    /// Ya existe un elemento con ese nombre o ruta.
    #[error("{0}")]
    AlreadyExists(String),

    /// Un dato no se pudo serializar o interpretar.
    #[error("dato no válido: {0}")]
    InvalidData(String),

    /// El documento está protegido con contraseña.
    #[error("{0} está protegido con contraseña")]
    Protected(PathBuf),

    /// El motor PDF no está disponible o no se pudo cargar.
    #[error("motor PDF no disponible: {0}")]
    PdfEngine(String),

    /// Fallo de SQLite.
    #[error("error de base de datos: {0}")]
    Db(#[from] rusqlite::Error),

    /// La base de datos la creó una versión más nueva de la aplicación.
    #[error(
        "la biblioteca usa el esquema {found}, pero esta versión solo admite hasta el {supported}; actualiza Nebula Reader"
    )]
    SchemaTooNew {
        /// Versión encontrada en la base de datos.
        found: u32,
        /// Versión máxima conocida por esta compilación.
        supported: u32,
    },
}

impl CoreError {
    /// Construye un [`CoreError::Archive`] a partir de un error del lector ZIP.
    pub(crate) fn archive(path: &std::path::Path, error: &zip::result::ZipError) -> Self {
        Self::Archive {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    }

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
