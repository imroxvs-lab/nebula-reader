//! Lectura de cómics empaquetados (CBZ).
//!
//! Las páginas se leen bajo demanda desde el archivo comprimido, sin extraerlo
//! a disco. Los backends RAR y 7z se añadirán en la fase 5.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::error::{CoreError, CoreResult};
use crate::natural::natural_cmp;

/// Límites de seguridad al abrir archivos no confiables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    /// Número máximo de entradas en el archivo.
    pub max_entries: usize,
    /// Tamaño máximo descomprimido de una página, en bytes.
    pub max_page_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_page_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Tipo de imagen de una página.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageType {
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// GIF.
    Gif,
    /// WebP.
    Webp,
    /// AVIF.
    Avif,
    /// BMP.
    Bmp,
}

impl ImageType {
    /// Tipo según la extensión del nombre (sin distinguir mayúsculas).
    pub fn from_name(name: &str) -> Option<Self> {
        let (_, extension) = name.rsplit_once('.')?;
        Some(match extension.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" | "jfif" => Self::Jpeg,
            "png" => Self::Png,
            "gif" => Self::Gif,
            "webp" => Self::Webp,
            "avif" => Self::Avif,
            "bmp" => Self::Bmp,
            _ => return None,
        })
    }

    /// Tipo MIME para la cabecera `Content-Type`.
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Avif => "image/avif",
            Self::Bmp => "image/bmp",
        }
    }
}

/// Contenido de una página.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageData {
    /// Bytes de la imagen tal como están en el archivo.
    pub bytes: Vec<u8>,
    /// Tipo de imagen.
    pub image_type: ImageType,
}

#[derive(Debug, Clone)]
struct PageEntry {
    /// Índice de la entrada dentro del ZIP.
    entry: usize,
    /// Ruta dentro del archivo, para ordenar y diagnosticar.
    name: String,
    image_type: ImageType,
}

/// Cómic abierto, con su índice de páginas ordenado.
#[derive(Debug)]
pub struct ComicArchive {
    path: PathBuf,
    zip: ZipArchive<BufReader<File>>,
    pages: Vec<PageEntry>,
    limits: ArchiveLimits,
}

impl ComicArchive {
    /// Abre un CBZ con los límites por defecto.
    ///
    /// # Errors
    /// Ver [`ComicArchive::open_with_limits`].
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        Self::open_with_limits(path, ArchiveLimits::default())
    }

    /// Abre un CBZ e indexa sus páginas en orden natural.
    ///
    /// Se ignoran directorios, entradas cifradas, metadatos de macOS
    /// (`__MACOSX/`, `._*`), archivos ocultos, rutas que salen del archivo
    /// (`..`) y cualquier archivo que no sea una imagen. Las rutas «absolutas»
    /// (`/x.jpg`) se tratan como relativas: es inofensivo porque nunca se
    /// extrae nada a disco.
    ///
    /// # Errors
    /// - [`CoreError::Io`] si no se puede abrir el archivo.
    /// - [`CoreError::Archive`] si no es un ZIP válido o supera los límites.
    /// - [`CoreError::NoPages`] si no contiene ninguna imagen.
    pub fn open_with_limits(path: impl AsRef<Path>, limits: ArchiveLimits) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|e| CoreError::io(&path, e))?;
        let mut zip =
            ZipArchive::new(BufReader::new(file)).map_err(|e| CoreError::archive(&path, &e))?;

        if zip.len() > limits.max_entries {
            return Err(CoreError::Archive {
                path,
                reason: format!(
                    "tiene {} entradas; el máximo es {}",
                    zip.len(),
                    limits.max_entries
                ),
            });
        }

        let mut pages = Vec::new();
        for entry in 0..zip.len() {
            let file = zip
                .by_index_raw(entry)
                .map_err(|e| CoreError::archive(&path, &e))?;
            if file.is_dir() || file.encrypted() || file.enclosed_name().is_none() {
                continue;
            }
            let Ok(name) = file.name() else { continue };
            if is_ignored(&name) {
                continue;
            }
            if let Some(image_type) = ImageType::from_name(&name) {
                pages.push(PageEntry {
                    entry,
                    name: name.into_owned(),
                    image_type,
                });
            }
        }

        if pages.is_empty() {
            return Err(CoreError::NoPages(path));
        }
        pages.sort_by(|a, b| natural_cmp(&a.name, &b.name));

        Ok(Self {
            path,
            zip,
            pages,
            limits,
        })
    }

    /// Ruta del archivo.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Número de páginas.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Nombre (ruta interna) de una página.
    pub fn page_name(&self, index: usize) -> Option<&str> {
        self.pages.get(index).map(|page| page.name.as_str())
    }

    /// Lee los bytes de una página.
    ///
    /// La lectura se corta en [`ArchiveLimits::max_page_bytes`] aunque la
    /// cabecera del ZIP declare un tamaño menor (protección contra zip bombs).
    ///
    /// # Errors
    /// - [`CoreError::PageOutOfRange`] si el índice no existe.
    /// - [`CoreError::PageTooLarge`] si la página supera el límite.
    /// - [`CoreError::Archive`] si la entrada está dañada.
    pub fn read_page(&mut self, index: usize) -> CoreResult<PageData> {
        let count = self.pages.len();
        let page = self
            .pages
            .get(index)
            .ok_or(CoreError::PageOutOfRange { index, count })?;
        let (entry, image_type) = (page.entry, page.image_type);
        let limit = self.limits.max_page_bytes;

        let file = self
            .zip
            .by_index(entry)
            .map_err(|e| CoreError::archive(&self.path, &e))?;
        if file.size() > limit {
            return Err(CoreError::PageTooLarge { index, limit });
        }
        let capacity = usize::try_from(file.size()).unwrap_or_default();
        let mut bytes = Vec::with_capacity(capacity);
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| CoreError::Archive {
                path: self.path.clone(),
                reason: format!("página {index} dañada: {e}"),
            })?;
        if bytes.len() as u64 > limit {
            return Err(CoreError::PageTooLarge { index, limit });
        }
        Ok(PageData { bytes, image_type })
    }
}

/// Lee una entrada por nombre, con un límite de tamaño descomprimido.
///
/// La lectura se corta en `limit` aunque la cabecera declare un tamaño menor
/// (protección contra zip bombs).
///
/// # Errors
/// - [`CoreError::NotFoundInArchive`] si la entrada no existe o está cifrada.
/// - [`CoreError::PageTooLarge`] si supera el límite.
/// - [`CoreError::Archive`] si está dañada.
pub(crate) fn read_entry<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    archive: &Path,
    name: &str,
    limit: u64,
) -> CoreResult<Vec<u8>> {
    let file = match zip.by_name(name) {
        Ok(file) if !file.encrypted() => file,
        Ok(_) | Err(zip::result::ZipError::FileNotFound) => {
            return Err(CoreError::NotFoundInArchive(name.to_owned()));
        }
        Err(e) => return Err(CoreError::archive(archive, &e)),
    };
    if file.size() > limit {
        return Err(CoreError::PageTooLarge { index: 0, limit });
    }
    let mut bytes = Vec::with_capacity(usize::try_from(file.size()).unwrap_or_default());
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| CoreError::Archive {
            path: archive.to_path_buf(),
            reason: format!("{name} dañado: {e}"),
        })?;
    if bytes.len() as u64 > limit {
        return Err(CoreError::PageTooLarge { index: 0, limit });
    }
    Ok(bytes)
}

/// Entradas que no son páginas aunque tengan extensión de imagen.
fn is_ignored(name: &str) -> bool {
    // `.` como segmento (`./01.jpg`) no es un archivo oculto.
    name.split('/')
        .any(|part| part == "__MACOSX" || (part.starts_with('.') && part != "."))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // Infraestructura de test.

    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    /// Crea un CBZ con las entradas indicadas (nombre, contenido).
    fn make_zip(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("test.cbz");
        let file = File::create(&path).expect("crear zip");
        let mut zip = zip::ZipWriter::new(file);
        for (name, data) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, SimpleFileOptions::default())
                    .expect("añadir carpeta");
            } else {
                zip.start_file(*name, SimpleFileOptions::default())
                    .expect("añadir entrada");
                zip.write_all(data).expect("escribir entrada");
            }
        }
        zip.finish().expect("cerrar zip");
        path
    }

    fn names(archive: &ComicArchive) -> Vec<&str> {
        (0..archive.page_count())
            .filter_map(|i| archive.page_name(i))
            .collect()
    }

    #[test]
    fn pages_are_sorted_naturally() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(
            dir.path(),
            &[("p10.jpg", b"10"), ("p2.jpg", b"2"), ("p1.jpg", b"1")],
        );
        let archive = ComicArchive::open(&path).expect("abrir");
        assert_eq!(names(&archive), ["p1.jpg", "p2.jpg", "p10.jpg"]);
    }

    #[test]
    fn non_images_and_metadata_are_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(
            dir.path(),
            &[
                ("ComicInfo.xml", b"<x/>"),
                ("caps/", b""),
                ("caps/01.png", b"a"),
                ("__MACOSX/caps/._01.png", b"b"),
                (".thumb.jpg", b"c"),
                ("notas.txt", b"d"),
            ],
        );
        let archive = ComicArchive::open(&path).expect("abrir");
        assert_eq!(names(&archive), ["caps/01.png"]);
    }

    #[test]
    fn unsafe_entry_names_are_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(
            dir.path(),
            &[
                ("../fuera.jpg", b"x"),
                ("a/../../fuera2.jpg", b"y"),
                ("ok.jpg", b"z"),
            ],
        );
        let archive = ComicArchive::open(&path).expect("abrir");
        assert_eq!(names(&archive), ["ok.jpg"]);
    }

    #[test]
    fn read_page_returns_bytes_and_type() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(dir.path(), &[("a.webp", b"WEBP-DATA")]);
        let mut archive = ComicArchive::open(&path).expect("abrir");
        assert_eq!(
            archive.read_page(0).ok(),
            Some(PageData {
                bytes: b"WEBP-DATA".to_vec(),
                image_type: ImageType::Webp
            })
        );
    }

    #[test]
    fn read_page_out_of_range_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(dir.path(), &[("a.jpg", b"x")]);
        let mut archive = ComicArchive::open(&path).expect("abrir");
        assert!(matches!(
            archive.read_page(1),
            Err(CoreError::PageOutOfRange { index: 1, count: 1 })
        ));
    }

    #[test]
    fn archive_without_images_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(dir.path(), &[("leeme.txt", b"hola")]);
        assert!(matches!(
            ComicArchive::open(&path),
            Err(CoreError::NoPages(_))
        ));
    }

    #[test]
    fn corrupt_archive_fails_with_archive_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("roto.cbz");
        std::fs::write(&path, b"esto no es un zip").expect("escribir");
        assert!(matches!(
            ComicArchive::open(&path),
            Err(CoreError::Archive { .. })
        ));
    }

    #[test]
    fn missing_archive_fails_with_io_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            ComicArchive::open(dir.path().join("no.cbz")),
            Err(CoreError::Io { .. })
        ));
    }

    #[test]
    fn too_many_entries_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(dir.path(), &[("a.jpg", b"1"), ("b.jpg", b"2")]);
        let limits = ArchiveLimits {
            max_entries: 1,
            ..ArchiveLimits::default()
        };
        assert!(matches!(
            ComicArchive::open_with_limits(&path, limits),
            Err(CoreError::Archive { .. })
        ));
    }

    #[test]
    fn oversized_page_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = make_zip(dir.path(), &[("a.jpg", &[0u8; 100])]);
        let limits = ArchiveLimits {
            max_page_bytes: 10,
            ..ArchiveLimits::default()
        };
        let mut archive = ComicArchive::open_with_limits(&path, limits).expect("abrir");
        assert!(matches!(
            archive.read_page(0),
            Err(CoreError::PageTooLarge {
                index: 0,
                limit: 10
            })
        ));
    }

    #[test]
    fn dot_slash_prefix_is_not_hidden() {
        assert!(!is_ignored("./01.jpg"));
    }

    #[test]
    fn image_type_is_detected_case_insensitively() {
        assert_eq!(ImageType::from_name("A/B.JPEG"), Some(ImageType::Jpeg));
    }

    #[test]
    fn unknown_extension_is_not_an_image() {
        assert_eq!(ImageType::from_name("a.tiff"), None);
    }
}
