//! Lectura de PDF con PDFium.
//!
//! PDFium se carga dinámicamente (`pdfium.dll`): si no está, la app sigue
//! funcionando y solo los PDF quedan deshabilitados.
//!
//! Los documentos no se mantienen abiertos: cada render vuelve a abrir el
//! archivo. PDFium lee el índice de objetos de forma perezosa, así que abrir es
//! barato, y así la memoria no crece con el número de PDF consultados.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use pdfium_render::prelude::{
    PdfDocument, PdfDocumentMetadataTagType, PdfRenderConfig, Pdfium, PdfiumError,
    PdfiumInternalError,
};

use crate::error::{CoreError, CoreResult};
use crate::metadata::BookMetadata;

/// Ancho máximo de render, en píxeles (protege frente a peticiones abusivas).
pub const MAX_RENDER_WIDTH: u32 = 4096;
/// Ancho mínimo de render.
pub const MIN_RENDER_WIDTH: u32 = 64;
/// Calidad JPEG de las páginas renderizadas: texto nítido con tamaño moderado.
const JPEG_QUALITY: u8 = 90;

/// Motor PDF cargado. Es `Send + Sync` (feature `thread_safe` de
/// `pdfium-render`, que serializa las llamadas a PDFium).
pub struct PdfEngine {
    pdfium: Pdfium,
}

impl std::fmt::Debug for PdfEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfEngine").finish_non_exhaustive()
    }
}

/// Página renderizada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    /// JPEG.
    pub jpeg: Vec<u8>,
    /// Ancho en píxeles.
    pub width: u32,
    /// Alto en píxeles.
    pub height: u32,
}

/// Información de un PDF.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfInfo {
    /// Número de páginas.
    pub page_count: u32,
    /// Proporción alto/ancho de la primera página (para maquetar antes de renderizar).
    pub aspect_ratio: f64,
}

impl PdfEngine {
    /// Carga PDFium desde una biblioteca concreta (`…/pdfium.dll`).
    ///
    /// # Errors
    /// [`CoreError::PdfEngine`] si no existe o no es una versión compatible.
    pub fn load(library: &Path) -> CoreResult<Self> {
        let bindings =
            Pdfium::bind_to_library(library).map_err(|e| CoreError::PdfEngine(e.to_string()))?;
        Ok(Self {
            pdfium: Pdfium::new(bindings),
        })
    }

    fn open<'a>(&'a self, path: &Path) -> CoreResult<PdfDocument<'a>> {
        // Se comprueba antes para dar un error de E/S claro (p. ej. disco desconectado).
        std::fs::metadata(path).map_err(|e| CoreError::io(path, e))?;
        self.pdfium
            .load_pdf_from_file(path, None)
            .map_err(|e| pdf_error(path, e))
    }

    /// Número de páginas y proporción de la primera.
    ///
    /// # Errors
    /// Ver [`pdf_error`]: archivo inexistente, dañado o protegido.
    pub fn info(&self, path: &Path) -> CoreResult<PdfInfo> {
        let document = self.open(path)?;
        let pages = document.pages();
        let page_count = u32::try_from(pages.len()).unwrap_or_default();
        if page_count == 0 {
            return Err(CoreError::NoPages(path.to_path_buf()));
        }
        let aspect_ratio = pages
            .get(0)
            .map(|page| f64::from(page.height().value) / f64::from(page.width().value.max(1.0)))
            .unwrap_or(1.414);
        Ok(PdfInfo {
            page_count,
            aspect_ratio,
        })
    }

    /// Metadatos (título y autor) del PDF.
    ///
    /// # Errors
    /// Errores de apertura del archivo.
    pub fn metadata(&self, path: &Path) -> CoreResult<BookMetadata> {
        let document = self.open(path)?;
        let tag = |kind| {
            document
                .metadata()
                .get(kind)
                .map(|t| crate::xml::collapse_whitespace(t.value()))
                .filter(|v| !v.is_empty() && !looks_generated(v))
        };
        Ok(BookMetadata {
            title: tag(PdfDocumentMetadataTagType::Title),
            author: tag(PdfDocumentMetadataTagType::Author),
            series: None,
            series_index: None,
        })
    }

    /// Renderiza una página a `width` píxeles de ancho (acotado) como JPEG.
    ///
    /// # Errors
    /// - [`CoreError::PageOutOfRange`] si la página no existe.
    /// - Errores de apertura o de render.
    pub fn render(&self, path: &Path, index: u32, width: u32) -> CoreResult<RenderedPage> {
        let document = self.open(path)?;
        let pages = document.pages();
        let count = usize::try_from(pages.len()).unwrap_or_default();
        let page_index = u16::try_from(index)
            .ok()
            .filter(|i| usize::from(*i) < count)
            .ok_or(CoreError::PageOutOfRange {
                index: index as usize,
                count,
            })?;
        let page = pages
            .get(i32::from(page_index))
            .map_err(|e| pdf_error(path, e))?;
        let width = width.clamp(MIN_RENDER_WIDTH, MAX_RENDER_WIDTH);
        let config = PdfRenderConfig::new()
            .set_target_width(i32::try_from(width).unwrap_or(i32::MAX))
            // Acota páginas extremadamente alargadas.
            .set_maximum_height(i32::try_from(MAX_RENDER_WIDTH * 4).unwrap_or(i32::MAX))
            .render_form_data(true);
        let image = page
            .render_with_config(&config)
            .and_then(|bitmap| bitmap.as_image())
            .map_err(|e| pdf_error(path, e))?
            .to_rgb8();
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(Cursor::new(&mut jpeg), JPEG_QUALITY)
            .encode_image(&image)
            .map_err(|e| CoreError::Image(e.to_string()))?;
        Ok(RenderedPage {
            width: image.width(),
            height: image.height(),
            jpeg,
        })
    }
}

/// Títulos que los programas ponen por defecto y no aportan nada.
fn looks_generated(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower == "untitled"
        || lower.starts_with("microsoft word - ")
        || lower.ends_with(".pdf")
        || lower.ends_with(".doc")
        || lower.ends_with(".docx")
}

fn pdf_error(path: &Path, error: PdfiumError) -> CoreError {
    match error {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
            CoreError::Protected(path.to_path_buf())
        }
        other => CoreError::Archive {
            path: path.to_path_buf(),
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_titles_are_ignored() {
        assert!(
            ["Untitled", "Microsoft Word - informe.docx", "scan001.pdf"]
                .iter()
                .all(|t| looks_generated(t))
        );
    }

    #[test]
    fn real_titles_are_kept() {
        assert!(!looks_generated("Manual de Rust"));
    }
}
