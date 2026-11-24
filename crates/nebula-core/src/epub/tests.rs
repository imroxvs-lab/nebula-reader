#![allow(clippy::expect_used)] // Infraestructura de test.

use std::io::Write;

use zip::write::SimpleFileOptions;

use super::*;

const CONTAINER: &str = r##"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"##;

const OPF3: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Cien años &amp; soledad</dc:title>
    <dc:creator id="a1">Gabriel García Márquez</dc:creator>
    <dc:language>es</dc:language>
    <meta property="belongs-to-collection" id="c1">Clásicos</meta>
    <meta refines="#c1" property="group-position">3</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="Text/cap%201.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="Text/cap2.xhtml" media-type="application/xhtml+xml"/>
    <item id="notes" href="Text/notas.xhtml" media-type="application/xhtml+xml"/>
    <item id="img" href="Images/portada.jpg" media-type="image/jpeg" properties="cover-image"/>
    <item id="css" href="Styles/a.css" media-type="text/css"/>
    <item id="evil" href="../../fuera.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
    <itemref idref="missing"/>
    <itemref idref="c2"/>
    <itemref idref="notes" linear="no"/>
    <itemref idref="img"/>
  </spine>
</package>"##;

const NAV: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<body>
  <nav epub:type="landmarks"><ol><li><a href="Text/cap2.xhtml">Ignorar</a></li></ol></nav>
  <nav epub:type="toc" id="toc">
    <h1>Índice</h1>
    <ol>
      <li><a href="Text/cap%201.xhtml">Capítulo <em>uno</em></a>
        <ol><li><a href="Text/cap%201.xhtml#s2">Sección 2</a></li></ol>
      </li>
      <li><a href="Text/cap2.xhtml">Capítulo dos</a></li>
      <li><a href="http://example.com">Externo</a></li>
    </ol>
  </nav>
</body>
</html>"##;

const OPF2: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>El Hobbit</dc:title>
    <dc:creator opf:role="aut">J. R. R. Tolkien</dc:creator>
    <meta name="cover" content="cover-img"/>
    <meta name="calibre:series" content="Tierra Media"/>
    <meta name="calibre:series_index" content="1.0"/>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="p1" href="parte1.html" media-type="application/xhtml+xml"/>
    <item id="cover-img" href="img/c.png" media-type="image/png"/>
  </manifest>
  <spine toc="ncx"><itemref idref="p1"/></spine>
</package>"##;

const NCX: &str = r##"<?xml version="1.0"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/">
  <navMap>
    <navPoint id="n1"><navLabel><text>Un viaje inesperado</text></navLabel><content src="parte1.html#c1"/>
      <navPoint id="n2"><navLabel><text>Bilbo</text></navLabel><content src="parte1.html#c1b"/></navPoint>
    </navPoint>
  </navMap>
</ncx>"##;

fn make_epub(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
    let path = dir.join("libro.epub");
    let mut zip = zip::ZipWriter::new(File::create(&path).expect("crear"));
    for (name, data) in entries {
        zip.start_file(*name, SimpleFileOptions::default())
            .expect("entrada");
        zip.write_all(data).expect("escribir");
    }
    zip.finish().expect("cerrar");
    path
}

fn epub3(dir: &Path) -> PathBuf {
    make_epub(
        dir,
        &[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", CONTAINER.as_bytes()),
            ("OEBPS/content.opf", OPF3.as_bytes()),
            ("OEBPS/nav.xhtml", NAV.as_bytes()),
            (
                "OEBPS/Text/cap 1.xhtml",
                b"<html><body><p>uno</p></body></html>",
            ),
            (
                "OEBPS/Text/cap2.xhtml",
                b"<html><body><p>dos dos</p></body></html>",
            ),
            ("OEBPS/Text/notas.xhtml", b"<html/>"),
            ("OEBPS/Images/portada.jpg", b"JPEG"),
            ("OEBPS/Styles/a.css", b"p{}"),
        ],
    )
}

fn epub2(dir: &Path) -> PathBuf {
    let container = CONTAINER.replace("OEBPS/content.opf", "content.opf");
    make_epub(
        dir,
        &[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", container.as_bytes()),
            ("content.opf", OPF2.as_bytes()),
            ("toc.ncx", NCX.as_bytes()),
            ("parte1.html", b"<html><body>x</body></html>"),
            ("img/c.png", b"PNG"),
        ],
    )
}

#[test]
fn epub3_metadata_is_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = EpubBook::open(epub3(dir.path())).expect("abrir");
    assert_eq!(
        book.metadata(),
        &EpubMetadata {
            title: Some("Cien años & soledad".to_owned()),
            authors: vec!["Gabriel García Márquez".to_owned()],
            language: Some("es".to_owned()),
            series: Some("Clásicos".to_owned()),
            series_index: Some(3.0),
        }
    );
}

#[test]
fn epub3_spine_skips_missing_and_non_documents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = EpubBook::open(epub3(dir.path())).expect("abrir");
    let spine: Vec<(&str, bool)> = book
        .spine()
        .iter()
        .map(|item| (item.href.as_str(), item.linear))
        .collect();
    assert_eq!(
        spine,
        [
            ("OEBPS/Text/cap 1.xhtml", true),
            ("OEBPS/Text/cap2.xhtml", true),
            ("OEBPS/Text/notas.xhtml", false),
        ]
    );
}

#[test]
fn epub3_toc_comes_from_nav_with_levels_and_fragments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = EpubBook::open(epub3(dir.path())).expect("abrir");
    let toc: Vec<(&str, &str, Option<&str>, u32)> = book
        .toc()
        .iter()
        .map(|e| {
            (
                e.title.as_str(),
                e.href.as_str(),
                e.fragment.as_deref(),
                e.level,
            )
        })
        .collect();
    assert_eq!(
        toc,
        [
            ("Capítulo uno", "OEBPS/Text/cap 1.xhtml", None, 0),
            ("Sección 2", "OEBPS/Text/cap 1.xhtml", Some("s2"), 1),
            ("Capítulo dos", "OEBPS/Text/cap2.xhtml", None, 0),
        ]
    );
}

#[test]
fn epub3_cover_uses_cover_image_property() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut book = EpubBook::open(epub3(dir.path())).expect("abrir");
    assert_eq!(book.read_cover().ok().flatten(), Some(b"JPEG".to_vec()));
}

#[test]
fn manifest_entries_outside_root_are_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = EpubBook::open(epub3(dir.path())).expect("abrir");
    assert!(
        book.manifest
            .iter()
            .all(|item| !item.href.contains("fuera"))
    );
}

#[test]
fn resources_are_read_with_media_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut book = EpubBook::open(epub3(dir.path())).expect("abrir");
    let css = book.read_resource("OEBPS/Styles/a.css").expect("css");
    assert_eq!(
        (css.bytes, css.media_type.as_str()),
        (b"p{}".to_vec(), "text/css")
    );
}

#[test]
fn missing_resource_is_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut book = EpubBook::open(epub3(dir.path())).expect("abrir");
    assert!(matches!(
        book.read_resource("OEBPS/no.css"),
        Err(CoreError::NotFoundInArchive(_))
    ));
}

#[test]
fn traversal_in_resource_path_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut book = EpubBook::open(epub3(dir.path())).expect("abrir");
    assert!(matches!(
        book.read_resource("../../secreto"),
        Err(CoreError::NotFoundInArchive(_))
    ));
}

#[test]
fn epub2_metadata_series_and_cover() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut book = EpubBook::open(epub2(dir.path())).expect("abrir");
    let meta = book.metadata().clone();
    assert_eq!(
        (
            meta.title.as_deref(),
            meta.authors.as_slice(),
            meta.series.as_deref(),
            meta.series_index,
            book.read_cover().ok().flatten()
        ),
        (
            Some("El Hobbit"),
            ["J. R. R. Tolkien".to_owned()].as_slice(),
            Some("Tierra Media"),
            Some(1.0),
            Some(b"PNG".to_vec())
        )
    );
}

#[test]
fn epub2_toc_comes_from_ncx() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = EpubBook::open(epub2(dir.path())).expect("abrir");
    let toc: Vec<(&str, Option<&str>, u32)> = book
        .toc()
        .iter()
        .map(|e| (e.title.as_str(), e.fragment.as_deref(), e.level))
        .collect();
    assert_eq!(
        toc,
        [
            ("Un viaje inesperado", Some("c1"), 0),
            ("Bilbo", Some("c1b"), 1)
        ]
    );
}

#[test]
fn missing_container_is_an_archive_error_not_a_panic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = make_epub(dir.path(), &[("mimetype", b"application/epub+zip")]);
    assert!(matches!(
        EpubBook::open(path),
        Err(CoreError::NotFoundInArchive(_))
    ));
}

#[test]
fn empty_spine_is_an_archive_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let opf = r#"<package><metadata/><manifest/><spine/></package>"#;
    let path = make_epub(
        dir.path(),
        &[
            ("META-INF/container.xml", CONTAINER.as_bytes()),
            ("OEBPS/content.opf", opf.as_bytes()),
        ],
    );
    assert!(matches!(
        EpubBook::open(path),
        Err(CoreError::Archive { .. })
    ));
}

#[test]
fn garbage_xml_does_not_panic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = make_epub(
        dir.path(),
        &[
            ("META-INF/container.xml", b"<<<&&&\xff\xfe"),
            ("OEBPS/content.opf", b"\x00\x01"),
        ],
    );
    assert!(EpubBook::open(path).is_err());
}

#[test]
fn bom_is_stripped_from_text() {
    assert_eq!(decode_text(b"\xEF\xBB\xBFhola".to_vec()), "hola");
}
