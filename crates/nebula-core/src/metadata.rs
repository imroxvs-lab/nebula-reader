//! Metadatos de libros y cómics leídos del propio archivo.
//!
//! - EPUB: paquete OPF (`dc:title`, `dc:creator`, serie de EPUB 3 o Calibre).
//! - CBZ, CBR y CB7: `ComicInfo.xml` (formato de ComicRack: `Series`, `Number`, `Writer`).
//!
//! Leer metadatos nunca es imprescindible: si falla, se usa el nombre del archivo.

use std::path::Path;

use crate::archive::ComicArchive;
use crate::domain::BookFormat;
use crate::epub::EpubBook;
use crate::error::CoreResult;
use crate::xml;
use quick_xml::events::Event;

/// Longitud máxima de cada campo guardado.
const MAX_FIELD_CHARS: usize = 300;

/// Metadatos descriptivos de una obra.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookMetadata {
    /// Título real, si el archivo lo declara.
    pub title: Option<String>,
    /// Autor o autores, separados por «, ».
    pub author: Option<String>,
    /// Serie a la que pertenece.
    pub series: Option<String>,
    /// Número dentro de la serie.
    pub series_index: Option<f64>,
}

impl BookMetadata {
    /// Indica si no contiene nada útil.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.author.is_none()
            && self.series.is_none()
            && self.series_index.is_none()
    }
}

/// Lee los metadatos de un archivo según su formato.
///
/// # Errors
/// Errores de lectura del archivo; el llamador decide si ignorarlos.
pub fn read_metadata(path: &Path, format: BookFormat) -> CoreResult<BookMetadata> {
    match format {
        BookFormat::Epub => Ok(from_epub(&EpubBook::open(path)?)),
        BookFormat::Cbz | BookFormat::Cbr | BookFormat::Cb7 => read_comic_info(path),
        // Los metadatos de PDF necesitan el motor PDFium (capa de aplicación).
        BookFormat::Pdf => Ok(BookMetadata::default()),
    }
}

/// Metadatos de un EPUB ya abierto.
pub fn from_epub(book: &EpubBook) -> BookMetadata {
    let meta = book.metadata();
    let authors = meta
        .authors
        .iter()
        .map(|a| clean(a))
        .filter(|a| !a.is_empty())
        .collect::<Vec<_>>();
    BookMetadata {
        title: meta.title.as_deref().map(clean).filter(|t| !t.is_empty()),
        author: (!authors.is_empty()).then(|| authors.join(", ")),
        series: meta.series.as_deref().map(clean).filter(|s| !s.is_empty()),
        series_index: meta.series_index.filter(|n| n.is_finite()),
    }
}

fn read_comic_info(path: &Path) -> CoreResult<BookMetadata> {
    let mut archive = ComicArchive::open(path)?;
    Ok(archive
        .read_comic_info()?
        .map(|bytes| parse_comic_info(&String::from_utf8_lossy(&bytes)))
        .unwrap_or_default())
}

/// Interpreta un `ComicInfo.xml`.
///
/// El título del número (`Title`) suele ser el de la historia, no el del cómic,
/// así que solo se usa si no hay serie.
pub fn parse_comic_info(xml_text: &str) -> BookMetadata {
    let mut reader = xml::reader(xml_text);
    let (mut title, mut series, mut number, mut writer) = (None, None, None, None);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let slot = match xml::local_name(&e).as_str() {
                    "title" => &mut title,
                    "series" => &mut series,
                    "number" => &mut number,
                    "writer" => &mut writer,
                    _ => continue,
                };
                let value = clean(&xml::element_text(&mut reader));
                if !value.is_empty() {
                    *slot = Some(value);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    BookMetadata {
        title: if series.is_none() { title } else { None },
        author: writer,
        series,
        series_index: number.and_then(|n| n.replace(',', ".").parse().ok()),
    }
}

fn clean(text: &str) -> String {
    let collapsed = xml::collapse_whitespace(text);
    match collapsed.char_indices().nth(MAX_FIELD_CHARS) {
        Some((cut, _)) => collapsed[..cut].to_owned(),
        None => collapsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comic_info_fields_are_read() {
        let xml = r"<?xml version='1.0'?>
            <ComicInfo><Title>El comienzo</Title><Series>Saga</Series>
            <Number>12</Number><Writer>Brian K. Vaughan</Writer></ComicInfo>";
        assert_eq!(
            parse_comic_info(xml),
            BookMetadata {
                title: None,
                author: Some("Brian K. Vaughan".to_owned()),
                series: Some("Saga".to_owned()),
                series_index: Some(12.0),
            }
        );
    }

    #[test]
    fn comic_info_title_is_used_without_series() {
        let xml = "<ComicInfo><Title>Maus</Title><Number>1,5</Number></ComicInfo>";
        let meta = parse_comic_info(xml);
        assert_eq!(
            (meta.title.as_deref(), meta.series_index),
            (Some("Maus"), Some(1.5))
        );
    }

    #[test]
    fn invalid_comic_info_is_empty() {
        assert!(parse_comic_info("<<<no es xml").is_empty());
    }

    #[test]
    fn long_fields_are_truncated() {
        assert_eq!(clean(&"a".repeat(1000)).chars().count(), MAX_FIELD_CHARS);
    }
}
