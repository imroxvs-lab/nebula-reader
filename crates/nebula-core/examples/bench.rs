//! Herramienta de desarrollo: mide las operaciones más pesadas del núcleo.
//!
//! ```text
//! cargo run --release -p nebula-core --example bench -- <cbz> <cbr> <epub>
//! ```
//! Sirve para comparar perfiles de compilación (p. ej. `opt-level`).
#![allow(clippy::print_stdout, clippy::print_stderr)] // Herramienta de consola.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use nebula_core::cover::render_cover;
use nebula_core::{ComicArchive, EpubBook};

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let [cbz, cbr, epub] = args.as_slice() else {
        eprintln!("uso: bench <cbz> <cbr> <epub>");
        return ExitCode::FAILURE;
    };
    match run(cbz, cbr, epub) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cbz: &PathBuf, cbr: &PathBuf, epub: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Portadas: decodificar páginas grandes y reducirlas (lo más costoso en CPU).
    let mut comic = ComicArchive::open(cbz)?;
    let pages = comic.page_count().min(20);
    let start = Instant::now();
    for page in 0..pages {
        render_cover(&comic.read_page(page)?.bytes)?;
    }
    let covers = start.elapsed();
    println!(
        "portadas: {pages} en {covers:?} ({:?}/página)",
        covers / pages as u32
    );

    // Lectura secuencial de un RAR sólido (descompresión).
    let start = Instant::now();
    let mut rar = ComicArchive::open(cbr)?;
    let mut bytes = 0;
    for page in 0..rar.page_count() {
        bytes += rar.read_page(page)?.bytes.len();
    }
    println!(
        "CBR: {} páginas ({} MB) en {:?}",
        rar.page_count(),
        bytes / 1_000_000,
        start.elapsed()
    );

    // Apertura de EPUB (ZIP + XML), repetida.
    let start = Instant::now();
    for _ in 0..200 {
        let book = EpubBook::open(epub)?;
        std::hint::black_box(book.spine().len());
    }
    println!("EPUB: 200 aperturas en {:?}", start.elapsed());
    Ok(())
}
