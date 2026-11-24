//! Tests de PDF con PDFium real.
//!
//! Necesitan la biblioteca nativa: se ejecutan si la variable `NEBULA_PDFIUM`
//! apunta a `pdfium.dll` y, si no, se omiten (con un aviso).
//!
//! ```text
//! set NEBULA_PDFIUM=src-tauri\resources\pdfium\pdfium.dll
//! cargo test -p nebula-core --features pdf --test pdf
//! ```
#![cfg(feature = "pdf")]
#![allow(clippy::expect_used, clippy::print_stderr)] // Infraestructura de test.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use nebula_core::CoreError;
use nebula_core::pdf::PdfEngine;

fn engine() -> Option<&'static PdfEngine> {
    static ENGINE: OnceLock<Option<PdfEngine>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let path = std::env::var_os("NEBULA_PDFIUM").map(PathBuf::from)?;
            // Las rutas relativas se interpretan desde la raíz del workspace.
            let path = if path.is_relative() {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(path)
            } else {
                path
            };
            Some(PdfEngine::load(&path).expect("cargar PDFium"))
        })
        .as_ref()
}

macro_rules! require_engine {
    () => {
        match engine() {
            Some(engine) => engine,
            None => {
                eprintln!("NEBULA_PDFIUM no definida: test omitido");
                return;
            }
        }
    };
}

fn write_pdf(dir: &Path, pages: u32) -> PathBuf {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            (0..pages)
                .map(|i| format!("{} 0 R ", 5 + i * 2))
                .collect::<String>()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        "<< /Title (Informe anual) /Author (Ana Autora) >>".to_owned(),
    ];
    for page in 1..=pages {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            6 + (page - 1) * 2
        ));
        let stream = format!("BT /F1 72 Tf 200 400 Td ({page}) Tj ET");
        objects.push(format!(
            "<< /Length {} >>\nstream\n{stream}\nendstream",
            stream.len() + 1
        ));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(tail, "{offset:010} 00000 n ");
    }
    let _ = write!(
        tail,
        "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    out.extend_from_slice(tail.as_bytes());
    let path = dir.join("doc.pdf");
    std::fs::write(&path, out).expect("escribir pdf");
    path
}

#[test]
fn info_reports_pages_and_a4_ratio() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    let info = engine.info(&write_pdf(dir.path(), 3)).expect("info");
    assert_eq!(
        (info.page_count, (info.aspect_ratio * 100.0).round()),
        (3, 142.0)
    );
}

#[test]
fn render_produces_jpeg_at_requested_width() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    let page = engine
        .render(&write_pdf(dir.path(), 2), 1, 600)
        .expect("render");
    assert_eq!(
        (&page.jpeg[..3], page.width, page.height),
        (&[0xFF, 0xD8, 0xFF][..], 600, 849)
    );
}

#[test]
fn render_width_is_clamped() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    let page = engine
        .render(&write_pdf(dir.path(), 1), 0, 100_000)
        .expect("render");
    assert_eq!(page.width, nebula_core::pdf::MAX_RENDER_WIDTH);
}

#[test]
fn page_out_of_range_is_reported() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        engine.render(&write_pdf(dir.path(), 2), 5, 600),
        Err(CoreError::PageOutOfRange { index: 5, count: 2 })
    ));
}

#[test]
fn metadata_is_read() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    let meta = engine.metadata(&write_pdf(dir.path(), 1)).expect("meta");
    assert_eq!(
        (meta.title.as_deref(), meta.author.as_deref()),
        (Some("Informe anual"), Some("Ana Autora"))
    );
}

#[test]
fn corrupt_pdf_is_an_archive_error() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("roto.pdf");
    std::fs::write(&path, b"%PDF-1.4 basura").expect("escribir");
    assert!(matches!(engine.info(&path), Err(CoreError::Archive { .. })));
}

#[test]
fn missing_pdf_is_an_io_error() {
    let engine = require_engine!();
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        engine.info(&dir.path().join("no.pdf")),
        Err(CoreError::Io { .. })
    ));
}
