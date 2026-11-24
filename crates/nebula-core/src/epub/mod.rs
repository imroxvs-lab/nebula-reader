//! Lectura de libros EPUB 2 y EPUB 3.
//!
//! Se lee `META-INF/container.xml` para localizar el paquete OPF; de él se
//! obtienen metadatos, manifiesto, orden de lectura (spine) y portada. El índice
//! sale del documento de navegación de EPUB 3 (`nav`) o, si no existe, del NCX de
//! EPUB 2. Los recursos se leen bajo demanda desde el ZIP, sin extraerlo.

pub mod path;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use serde::Serialize;
use zip::ZipArchive;

use crate::archive::{ArchiveLimits, read_entry};
use crate::error::{CoreError, CoreResult};
use crate::xml;

/// Tamaño máximo de los documentos XML de estructura (container, OPF, NCX, nav).
const MAX_XML_BYTES: u64 = 16 * 1024 * 1024;
/// Profundidad máxima del índice que se conserva.
const MAX_TOC_DEPTH: u32 = 8;

/// Metadatos de un EPUB.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpubMetadata {
    /// Título (`dc:title`).
    pub title: Option<String>,
    /// Autores (`dc:creator`), en el orden del libro.
    pub authors: Vec<String>,
    /// Idioma (`dc:language`).
    pub language: Option<String>,
    /// Serie (EPUB 3 `belongs-to-collection` o `calibre:series`).
    pub series: Option<String>,
    /// Número dentro de la serie.
    pub series_index: Option<f64>,
}

/// Documento del orden de lectura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineItem {
    /// Ruta dentro del ZIP.
    pub href: String,
    /// Tamaño descomprimido (sirve para estimar el progreso global).
    pub size: u64,
    /// `false` para contenido auxiliar (`linear="no"`).
    pub linear: bool,
}

/// Entrada del índice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TocEntry {
    /// Texto mostrado.
    pub title: String,
    /// Ruta dentro del ZIP (sin fragmento).
    pub href: String,
    /// Ancla dentro del documento, si la hay.
    pub fragment: Option<String>,
    /// Nivel de anidamiento (0 = raíz).
    pub level: u32,
}

/// Recurso leído del libro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// Contenido.
    pub bytes: Vec<u8>,
    /// Tipo MIME.
    pub media_type: String,
}

#[derive(Debug, Clone)]
struct ManifestItem {
    id: String,
    href: String,
    media_type: String,
    properties: String,
}

/// Libro EPUB abierto.
#[derive(Debug)]
pub struct EpubBook {
    path: PathBuf,
    zip: ZipArchive<BufReader<File>>,
    limits: ArchiveLimits,
    manifest: Vec<ManifestItem>,
    spine: Vec<SpineItem>,
    toc: Vec<TocEntry>,
    metadata: EpubMetadata,
    cover: Option<String>,
}

impl EpubBook {
    /// Abre un EPUB con los límites por defecto.
    ///
    /// # Errors
    /// Ver [`EpubBook::open_with_limits`].
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        Self::open_with_limits(path, ArchiveLimits::default())
    }

    /// Abre un EPUB y lee su estructura.
    ///
    /// # Errors
    /// - [`CoreError::Io`] si no se puede abrir el archivo.
    /// - [`CoreError::Archive`] si no es un ZIP, falta el paquete OPF o no hay
    ///   ningún documento legible en el orden de lectura.
    pub fn open_with_limits(path: impl AsRef<Path>, limits: ArchiveLimits) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|e| CoreError::io(&path, e))?;
        let mut zip =
            ZipArchive::new(BufReader::new(file)).map_err(|e| CoreError::archive(&path, &e))?;
        if zip.len() > limits.max_entries {
            return Err(invalid(&path, "tiene demasiadas entradas"));
        }

        let container = read_text(&mut zip, &path, "META-INF/container.xml")?;
        let opf_path = parse_container(&container)
            .ok_or_else(|| invalid(&path, "container.xml no indica el paquete OPF"))?;
        let opf = read_text(&mut zip, &path, &opf_path)?;
        let package = parse_opf(&opf, &opf_path);

        let mut spine = Vec::with_capacity(package.spine.len());
        for (idref, linear) in &package.spine {
            let Some(item) = package.manifest.iter().find(|item| &item.id == idref) else {
                continue;
            };
            if !is_document(&item.media_type) {
                continue;
            }
            let Ok(entry) = zip.by_name(&item.href) else {
                continue;
            };
            spine.push(SpineItem {
                href: item.href.clone(),
                size: entry.size(),
                linear: *linear,
            });
        }
        if spine.is_empty() {
            return Err(invalid(&path, "el orden de lectura está vacío"));
        }

        let mut book = Self {
            path,
            zip,
            limits,
            manifest: package.manifest,
            spine,
            toc: Vec::new(),
            metadata: package.metadata,
            cover: None,
        };
        book.cover = book.find_cover(package.cover_id.as_deref());
        book.toc = book.read_toc(package.ncx_id.as_deref());
        Ok(book)
    }

    /// Ruta del archivo.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Metadatos.
    pub fn metadata(&self) -> &EpubMetadata {
        &self.metadata
    }

    /// Orden de lectura.
    pub fn spine(&self) -> &[SpineItem] {
        &self.spine
    }

    /// Índice (puede estar vacío).
    pub fn toc(&self) -> &[TocEntry] {
        &self.toc
    }

    /// Ruta de la imagen de portada, si se encontró.
    pub fn cover_href(&self) -> Option<&str> {
        self.cover.as_deref()
    }

    /// Lee un recurso por su ruta dentro del ZIP (ya normalizada).
    ///
    /// # Errors
    /// - [`CoreError::NotFoundInArchive`] si no existe.
    /// - [`CoreError::PageTooLarge`] si supera el límite de tamaño.
    /// - [`CoreError::Archive`] si la entrada está dañada.
    pub fn read_resource(&mut self, href: &str) -> CoreResult<Resource> {
        let href =
            path::normalize(href).ok_or_else(|| CoreError::NotFoundInArchive(href.to_owned()))?;
        let media_type = self.media_type(&href);
        let bytes = read_entry(&mut self.zip, &self.path, &href, self.limits.max_page_bytes)?;
        Ok(Resource { bytes, media_type })
    }

    /// Lee la imagen de portada, si existe.
    ///
    /// # Errors
    /// Ver [`EpubBook::read_resource`].
    pub fn read_cover(&mut self) -> CoreResult<Option<Vec<u8>>> {
        match self.cover.clone() {
            Some(href) => Ok(Some(self.read_resource(&href)?.bytes)),
            None => Ok(None),
        }
    }

    /// Tipo MIME de un recurso: el del manifiesto o, si no figura, por extensión.
    pub fn media_type(&self, href: &str) -> String {
        self.manifest
            .iter()
            .find(|item| item.href == href)
            .map(|item| item.media_type.clone())
            .unwrap_or_else(|| media_type_from_extension(href).to_owned())
    }

    fn find_cover(&self, cover_id: Option<&str>) -> Option<String> {
        let is_image = |item: &&ManifestItem| item.media_type.starts_with("image/");
        // EPUB 3: propiedad `cover-image`.
        let epub3 = self.manifest.iter().filter(is_image).find(|item| {
            item.properties
                .split_whitespace()
                .any(|p| p == "cover-image")
        });
        // EPUB 2: `<meta name="cover" content="id">`.
        let epub2 = || {
            cover_id.and_then(|id| {
                self.manifest
                    .iter()
                    .filter(is_image)
                    .find(|item| item.id == id)
            })
        };
        // Heurística: una imagen cuyo id o nombre contenga «cover».
        let guess = || {
            self.manifest.iter().filter(is_image).find(|item| {
                item.id.to_ascii_lowercase().contains("cover")
                    || item.href.to_ascii_lowercase().contains("cover")
            })
        };
        epub3
            .or_else(epub2)
            .or_else(guess)
            .map(|item| item.href.clone())
    }

    fn read_toc(&mut self, ncx_id: Option<&str>) -> Vec<TocEntry> {
        let nav = self
            .manifest
            .iter()
            .find(|item| item.properties.split_whitespace().any(|p| p == "nav"))
            .map(|item| item.href.clone());
        if let Some(nav) = nav
            && let Ok(text) = read_text(&mut self.zip, &self.path, &nav)
        {
            let toc = parse_nav(&text, &nav);
            if !toc.is_empty() {
                return toc;
            }
        }

        let ncx = self
            .manifest
            .iter()
            .find(|item| {
                ncx_id.is_some_and(|id| item.id == id)
                    || item.media_type == "application/x-dtbncx+xml"
            })
            .map(|item| item.href.clone());
        match ncx {
            Some(ncx) => read_text(&mut self.zip, &self.path, &ncx)
                .map(|text| parse_ncx(&text, &ncx))
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

fn invalid(path: &Path, reason: &str) -> CoreError {
    CoreError::Archive {
        path: path.to_path_buf(),
        reason: reason.to_owned(),
    }
}

fn read_text(
    zip: &mut ZipArchive<BufReader<File>>,
    archive: &Path,
    name: &str,
) -> CoreResult<String> {
    let bytes = read_entry(zip, archive, name, MAX_XML_BYTES)?;
    Ok(decode_text(bytes))
}

/// Decodifica texto UTF-8 (con o sin BOM). Los bytes inválidos se sustituyen.
fn decode_text(bytes: Vec<u8>) -> String {
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    };
    match text.strip_prefix('\u{feff}') {
        Some(stripped) => stripped.to_owned(),
        None => text,
    }
}

fn is_document(media_type: &str) -> bool {
    matches!(
        media_type,
        "application/xhtml+xml" | "text/html" | "application/xml" | "text/xml"
    ) || media_type.ends_with("+xml") && media_type != "image/svg+xml"
}

/// Tipo MIME por extensión para recursos que no figuran en el manifiesto.
pub fn media_type_from_extension(href: &str) -> &'static str {
    let extension = href
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "xhtml" | "xht" => "application/xhtml+xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "mp3" => "audio/mpeg",
        "ncx" => "application/x-dtbncx+xml",
        _ => "application/octet-stream",
    }
}

/// Ruta del paquete OPF según `container.xml`.
fn parse_container(xml: &str) -> Option<String> {
    let mut reader = xml::reader(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if xml::local_name(&e) == "rootfile" => {
                let media_type = xml::attribute(&e, "media-type").unwrap_or_default();
                if media_type.is_empty() || media_type == "application/oebps-package+xml" {
                    return xml::attribute(&e, "full-path").and_then(|p| path::normalize(&p));
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
struct Package {
    metadata: EpubMetadata,
    manifest: Vec<ManifestItem>,
    /// `(idref, linear)`
    spine: Vec<(String, bool)>,
    ncx_id: Option<String>,
    cover_id: Option<String>,
}

fn parse_opf(opf: &str, opf_path: &str) -> Package {
    let mut package = Package::default();
    let mut reader = xml::reader(opf);
    // `refines` de EPUB 3: id del elemento de colección → número de serie.
    let mut collection_ids: Vec<(String, String)> = Vec::new();
    let mut refined_positions: Vec<(String, String)> = Vec::new();

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        let (Event::Start(e) | Event::Empty(e)) = &event else {
            continue;
        };
        let is_start = matches!(event, Event::Start(_));
        let name = xml::local_name(e);
        match name.as_str() {
            "title" if is_start && package.metadata.title.is_none() => {
                package.metadata.title =
                    Some(xml::element_text(&mut reader)).filter(|t| !t.is_empty());
            }
            "creator" if is_start => {
                let author = xml::element_text(&mut reader);
                if !author.is_empty() {
                    package.metadata.authors.push(author);
                }
            }
            "language" if is_start && package.metadata.language.is_none() => {
                package.metadata.language =
                    Some(xml::element_text(&mut reader)).filter(|t| !t.is_empty());
            }
            "meta" => {
                let property = xml::attribute(e, "property");
                let meta_name = xml::attribute(e, "name");
                let content = xml::attribute(e, "content");
                match (meta_name.as_deref(), property.as_deref()) {
                    (Some("cover"), _) => package.cover_id = content,
                    (Some("calibre:series"), _) => {
                        package.metadata.series = content.filter(|c| !c.is_empty());
                    }
                    (Some("calibre:series_index"), _) => {
                        package.metadata.series_index = content.and_then(|c| c.parse().ok());
                    }
                    (_, Some("belongs-to-collection")) if is_start => {
                        let id = xml::attribute(e, "id");
                        let value = xml::element_text(&mut reader);
                        if package.metadata.series.is_none() && !value.is_empty() {
                            package.metadata.series = Some(value.clone());
                        }
                        if let Some(id) = id {
                            collection_ids.push((format!("#{id}"), value));
                        }
                    }
                    (_, Some("group-position")) if is_start => {
                        let refines = xml::attribute(e, "refines").unwrap_or_default();
                        refined_positions.push((refines, xml::element_text(&mut reader)));
                    }
                    _ => {}
                }
            }
            "item" => {
                let (Some(id), Some(href)) = (xml::attribute(e, "id"), xml::attribute(e, "href"))
                else {
                    continue;
                };
                let Some(href) = path::resolve(opf_path, &href) else {
                    continue;
                };
                package.manifest.push(ManifestItem {
                    id,
                    href,
                    media_type: xml::attribute(e, "media-type").unwrap_or_default(),
                    properties: xml::attribute(e, "properties").unwrap_or_default(),
                });
            }
            "spine" => package.ncx_id = xml::attribute(e, "toc"),
            "itemref" => {
                if let Some(idref) = xml::attribute(e, "idref") {
                    let linear = xml::attribute(e, "linear").as_deref() != Some("no");
                    package.spine.push((idref, linear));
                }
            }
            _ => {}
        }
    }

    if package.metadata.series_index.is_none() {
        package.metadata.series_index = refined_positions.iter().find_map(|(refines, value)| {
            collection_ids
                .iter()
                .any(|(id, _)| id == refines)
                .then(|| value.parse().ok())
                .flatten()
        });
    }
    package
}

/// Índice del documento de navegación de EPUB 3 (`<nav epub:type="toc">`).
fn parse_nav(xhtml: &str, nav_path: &str) -> Vec<TocEntry> {
    let mut reader = xml::reader(xhtml);
    let mut entries = Vec::new();
    // Profundidad de `nav` (0 = fuera) y nivel de `ol` dentro del índice.
    let mut in_toc = false;
    let mut nav_depth = 0u32;
    let mut list_level = 0u32;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match xml::local_name(&e).as_str() {
                "nav" => {
                    if in_toc {
                        nav_depth += 1;
                    } else {
                        let kind = xml::attribute(&e, "type").unwrap_or_default();
                        // Algunos libros omiten `epub:type`; si no hay otro, sirve el primero.
                        if kind.split_whitespace().any(|t| t == "toc") || kind.is_empty() {
                            in_toc = true;
                            nav_depth = 1;
                        }
                    }
                }
                "ol" if in_toc => list_level += 1,
                "a" if in_toc => {
                    let href = xml::attribute(&e, "href").unwrap_or_default();
                    let title = xml::element_text(&mut reader);
                    push_entry(
                        &mut entries,
                        nav_path,
                        &href,
                        title,
                        list_level.saturating_sub(1),
                    );
                }
                _ => {}
            },
            Ok(Event::End(e)) if in_toc => {
                match e.local_name().as_ref().to_ascii_lowercase().as_str() {
                    "ol" => list_level = list_level.saturating_sub(1),
                    "nav" => {
                        nav_depth -= 1;
                        if nav_depth == 0 {
                            // Si el primer `nav` no tenía tipo y resultó vacío, se sigue buscando.
                            if !entries.is_empty() {
                                break;
                            }
                            in_toc = false;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    entries
}

/// Índice del NCX de EPUB 2 (`navMap/navPoint`).
fn parse_ncx(ncx: &str, ncx_path: &str) -> Vec<TocEntry> {
    let mut reader = xml::reader(ncx);
    let mut entries = Vec::new();
    let mut depth = 0u32;
    let mut pending_title: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match xml::local_name(&e).as_str() {
                "navpoint" => depth += 1,
                "text" if depth > 0 => pending_title = Some(xml::element_text(&mut reader)),
                "content" if depth > 0 => {
                    let href = xml::attribute(&e, "src").unwrap_or_default();
                    let title = pending_title.take().unwrap_or_default();
                    push_entry(&mut entries, ncx_path, &href, title, depth - 1);
                }
                _ => {}
            },
            Ok(Event::Empty(e)) if depth > 0 && xml::local_name(&e) == "content" => {
                let href = xml::attribute(&e, "src").unwrap_or_default();
                let title = pending_title.take().unwrap_or_default();
                push_entry(&mut entries, ncx_path, &href, title, depth - 1);
            }
            Ok(Event::End(e)) => {
                if e.local_name().as_ref().eq_ignore_ascii_case("navpoint") {
                    depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    entries
}

fn push_entry(entries: &mut Vec<TocEntry>, base: &str, href: &str, title: String, level: u32) {
    let Some(target) = path::resolve(base, href) else {
        return;
    };
    if title.is_empty() || level > MAX_TOC_DEPTH {
        return;
    }
    let (_, fragment) = path::split_fragment(href);
    entries.push(TocEntry {
        title,
        href: target,
        fragment: fragment.map(str::to_owned),
        level,
    });
}

#[cfg(test)]
mod tests;
