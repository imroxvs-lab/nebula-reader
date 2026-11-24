//! Utilidades mínimas sobre `quick-xml` para leer los XML de EPUB y CBZ.
//!
//! Se comparan siempre nombres locales (sin prefijo de espacio de nombres):
//! los EPUB reales mezclan prefijos (`dc:`, `opf:`, sin prefijo…) y para lo que
//! necesitamos no hay ambigüedad.

use quick_xml::XmlVersion;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::reader::Reader;

/// Lector configurado para documentos de libros (tolerante con espacios).
pub(crate) fn reader(xml: &str) -> Reader<&[u8]> {
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    // Muchos EPUB tienen etiquetas de cierre que no casan (HTML descuidado);
    // no deben impedir leer el resto.
    config.check_end_names = false;
    config.allow_unmatched_ends = true;
    config.allow_dangling_amp = true;
    reader
}

/// Nombre local de un elemento, en minúsculas.
pub(crate) fn local_name(element: &BytesStart<'_>) -> String {
    element.local_name().as_ref().to_ascii_lowercase()
}

/// Valor de un atributo por su nombre local (sin distinguir mayúsculas).
pub(crate) fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .with_checks(false)
        .flatten()
        .find(|attr| attr.key.local_name().as_ref().eq_ignore_ascii_case(name))
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.trim().to_owned())
        })
}

/// Texto resuelto de una referencia a entidad (`&amp;`, `&#233;`, `&nbsp;`…).
pub(crate) fn entity_text(reference: &BytesRef<'_>) -> Option<String> {
    if reference.is_char_ref() {
        return reference
            .resolve_char_ref()
            .ok()
            .flatten()
            .map(String::from);
    }
    let name = reference.xml10_content();
    let text = match name.as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "mdash" => "—",
        "ndash" => "–",
        "hellip" => "…",
        "laquo" => "«",
        "raquo" => "»",
        "iexcl" => "¡",
        "iquest" => "¿",
        _ => return None,
    };
    Some(text.to_owned())
}

/// Lee todo el texto hasta el cierre del elemento actual (incluido el de sus
/// descendientes) y lo devuelve con los espacios colapsados.
///
/// Se debe llamar justo después de recibir el `Start` del elemento.
pub(crate) fn element_text(reader: &mut Reader<&[u8]>) -> String {
    let mut text = String::new();
    let mut depth = 1usize;
    while depth > 0 {
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => depth -= 1,
            Ok(Event::Text(t)) => text.push_str(&t.xml10_content()),
            Ok(Event::CData(c)) => text.push_str(&c.xml10_content()),
            Ok(Event::GeneralRef(r)) => {
                if let Some(resolved) = entity_text(&r) {
                    text.push_str(&resolved);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    collapse_whitespace(&text)
}

/// Colapsa espacios (incluidos saltos de línea) y recorta los extremos.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_start_text(xml: &str) -> String {
        let mut reader = reader(xml);
        loop {
            match reader.read_event() {
                Ok(Event::Start(_)) => return element_text(&mut reader),
                Ok(Event::Eof) | Err(_) => return String::new(),
                _ => {}
            }
        }
    }

    #[test]
    fn element_text_resolves_entities_and_nesting() {
        assert_eq!(
            first_start_text("<a>Tom &amp; <b>Jerry</b>&#233;&nbsp;x</a>"),
            "Tom & Jerryé x"
        );
    }

    #[test]
    fn element_text_collapses_whitespace() {
        assert_eq!(
            first_start_text("<a>\n  Hola\n   mundo  </a>"),
            "Hola mundo"
        );
    }

    #[test]
    fn attribute_matches_local_name_ignoring_prefix() {
        let mut reader = reader(r#"<item opf:Href="a&amp;b.xhtml" id=" x "/>"#);
        let Ok(Event::Empty(element)) = reader.read_event() else {
            panic!("se esperaba un elemento vacío");
        };
        assert_eq!(
            (attribute(&element, "href"), attribute(&element, "id")),
            (Some("a&b.xhtml".to_owned()), Some("x".to_owned()))
        );
    }
}
