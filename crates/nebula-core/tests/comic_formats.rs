//! Tests de CBR (RAR) y CB7 (7z).
//!
//! Los RAR de `tests/fixtures` se crearon con WinRAR (no hay forma libre de
//! crear RAR); los 7z se generan en cada test.

#![allow(clippy::expect_used)] // Infraestructura de test.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use nebula_core::archive::ContainerKind;
use nebula_core::metadata::read_metadata;
use nebula_core::{BookFormat, ComicArchive, CoreError};
use sevenz_rust2::{ArchiveEntry, ArchiveWriter, SourceReader};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn names(archive: &ComicArchive) -> Vec<&str> {
    (0..archive.page_count())
        .filter_map(|i| archive.page_name(i))
        .collect()
}

const PAGES: [(&str, &[u8]); 4] = [
    ("p10.jpg", b"diez"),
    ("p2.jpg", b"dos"),
    ("p1.jpg", b"uno"),
    ("leeme.txt", b"no es imagen"),
];

fn make_7z(dir: &Path, solid: bool) -> PathBuf {
    let path = dir.join(if solid { "solido.cb7" } else { "normal.cb7" });
    let mut writer =
        ArchiveWriter::new(std::fs::File::create(&path).expect("crear")).expect("writer");
    if solid {
        let entries = PAGES
            .iter()
            .map(|(n, _)| ArchiveEntry::new_file(n))
            .collect();
        let readers = PAGES
            .iter()
            .map(|(_, d)| SourceReader::new(Cursor::new(d.to_vec())))
            .collect();
        writer
            .push_archive_entries(entries, readers)
            .expect("entradas sólidas");
    } else {
        for (name, data) in PAGES {
            writer
                .push_archive_entry(ArchiveEntry::new_file(name), Some(Cursor::new(data)))
                .expect("entrada");
        }
    }
    writer.finish().expect("cerrar").flush().expect("flush");
    path
}

#[test]
fn rar_pages_are_listed_in_natural_order() {
    let archive = ComicArchive::open(fixture("comic.cbr")).expect("abrir");
    assert_eq!(
        (archive.kind(), names(&archive)),
        (ContainerKind::Rar, vec!["p1.png", "p2.png", "p10.png"])
    );
}

#[test]
fn rar_pages_are_read_in_any_order() {
    let mut archive = ComicArchive::open(fixture("comic.cbr")).expect("abrir");
    let sizes: Vec<usize> = [2, 0, 1, 0]
        .into_iter()
        .map(|i| archive.read_page(i).expect("página").bytes.len())
        .collect();
    let png = sizes.iter().all(|&s| s > 50);
    assert!(png, "{sizes:?}");
}

#[test]
fn rar_page_bytes_are_valid_png() {
    let mut archive = ComicArchive::open(fixture("comic.cbr")).expect("abrir");
    let page = archive.read_page(1).expect("página");
    assert_eq!(&page.bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn solid_rar_is_readable() {
    let mut archive = ComicArchive::open(fixture("solid.cbr")).expect("abrir");
    let last = archive.page_count() - 1;
    assert!(archive.read_page(last).is_ok() && archive.read_page(0).is_ok());
}

#[test]
fn rar_with_encrypted_data_is_protected() {
    assert!(matches!(
        ComicArchive::open(fixture("protected-data.cbr")),
        Err(CoreError::Protected(_))
    ));
}

#[test]
fn rar_with_encrypted_headers_is_protected() {
    assert!(matches!(
        ComicArchive::open(fixture("protected-headers.cbr")),
        Err(CoreError::Protected(_))
    ));
}

#[test]
fn comic_info_is_read_from_rar() {
    let meta = read_metadata(&fixture("comic.cbr"), BookFormat::Cbr).expect("metadatos");
    assert_eq!(
        (
            meta.series.as_deref(),
            meta.series_index,
            meta.author.as_deref()
        ),
        (Some("Prueba RAR"), Some(3.0), Some("Autora RAR"))
    );
}

#[test]
fn seven_zip_pages_are_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut archive = ComicArchive::open(make_7z(dir.path(), false)).expect("abrir");
    let page = archive.read_page(2).expect("página").bytes;
    assert_eq!(
        (archive.kind(), names(&archive), page),
        (
            ContainerKind::SevenZ,
            vec!["p1.jpg", "p2.jpg", "p10.jpg"],
            b"diez".to_vec()
        )
    );
}

#[test]
fn solid_seven_zip_pages_are_read_in_any_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut archive = ComicArchive::open(make_7z(dir.path(), true)).expect("abrir");
    let pages: Vec<Vec<u8>> = [2, 0, 1]
        .into_iter()
        .map(|i| archive.read_page(i).expect("página").bytes)
        .collect();
    assert_eq!(pages, [b"diez".to_vec(), b"uno".to_vec(), b"dos".to_vec()]);
}

#[test]
fn zip_named_cbr_is_detected_by_signature() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("en-realidad-zip.cbr");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("crear"));
    zip.start_file("a.jpg", zip::write::SimpleFileOptions::default())
        .expect("entrada");
    zip.write_all(b"x").expect("escribir");
    zip.finish().expect("cerrar");
    assert_eq!(
        ComicArchive::open(&path).map(|a| a.kind()).ok(),
        Some(ContainerKind::Zip)
    );
}

#[test]
fn unknown_signature_is_an_archive_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("x.cb7");
    std::fs::write(&path, b"nada de nada").expect("escribir");
    assert!(matches!(
        ComicArchive::open(&path),
        Err(CoreError::Archive { .. })
    ));
}
