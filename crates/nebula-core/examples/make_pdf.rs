//! Herramienta de desarrollo: genera un PDF de prueba con páginas numeradas.
//!
//! ```text
//! cargo run -p nebula-core --example make_pdf -- <salida.pdf> [páginas=500]
//! ```
#![allow(clippy::print_stdout, clippy::print_stderr)] // Herramienta de consola.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("uso: make_pdf <salida.pdf> [páginas]");
        return ExitCode::FAILURE;
    };
    let pages = args.next().and_then(|v| v.parse().ok()).unwrap_or(500u32);
    match std::fs::write(
        &output,
        build_pdf(
            pages,
            &format!("Documento de prueba ({pages} paginas)"),
            "Nebula Reader",
        ),
    ) {
        Ok(()) => {
            println!("{} ({pages} páginas)", output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// PDF 1.4 mínimo y válido: A4, texto Helvetica, tabla xref correcta.
fn build_pdf(pages: u32, title: &str, author: &str) -> Vec<u8> {
    // Objetos: 1 catálogo, 2 páginas, 3 fuente, 4 info, luego (página, contenido) por página.
    let mut objects: Vec<String> = Vec::new();
    let kids: String = (0..pages).map(|i| format!("{} 0 R ", 5 + i * 2)).collect();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
    objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>"));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned());
    objects.push(format!("<< /Title ({title}) /Author ({author}) >>"));
    for page in 1..=pages {
        let content_id = 5 + (page - 1) * 2 + 1;
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>"
        ));
        let mut stream = format!(
            "BT /F1 96 Tf 230 420 Td ({page}) Tj ET\nBT /F1 14 Tf 72 780 Td (Pagina {page} de {pages}) Tj ET\n"
        );
        for line in 0..30 {
            let _ = writeln!(
                stream,
                "BT /F1 11 Tf 72 {} Td (Linea {line} de texto de relleno para la pagina {page}.) Tj ET",
                740 - line * 12
            );
        }
        objects.push(format!(
            "<< /Length {} >>\nstream\n{stream}endstream",
            stream.len()
        ));
    }

    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(table, "{offset:010} 00000 n ");
    }
    let _ = write!(
        table,
        "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    out.extend_from_slice(table.as_bytes());
    out
}
