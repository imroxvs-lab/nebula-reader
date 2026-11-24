//! Tipos del dominio: libros, formatos y consultas de la biblioteca.
//!
//! Todos los tipos que cruzan a la interfaz se serializan en `camelCase`
//! para que coincidan con las convenciones de TypeScript.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::metadata::BookMetadata;

/// Título usado cuando no se puede derivar ninguno del nombre del archivo.
pub const UNTITLED: &str = "Sin título";

/// Identificador de un libro en la biblioteca (clave primaria SQLite).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BookId(i64);

impl BookId {
    /// Envuelve un identificador de la base de datos.
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    /// Valor numérico subyacente.
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Instante en segundos desde la época Unix (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Crea un instante a partir de segundos Unix.
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// Instante actual. Si el reloj del sistema es anterior a 1970, devuelve 0.
    pub fn now() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        Self(seconds)
    }

    /// Segundos desde la época Unix.
    pub const fn as_unix_seconds(self) -> i64 {
        self.0
    }
}

/// Categoría de una obra, usada para filtrar la biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BookKind {
    /// Libro de texto reflowable o paginado (EPUB, PDF).
    Book,
    /// Cómic o manga basado en imágenes (CBZ, CBR, CB7).
    Comic,
}

/// Formato de archivo soportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BookFormat {
    /// Cómic en ZIP.
    Cbz,
    /// Cómic en RAR.
    Cbr,
    /// Cómic en 7z.
    Cb7,
    /// Libro EPUB.
    Epub,
    /// Documento PDF.
    Pdf,
}

impl BookFormat {
    /// Todos los formatos soportados.
    pub const ALL: [Self; 5] = [Self::Cbz, Self::Cbr, Self::Cb7, Self::Epub, Self::Pdf];

    /// Extensión canónica, en minúsculas y sin punto. También es la
    /// representación persistida en la base de datos.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Cb7 => "cb7",
            Self::Epub => "epub",
            Self::Pdf => "pdf",
        }
    }

    /// Categoría del formato.
    pub const fn kind(self) -> BookKind {
        match self {
            Self::Cbz | Self::Cbr | Self::Cb7 => BookKind::Comic,
            Self::Epub | Self::Pdf => BookKind::Book,
        }
    }

    /// Formato correspondiente a una extensión (sin punto, sin distinguir
    /// mayúsculas). `None` si no está soportada.
    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str().eq_ignore_ascii_case(extension))
    }

    /// Formato de un archivo según su extensión.
    ///
    /// La detección por contenido (magic bytes) llegará con los lectores de
    /// cada formato; para importar basta con la extensión.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }
}

impl fmt::Display for BookFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BookFormat {
    type Err = CoreError;

    /// Interpreta la representación persistida (ver [`BookFormat::as_str`]).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_extension(s).ok_or_else(|| CoreError::UnknownFormat(s.to_owned()))
    }
}

/// Libro almacenado en la biblioteca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    /// Identificador en la biblioteca.
    pub id: BookId,
    /// Ruta absoluta del archivo.
    pub path: PathBuf,
    /// Título mostrado.
    pub title: String,
    /// Formato del archivo.
    pub format: BookFormat,
    /// Tamaño del archivo en bytes.
    pub file_size: u64,
    /// Momento en que se añadió a la biblioteca.
    pub added_at: Timestamp,
    /// Última vez que se abrió, si alguna.
    pub last_opened_at: Option<Timestamp>,
    /// Número de páginas, si ya se conoce (se calcula al abrirlo o al generar la portada).
    pub page_count: Option<u32>,
    /// Estado de la miniatura de portada.
    pub cover: CoverStatus,
    /// Fracción leída (0.0–1.0), si se ha empezado a leer.
    pub progress: Option<f64>,
    /// Autor o autores.
    pub author: Option<String>,
    /// Serie a la que pertenece.
    pub series: Option<String>,
    /// Número dentro de la serie.
    pub series_index: Option<f64>,
}

/// Estado de la miniatura de portada de un libro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverStatus {
    /// Aún no se ha intentado generar.
    #[default]
    Pending,
    /// La miniatura está en la caché.
    Ready,
    /// No se pudo generar (archivo dañado o formato no compatible); no se reintenta.
    Failed,
}

impl CoverStatus {
    /// Representación persistida.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    /// Interpreta la representación persistida; valores desconocidos cuentan como pendientes.
    pub fn from_db(value: &str) -> Self {
        match value {
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

/// Posición de lectura dentro de un libro.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Locator {
    /// Página de un cómic o documento paginado (índice desde 0).
    Page {
        /// Índice de la página.
        index: u32,
    },
    /// Posición en un EPUB: documento del orden de lectura y fracción leída de él.
    /// Es independiente del tamaño de letra y de ventana.
    Epub {
        /// Índice en el orden de lectura.
        spine: u32,
        /// Fracción del documento (0.0–1.0).
        position: f64,
    },
}

impl Locator {
    /// Unidad de lectura (página o documento) a la que apunta.
    pub const fn unit(&self) -> u32 {
        match *self {
            Self::Page { index } => index,
            Self::Epub { spine, .. } => spine,
        }
    }
}

/// Progreso de lectura guardado.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgress {
    /// Dónde se quedó el lector.
    pub locator: Locator,
    /// Fracción leída (0.0–1.0).
    pub percent: f64,
    /// Cuándo se guardó.
    pub updated_at: Timestamp,
}

impl Book {
    /// Categoría del libro, derivada de su formato.
    pub const fn kind(&self) -> BookKind {
        self.format.kind()
    }
}

/// Datos de un libro aún no insertado en la biblioteca.
#[derive(Debug, Clone, PartialEq)]
pub struct NewBook {
    /// Ruta absoluta del archivo.
    pub path: PathBuf,
    /// Título derivado del nombre del archivo.
    pub title: String,
    /// Formato detectado.
    pub format: BookFormat,
    /// Tamaño en bytes.
    pub file_size: u64,
    /// Metadatos leídos del archivo (vacíos si no se leyeron).
    pub metadata: BookMetadata,
}

impl NewBook {
    /// Construye un libro nuevo detectando el formato y el título a partir de la ruta.
    ///
    /// # Errors
    /// [`CoreError::UnsupportedFormat`] si la extensión no corresponde a un formato soportado.
    pub fn from_path(path: impl Into<PathBuf>, file_size: u64) -> CoreResult<Self> {
        let path = path.into();
        let Some(format) = BookFormat::from_path(&path) else {
            return Err(CoreError::UnsupportedFormat { path });
        };
        let title = title_from_path(&path);
        Ok(Self {
            path,
            title,
            format,
            file_size,
            metadata: BookMetadata::default(),
        })
    }

    /// Completa el libro con los metadatos del archivo; el título declarado
    /// sustituye al derivado del nombre.
    #[must_use]
    pub fn with_metadata(mut self, metadata: BookMetadata) -> Self {
        if let Some(title) = &metadata.title {
            self.title.clone_from(title);
        }
        self.metadata = metadata;
        self
    }
}

/// Deriva un título legible del nombre de un archivo.
///
/// - Usa el nombre sin extensión.
/// - Sustituye `_` por espacios.
/// - Si no queda ningún espacio, trata los `.` como separadores
///   (`The.Hobbit.1937` → `The Hobbit 1937`); si hay espacios, los conserva
///   (`Vol. 1`).
/// - Colapsa espacios repetidos y recorta los extremos.
/// - Nunca devuelve una cadena vacía: recurre a [`UNTITLED`].
///
/// ```
/// use std::path::Path;
/// use nebula_core::title_from_path;
///
/// assert_eq!(title_from_path(Path::new("Saga_Vol_01.cbz")), "Saga Vol 01");
/// assert_eq!(title_from_path(Path::new("The.Hobbit.epub")), "The Hobbit");
/// ```
pub fn title_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let underscores_replaced = stem.replace('_', " ");
    let separators_replaced = if underscores_replaced.contains(char::is_whitespace) {
        underscores_replaced
    } else {
        underscores_replaced.replace('.', " ")
    };

    let title = separators_replaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if title.is_empty() {
        UNTITLED.to_owned()
    } else {
        title
    }
}

/// Criterio de ordenación de la biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    /// Alfabético por título.
    Title,
    /// Añadidos más recientemente primero.
    #[default]
    AddedAt,
    /// Abiertos más recientemente primero; los nunca abiertos al final.
    LastOpened,
}

/// Filtros y orden para listar la biblioteca.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryQuery {
    /// Texto a buscar en el título (sin distinguir mayúsculas).
    pub search: Option<String>,
    /// Restringe a libros o a cómics.
    pub kind: Option<BookKind>,
    /// Orden del resultado.
    pub sort: SortBy,
    /// Número máximo de resultados; `None` = todos.
    pub limit: Option<u32>,
}

/// Recuento de la biblioteca por categoría.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    /// Libros (EPUB, PDF).
    pub books: u64,
    /// Cómics (CBZ, CBR, CB7).
    pub comics: u64,
}

impl LibraryQuery {
    /// Término de búsqueda recortado; `None` si está ausente o vacío.
    pub fn search_term(&self) -> Option<&str> {
        self.search
            .as_deref()
            .map(str::trim)
            .filter(|term| !term.is_empty())
    }
}

/// Archivo que no se pudo importar y el motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    /// Ruta afectada.
    pub path: PathBuf,
    /// Descripción legible del problema.
    pub reason: String,
}

/// Resumen de una importación a la biblioteca.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// Libros añadidos.
    pub added: usize,
    /// Archivos ya presentes en la biblioteca.
    pub skipped_duplicates: usize,
    /// Archivos ignorados por no tener un formato soportado.
    pub unsupported: usize,
    /// Archivos que fallaron por otros motivos.
    pub failures: Vec<ImportFailure>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn format_detected_from_lowercase_extension() {
        assert_eq!(
            BookFormat::from_path(Path::new("a/b/batman.cbz")),
            Some(BookFormat::Cbz)
        );
    }

    #[test]
    fn format_detection_ignores_case() {
        assert_eq!(
            BookFormat::from_path(Path::new("LIBRO.EpUb")),
            Some(BookFormat::Epub)
        );
    }

    #[test]
    fn unsupported_extension_has_no_format() {
        assert_eq!(BookFormat::from_path(Path::new("notas.txt")), None);
    }

    #[test]
    fn file_without_extension_has_no_format() {
        assert_eq!(BookFormat::from_path(Path::new("cbz")), None);
    }

    #[test]
    fn format_roundtrips_through_str() {
        for format in BookFormat::ALL {
            assert_eq!(format.as_str().parse::<BookFormat>().ok(), Some(format));
        }
    }

    #[test]
    fn parsing_unknown_format_fails() {
        assert!(matches!(
            "mobi".parse::<BookFormat>(),
            Err(CoreError::UnknownFormat(s)) if s == "mobi"
        ));
    }

    #[test]
    fn comic_formats_are_comics() {
        for format in [BookFormat::Cbz, BookFormat::Cbr, BookFormat::Cb7] {
            assert_eq!(format.kind(), BookKind::Comic);
        }
    }

    #[test]
    fn text_formats_are_books() {
        for format in [BookFormat::Epub, BookFormat::Pdf] {
            assert_eq!(format.kind(), BookKind::Book);
        }
    }

    #[test]
    fn title_replaces_underscores() {
        assert_eq!(title_from_path(Path::new("Saga_Vol_01.cbz")), "Saga Vol 01");
    }

    #[test]
    fn title_treats_dots_as_separators_without_spaces() {
        assert_eq!(
            title_from_path(Path::new("The.Hobbit.1937.epub")),
            "The Hobbit 1937"
        );
    }

    #[test]
    fn title_keeps_dots_when_spaces_exist() {
        assert_eq!(
            title_from_path(Path::new("Dr. Slump Vol. 1.cbr")),
            "Dr. Slump Vol. 1"
        );
    }

    #[test]
    fn title_collapses_whitespace() {
        assert_eq!(
            title_from_path(Path::new("  One   Piece _ 001 .cbz")),
            "One Piece 001"
        );
    }

    #[test]
    fn title_falls_back_when_empty() {
        assert_eq!(title_from_path(Path::new("___.pdf")), UNTITLED);
    }

    #[test]
    fn new_book_detects_format_and_title() {
        let book = NewBook::from_path("C:/comics/Saga_01.cbz", 1024).ok();
        assert_eq!(
            book.map(|b| (b.format, b.title)),
            Some((BookFormat::Cbz, "Saga 01".to_owned()))
        );
    }

    #[test]
    fn new_book_rejects_unsupported_format() {
        assert!(matches!(
            NewBook::from_path("C:/docs/a.docx", 1),
            Err(CoreError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn blank_search_term_is_ignored() {
        let query = LibraryQuery {
            search: Some("   ".to_owned()),
            ..LibraryQuery::default()
        };
        assert_eq!(query.search_term(), None);
    }

    #[test]
    fn search_term_is_trimmed() {
        let query = LibraryQuery {
            search: Some("  saga ".to_owned()),
            ..LibraryQuery::default()
        };
        assert_eq!(query.search_term(), Some("saga"));
    }

    #[test]
    fn library_query_deserializes_from_partial_camel_case_json() {
        let query: LibraryQuery = from_json(r#"{"kind":"comic","sort":"lastOpened"}"#);
        assert_eq!(
            query,
            LibraryQuery {
                search: None,
                kind: Some(BookKind::Comic),
                sort: SortBy::LastOpened,
                limit: None,
            }
        );
    }

    /// Deserializa con `serde_json` solo en tests.
    fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> T {
        #[allow(clippy::expect_used)] // Datos de test fijos y válidos.
        serde_json::from_str(json).expect("JSON de test válido")
    }

    #[test]
    fn timestamp_now_is_after_2020() {
        assert!(Timestamp::now().as_unix_seconds() > 1_577_836_800);
    }

    proptest! {
        #[test]
        fn title_is_never_empty(name in "\\PC{0,40}") {
            let path = PathBuf::from(format!("{name}.cbz"));
            prop_assert!(!title_from_path(&path).is_empty());
        }

        #[test]
        fn title_is_normalized(name in "[a-zA-Z0-9 _.]{0,40}") {
            let title = title_from_path(&PathBuf::from(format!("{name}.epub")));
            prop_assert!(!title.contains('_'));
            prop_assert!(!title.contains("  "));
            prop_assert_eq!(title.trim(), title.as_str());
        }

        #[test]
        fn format_detection_is_case_insensitive(index in 0usize..5, upper in proptest::bool::ANY) {
            let format = BookFormat::ALL[index];
            let ext = if upper { format.as_str().to_uppercase() } else { format.as_str().to_owned() };
            prop_assert_eq!(BookFormat::from_extension(&ext), Some(format));
        }
    }
}
