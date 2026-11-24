//! Generación de miniaturas de portada.
//!
//! Las miniaturas se guardan como JPEG en una caché en disco
//! (`<datos>/thumbs/<id>.jpg`), de modo que la interfaz nunca decodifica la
//! imagen original a tamaño completo para mostrar la biblioteca.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{ImageReader, Limits};

use crate::archive::ComicArchive;
use crate::domain::BookId;
use crate::error::{CoreError, CoreResult};

/// Ancho de las miniaturas, en píxeles. Las tarjetas miden ~156 px de ancho, así
/// que 320 px se ven nítidas también con escalado de pantalla del 200 %.
pub const COVER_WIDTH: u32 = 320;
/// Alto máximo; protege frente a imágenes extremadamente verticales (webtoons).
pub const COVER_MAX_HEIGHT: u32 = COVER_WIDTH * 3;
/// Calidad JPEG de las miniaturas.
const JPEG_QUALITY: u8 = 82;
/// Dimensión máxima aceptada al decodificar una página.
const MAX_DIMENSION: u32 = 16_384;
/// Memoria máxima que puede reservar el decodificador.
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;

/// Ruta de la miniatura de un libro dentro de la carpeta de caché.
pub fn cover_path(thumbs_dir: &Path, id: BookId) -> PathBuf {
    thumbs_dir.join(format!("{id}.jpg"))
}

/// Reduce una imagen a miniatura JPEG de [`COVER_WIDTH`] píxeles de ancho como
/// máximo (no amplía imágenes pequeñas).
///
/// # Errors
/// [`CoreError::Image`] si el formato no se reconoce, la imagen está dañada o
/// supera los límites de tamaño.
pub fn render_cover(image_bytes: &[u8]) -> CoreResult<Vec<u8>> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);

    let mut reader = ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| CoreError::Image(e.to_string()))?;
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| CoreError::Image(e.to_string()))?;

    // `thumbnail` conserva la proporción y usa un filtro rápido adecuado para reducir.
    let thumbnail = if image.width() > COVER_WIDTH || image.height() > COVER_MAX_HEIGHT {
        image.thumbnail(COVER_WIDTH, COVER_MAX_HEIGHT)
    } else {
        image
    };
    // JPEG no admite transparencia.
    let rgb = thumbnail.to_rgb8();

    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|e| CoreError::Image(e.to_string()))?;
    Ok(output)
}

/// Genera la miniatura a partir de la primera página del cómic y la guarda en
/// `destination` de forma atómica (archivo temporal + renombrado).
///
/// # Errors
/// - Errores de lectura del archivo ([`ComicArchive::read_page`]).
/// - [`CoreError::Image`] si la página no se puede decodificar.
/// - [`CoreError::Io`] si no se puede escribir la miniatura.
pub fn write_comic_cover(archive: &mut ComicArchive, destination: &Path) -> CoreResult<()> {
    let page = archive.read_page(0)?;
    write_cover(&page.bytes, destination)
}

/// Genera la miniatura de una imagen cualquiera y la guarda de forma atómica.
///
/// # Errors
/// - [`CoreError::Image`] si la imagen no se puede decodificar.
/// - [`CoreError::Io`] si no se puede escribir la miniatura.
pub fn write_cover(image_bytes: &[u8], destination: &Path) -> CoreResult<()> {
    let jpeg = render_cover(image_bytes)?;
    write_atomically(destination, &jpeg)
}

fn write_atomically(destination: &Path, bytes: &[u8]) -> CoreResult<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
    }
    let temporary = destination.with_extension("jpg.tmp");
    std::fs::write(&temporary, bytes).map_err(|e| CoreError::io(&temporary, e))?;
    std::fs::rename(&temporary, destination).map_err(|e| CoreError::io(destination, e))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use image::{ImageFormat, RgbaImage};

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 90, 128]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("codificar png");
        bytes
    }

    fn dimensions(jpeg: &[u8]) -> (u32, u32) {
        ImageReader::new(Cursor::new(jpeg))
            .with_guessed_format()
            .expect("formato")
            .into_dimensions()
            .expect("dimensiones")
    }

    #[test]
    fn large_page_is_scaled_to_cover_width_keeping_ratio() {
        let jpeg = render_cover(&png(1600, 2400)).expect("miniatura");
        assert_eq!(dimensions(&jpeg), (320, 480));
    }

    #[test]
    fn small_page_is_not_enlarged() {
        let jpeg = render_cover(&png(100, 150)).expect("miniatura");
        assert_eq!(dimensions(&jpeg), (100, 150));
    }

    #[test]
    fn very_tall_page_is_bounded_by_max_height() {
        let jpeg = render_cover(&png(800, 12_000)).expect("miniatura");
        assert!(dimensions(&jpeg).1 <= COVER_MAX_HEIGHT);
    }

    #[test]
    fn output_is_jpeg() {
        let jpeg = render_cover(&png(10, 10)).expect("miniatura");
        assert_eq!(&jpeg[..3], &[0xFF, 0xD8, 0xFF]);
    }

    #[test]
    fn garbage_is_an_image_error() {
        assert!(matches!(
            render_cover(b"no soy una imagen"),
            Err(CoreError::Image(_))
        ));
    }

    #[test]
    fn oversized_dimensions_are_rejected() {
        assert!(matches!(
            render_cover(&png(MAX_DIMENSION + 1, 1)),
            Err(CoreError::Image(_))
        ));
    }

    #[test]
    fn cover_path_uses_book_id() {
        assert_eq!(
            cover_path(Path::new("thumbs"), BookId::new(7)),
            Path::new("thumbs").join("7.jpg")
        );
    }

    #[test]
    fn write_atomically_creates_parent_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let destination = dir.path().join("thumbs").join("1.jpg");
        write_atomically(&destination, b"x").expect("escribir");
        assert_eq!(
            (
                destination.exists(),
                destination.with_extension("jpg.tmp").exists()
            ),
            (true, false)
        );
    }
}
