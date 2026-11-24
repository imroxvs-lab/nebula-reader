//! Herramienta de desarrollo: genera un CBZ de prueba con páginas JPEG numeradas.
//!
//! ```text
//! cargo run --release -p nebula-core --example make_comic -- <salida> [páginas] [ancho] [alto] [apaisadas]
//! ```
//!
//! - `<salida>` termina en `.cbz` (ZIP), `.cb7` (7z) o es una carpeta (imágenes
//!   sueltas, p. ej. para empaquetarlas después con WinRAR como CBR).
//! - `[apaisadas]`: lista de páginas separadas por comas que se generan con
//!   ancho y alto intercambiados (dobles páginas escaneadas), p. ej. `7,12`.
//!
//! Cada página tiene un color distinto, textura de ruido (para que el tamaño del
//! JPEG sea realista) y su número dibujado con dígitos de siete segmentos. Los
//! nombres de archivo no llevan ceros a la izquierda (`p1.jpg`, `p10.jpg`…) para
//! comprobar el orden natural.
#![allow(clippy::print_stdout, clippy::print_stderr)] // Herramienta de consola.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use zip::write::SimpleFileOptions;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("uso: make_comic <salida.cbz> [páginas=220] [ancho=1600] [alto=2400]");
        return ExitCode::FAILURE;
    };
    let mut number = |default: u32| {
        args.next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let (pages, width, height) = (number(220), number(1600), number(2400));
    let wide: Vec<u32> = args
        .next()
        .map(|list| {
            list.split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default();

    match write_comic(&output, pages, width, height, &wide) {
        Ok(bytes) => {
            println!(
                "{} ({pages} páginas, {} MB)",
                output.display(),
                bytes / 1_000_000
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn write_comic(
    output: &PathBuf,
    pages: u32,
    width: u32,
    height: u32,
    wide: &[u32],
) -> Result<u64, Box<dyn std::error::Error>> {
    let jpeg_of = |page: u32| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let (w, h) = if wide.contains(&page) {
            (height, width)
        } else {
            (width, height)
        };
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 85).encode_image(&render_page(page, w, h))?;
        Ok(jpeg)
    };
    let extension = output
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("cbz") => {
            let mut zip = zip::ZipWriter::new(BufWriter::new(File::create(output)?));
            // Los JPEG ya están comprimidos: se guardan sin volver a comprimir,
            // como hacen la mayoría de los CBZ reales.
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for page in 1..=pages {
                zip.start_file(format!("p{page}.jpg"), options)?;
                zip.write_all(&jpeg_of(page)?)?;
            }
            zip.finish()?.flush()?;
        }
        Some("cb7") => {
            let mut writer = sevenz_rust2::ArchiveWriter::new(File::create(output)?)?;
            for page in 1..=pages {
                writer.push_archive_entry(
                    sevenz_rust2::ArchiveEntry::new_file(&format!("p{page}.jpg")),
                    Some(std::io::Cursor::new(jpeg_of(page)?)),
                )?;
            }
            writer.finish()?;
        }
        _ => {
            std::fs::create_dir_all(output)?;
            let mut total = 0;
            for page in 1..=pages {
                let jpeg = jpeg_of(page)?;
                total += jpeg.len() as u64;
                std::fs::write(output.join(format!("p{page}.jpg")), jpeg)?;
            }
            return Ok(total);
        }
    }
    Ok(std::fs::metadata(output)?.len())
}

fn render_page(page: u32, width: u32, height: u32) -> RgbImage {
    let hue = (page * 37 % 360) as f32;
    let mut seed = page.wrapping_mul(2_654_435_761);
    let mut image = RgbImage::from_fn(width, height, |x, y| {
        // Ruido barato (xorshift) para simular el detalle de una ilustración.
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed % 48) as f32 - 24.0;
        let light = 0.35 + 0.35 * (y as f32 / height as f32) + 0.1 * (x as f32 / width as f32);
        let [r, g, b] = hsl_to_rgb(hue, 0.55, light);
        Rgb([clamp(r + noise), clamp(g + noise), clamp(b + noise)])
    });
    draw_number(&mut image, page);
    image
}

fn clamp(value: f32) -> u8 {
    value.clamp(0.0, 255.0) as u8
}

fn hsl_to_rgb(hue: f32, saturation: f32, light: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * light - 1.0).abs()) * saturation;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = light - c / 2.0;
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0]
}

/// Segmentos encendidos (a, b, c, d, e, f, g) de cada dígito.
const SEGMENTS: [[bool; 7]; 10] = [
    [true, true, true, true, true, true, false],
    [false, true, true, false, false, false, false],
    [true, true, false, true, true, false, true],
    [true, true, true, true, false, false, true],
    [false, true, true, false, false, true, true],
    [true, false, true, true, false, true, true],
    [true, false, true, true, true, true, true],
    [true, true, true, false, false, false, false],
    [true, true, true, true, true, true, true],
    [true, true, true, true, false, true, true],
];

fn draw_number(image: &mut RgbImage, number: u32) {
    let digit_width = image.width() / 6;
    let digit_height = digit_width * 2;
    let thickness = (digit_width / 6).max(2);
    let digits: Vec<u32> = number
        .to_string()
        .chars()
        .filter_map(|c| c.to_digit(10))
        .collect();
    let total = digits.len() as u32 * (digit_width + thickness * 2);
    let mut left = image.width().saturating_sub(total) / 2;
    let top = image.height().saturating_sub(digit_height) / 2;
    for digit in digits {
        draw_digit(
            image,
            digit,
            left,
            top,
            digit_width,
            digit_height,
            thickness,
        );
        left += digit_width + thickness * 2;
    }
}

fn draw_digit(image: &mut RgbImage, digit: u32, x: u32, y: u32, w: u32, h: u32, t: u32) {
    let half = h / 2;
    let rects = [
        (x, y, w, t),                   // a
        (x + w - t, y, t, half),        // b
        (x + w - t, y + half, t, half), // c
        (x, y + h - t, w, t),           // d
        (x, y + half, t, half),         // e
        (x, y, t, half),                // f
        (x, y + half - t / 2, w, t),    // g
    ];
    let Some(segments) = SEGMENTS.get(digit as usize) else {
        return;
    };
    for (on, (rx, ry, rw, rh)) in segments.iter().zip(rects) {
        if !on {
            continue;
        }
        for py in ry..(ry + rh).min(image.height()) {
            for px in rx..(rx + rw).min(image.width()) {
                image.put_pixel(px, py, Rgb([250, 250, 250]));
            }
        }
    }
}
