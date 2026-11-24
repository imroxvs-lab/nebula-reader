//! Resolución segura de rutas dentro de un EPUB.
//!
//! Los `href` de un EPUB son URL relativas (con `%20`, `../`, fragmentos…). Se
//! convierten en rutas absolutas dentro del ZIP y se rechaza cualquier ruta que
//! intente salir de la raíz.

/// Separa una URL relativa en ruta y fragmento (`cap1.xhtml#nota3`).
pub fn split_fragment(href: &str) -> (&str, Option<&str>) {
    match href.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment).filter(|f| !f.is_empty())),
        None => (href, None),
    }
}

/// Resuelve `href` relativo al archivo `base` (ruta dentro del ZIP) y devuelve la
/// ruta normalizada **sin fragmento**.
///
/// Devuelve `None` si la URL es externa (`http:`, `mailto:`…), está vacía o se
/// sale de la raíz del archivo.
pub fn resolve(base: &str, href: &str) -> Option<String> {
    let (path, _) = split_fragment(href.trim());
    let path = path.split('?').next().unwrap_or_default();
    if path.is_empty() || has_scheme(path) {
        return None;
    }
    let decoded = percent_decode(path)?;
    let joined = if let Some(absolute) = decoded.strip_prefix('/') {
        absolute.to_owned()
    } else {
        match base.rsplit_once('/') {
            Some((directory, _)) => format!("{directory}/{decoded}"),
            None => decoded,
        }
    };
    normalize(&joined)
}

/// Normaliza una ruta: elimina `.` y segmentos vacíos y aplica `..`.
/// Devuelve `None` si `..` intenta salir de la raíz.
pub fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split(['/', '\\']) {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Decodifica secuencias `%XX`. Devuelve `None` si el resultado no es UTF-8.
pub fn percent_decode(input: &str) -> Option<String> {
    if !input.contains('%') {
        return Some(input.to_owned());
    }
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%'
            && let Some(hex) = input.get(index + 1..index + 3)
            && let Ok(value) = u8::from_str_radix(hex, 16)
        {
            output.push(value);
            index += 3;
            continue;
        }
        output.push(byte);
        index += 1;
    }
    String::from_utf8(output).ok()
}

/// Codifica cada segmento de una ruta para usarla en una URL.
pub fn percent_encode_path(path: &str) -> String {
    let mut output = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            output.push(char::from(byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn has_scheme(href: &str) -> bool {
    match href.split_once(':') {
        Some((scheme, _)) => {
            !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                && scheme
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn resolves_relative_to_base_directory() {
        assert_eq!(
            resolve("OEBPS/Text/cap1.xhtml", "../Images/a%20b.jpg"),
            Some("OEBPS/Images/a b.jpg".to_owned())
        );
    }

    #[test]
    fn strips_fragment_and_query() {
        assert_eq!(
            resolve("OEBPS/toc.xhtml", "cap2.xhtml?x=1#sec3"),
            Some("OEBPS/cap2.xhtml".to_owned())
        );
    }

    #[test]
    fn root_relative_href_ignores_base() {
        assert_eq!(resolve("a/b/c.xhtml", "/d.css"), Some("d.css".to_owned()));
    }

    #[test]
    fn base_at_root_resolves_plainly() {
        assert_eq!(
            resolve("content.opf", "cap.xhtml"),
            Some("cap.xhtml".to_owned())
        );
    }

    #[test]
    fn escaping_the_root_is_rejected() {
        assert_eq!(resolve("OEBPS/cap.xhtml", "../../etc/passwd"), None);
    }

    #[test]
    fn external_urls_are_rejected() {
        assert_eq!(
            [
                resolve("a.xhtml", "https://example.com"),
                resolve("a.xhtml", "mailto:x@y.z"),
                resolve("a.xhtml", "javascript:alert(1)"),
            ],
            [None, None, None]
        );
    }

    #[test]
    fn windows_drive_like_names_are_not_schemes_when_digit_first() {
        assert_eq!(
            resolve("a.xhtml", "1:2.xhtml"),
            Some("1:2.xhtml".to_owned())
        );
    }

    #[test]
    fn invalid_percent_sequences_are_kept_literally() {
        assert_eq!(percent_decode("100%zz"), Some("100%zz".to_owned()));
    }

    #[test]
    fn split_fragment_ignores_empty_fragment() {
        assert_eq!(split_fragment("a.xhtml#"), ("a.xhtml", None));
    }

    #[test]
    fn encode_keeps_slashes_and_escapes_spaces() {
        assert_eq!(
            percent_encode_path("OEBPS/a b/ñ.jpg"),
            "OEBPS/a%20b/%C3%B1.jpg"
        );
    }

    proptest! {
        #[test]
        fn resolved_paths_never_escape_or_contain_dot_segments(
            base in "[a-z]{1,4}(/[a-z]{1,4}){0,3}\\.xhtml",
            href in "(\\.\\./|\\./|[a-z]{1,4}/){0,6}[a-z]{1,4}\\.(jpg|css)",
        ) {
            if let Some(path) = resolve(&base, &href) {
                prop_assert!(!path.starts_with('/'));
                prop_assert!(path.split('/').all(|s| !s.is_empty() && s != "." && s != ".."));
            }
        }

        #[test]
        fn encode_then_decode_roundtrips(path in "[a-zA-Z0-9 ñáé._/-]{1,30}") {
            prop_assert_eq!(percent_decode(&percent_encode_path(&path)), Some(path));
        }
    }
}
