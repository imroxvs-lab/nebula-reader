//! Documento abierto para lectura, sea del formato que sea.

use std::path::Path;

use crate::archive::ComicArchive;
use crate::domain::BookFormat;
use crate::epub::EpubBook;
use crate::error::{CoreError, CoreResult};

/// Documento abierto. Se usa un `enum` (no `dyn`) porque los formatos son pocos
/// y conocidos, y cada uno expone operaciones distintas.
#[derive(Debug)]
pub enum OpenDocument {
    /// Cómic paginado por imágenes.
    Comic(ComicArchive),
    /// Libro EPUB.
    Epub(EpubBook),
}

impl OpenDocument {
    /// Abre un archivo según su formato.
    ///
    /// # Errors
    /// - [`CoreError::UnsupportedFormat`] si el formato aún no tiene lector.
    /// - Los errores de apertura de cada formato.
    pub fn open(path: &Path, format: BookFormat) -> CoreResult<Self> {
        match format {
            BookFormat::Cbz | BookFormat::Cbr | BookFormat::Cb7 => {
                ComicArchive::open(path).map(Self::Comic)
            }
            BookFormat::Epub => EpubBook::open(path).map(Self::Epub),
            BookFormat::Pdf => Err(CoreError::UnsupportedFormat {
                path: path.to_path_buf(),
            }),
        }
    }

    /// Unidades de lectura: páginas de un cómic o documentos del orden de lectura.
    pub fn unit_count(&self) -> usize {
        match self {
            Self::Comic(comic) => comic.page_count(),
            Self::Epub(book) => book.spine().len(),
        }
    }
}
