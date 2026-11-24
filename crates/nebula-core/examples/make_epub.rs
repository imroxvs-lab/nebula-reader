//! Herramienta de desarrollo: genera EPUB de prueba.
//!
//! ```text
//! cargo run -p nebula-core --example make_epub -- <carpeta-de-salida>
//! ```
//!
//! Crea `Prueba EPUB3.epub` (EPUB 3 con nav, portada, CSS, imagen, notas,
//! enlaces internos y externos, y un `<script>` hostil que **no** debe
//! ejecutarse) y `Prueba EPUB2.epub` (EPUB 2 con NCX y serie de Calibre).
#![allow(clippy::print_stdout, clippy::print_stderr)] // Herramienta de consola.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const PARAGRAPH: &str = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho \
tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua, rocín flaco y galgo \
corredor. Una olla de algo más vaca que carnero, salpicón las más noches, duelos y quebrantos los \
sábados, lentejas los viernes, algún palomino de añadidura los domingos, consumían las tres partes \
de su hacienda.";

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("uso: make_epub <carpeta-de-salida>");
        return ExitCode::FAILURE;
    };
    let result = std::fs::create_dir_all(&dir)
        .map_err(Into::into)
        .and_then(|()| write_epub3(&dir.join("Prueba EPUB3.epub")))
        .and_then(|()| write_epub2(&dir.join("Prueba EPUB2.epub")));
    match result {
        Ok(()) => {
            println!("EPUB generados en {}", dir.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

struct Epub {
    zip: zip::ZipWriter<BufWriter<File>>,
}

impl Epub {
    fn create(path: &Path) -> Result<Self> {
        let mut zip = zip::ZipWriter::new(BufWriter::new(File::create(path)?));
        // `mimetype` debe ir primero y sin comprimir.
        zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        zip.write_all(b"application/epub+zip")?;
        Ok(Self { zip })
    }

    fn add(&mut self, name: &str, data: &[u8]) -> Result<()> {
        self.zip.start_file(name, SimpleFileOptions::default())?;
        self.zip.write_all(data)?;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        self.zip.finish()?.flush()?;
        Ok(())
    }
}

fn container(opf: &str) -> String {
    format!(
        r##"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="{opf}" media-type="application/oebps-package+xml"/></rootfiles>
</container>"##
    )
}

fn jpeg(width: u32, height: u32, hue: (u8, u8, u8)) -> Result<Vec<u8>> {
    let image = RgbImage::from_fn(width, height, |x, y| {
        let t = (x + y) as f32 / (width + height) as f32;
        Rgb([
            (f32::from(hue.0) * (1.0 - t * 0.6)) as u8,
            (f32::from(hue.1) * (1.0 - t * 0.6)) as u8,
            (f32::from(hue.2) * (1.0 - t * 0.6)) as u8,
        ])
    });
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 85).encode_image(&image)?;
    Ok(bytes)
}

fn chapter(title: &str, body: &str) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="es">
<head><title>{title}</title><link rel="stylesheet" type="text/css" href="../Styles/libro.css"/></head>
<body>
{body}
</body>
</html>"##
    )
}

fn paragraphs(count: usize, chapter: usize) -> String {
    let mut text = String::new();
    for i in 1..=count {
        let _ = writeln!(text, "<p>[{chapter}.{i}] {PARAGRAPH}</p>");
    }
    text
}

fn write_epub3(path: &Path) -> Result<()> {
    let mut epub = Epub::create(path)?;
    epub.add(
        "META-INF/container.xml",
        container("OEBPS/content.opf").as_bytes(),
    )?;
    epub.add(
        "OEBPS/Styles/libro.css",
        b"body { font-family: serif; } h1 { text-align: center; font-size: 1.8em; margin: 1em 0; }\n\
          p { text-indent: 1.5em; margin: 0 0 0.4em; text-align: justify; }\n\
          .figura { text-align: center; } .nota { font-size: 0.85em; }",
    )?;
    epub.add("OEBPS/Images/portada.jpg", &jpeg(600, 900, (90, 60, 200))?)?;
    epub.add("OEBPS/Images/mapa.jpg", &jpeg(800, 500, (40, 160, 120))?)?;

    let chapters = 6;
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();
    for n in 1..=chapters {
        let mut body = format!(r##"<h1 id="inicio">Capítulo {n}</h1>"##);
        match n {
            1 => {
                body.push_str(
                    r##"<p>Este libro de prueba contiene un <a href="cap3.xhtml#seccion">enlace interno</a>,
                    un <a href="https://example.com">enlace externo</a> y una <a href="notas.xhtml#n1">nota</a>.</p>
                    <script>document.body.innerHTML = '<h1 style="color:red">SCRIPT EJECUTADO</h1>';</script>
                    <img src="../Images/mapa.jpg" alt="x" onerror="document.body.innerHTML='ONERROR EJECUTADO'" style="display:none"/>
                    <p onclick="document.body.innerHTML='ONCLICK EJECUTADO'">Si ves texto rojo «SCRIPT EJECUTADO», el bloqueo ha fallado.</p>"##,
                );
                body.push_str(&paragraphs(18, n));
            }
            2 => {
                body.push_str(&paragraphs(6, n));
                body.push_str(r##"<div class="figura"><img src="../Images/mapa.jpg" alt="Mapa"/><p>Figura 1: mapa.</p></div>"##);
                body.push_str(&paragraphs(20, n));
            }
            3 => {
                body.push_str(&paragraphs(24, n));
                body.push_str(r##"<h2 id="seccion">Sección enlazada</h2>"##);
                body.push_str(&paragraphs(10, n));
            }
            _ => body.push_str(&paragraphs(16 + n * 4, n)),
        }
        epub.add(
            &format!("OEBPS/Text/cap{n}.xhtml"),
            chapter(&format!("Capítulo {n}"), &body).as_bytes(),
        )?;
        let _ = writeln!(
            manifest,
            r##"<item id="cap{n}" href="Text/cap{n}.xhtml" media-type="application/xhtml+xml"/>"##
        );
        let _ = writeln!(spine, r##"<itemref idref="cap{n}"/>"##);
        if n == 3 {
            let _ = writeln!(
                nav,
                r##"<li><a href="Text/cap3.xhtml">Capítulo 3</a><ol><li><a href="Text/cap3.xhtml#seccion">Sección enlazada</a></li></ol></li>"##
            );
        } else {
            let _ = writeln!(
                nav,
                r##"<li><a href="Text/cap{n}.xhtml">Capítulo {n}</a></li>"##
            );
        }
    }
    epub.add(
        "OEBPS/Text/notas.xhtml",
        chapter(
            "Notas",
            r##"<h1>Notas</h1><p class="nota" id="n1">1. Nota de ejemplo. <a href="cap1.xhtml">Volver</a></p>"##,
        )
        .as_bytes(),
    )?;
    epub.add(
        "OEBPS/nav.xhtml",
        format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Índice</title></head>
<body><nav epub:type="toc"><h1>Índice</h1><ol>{nav}<li><a href="Text/notas.xhtml">Notas</a></li></ol></nav></body>
</html>"##
        )
        .as_bytes(),
    )?;
    epub.add(
        "OEBPS/content.opf",
        format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:nebula-prueba-epub3</dc:identifier>
    <dc:title>El ingenioso hidalgo (prueba)</dc:title>
    <dc:creator>Miguel de Cervantes</dc:creator>
    <dc:language>es</dc:language>
    <meta property="belongs-to-collection" id="col">Clásicos de prueba</meta>
    <meta refines="#col" property="group-position">1</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="Styles/libro.css" media-type="text/css"/>
    <item id="portada" href="Images/portada.jpg" media-type="image/jpeg" properties="cover-image"/>
    <item id="mapa" href="Images/mapa.jpg" media-type="image/jpeg"/>
    {manifest}
    <item id="notas" href="Text/notas.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    {spine}
    <itemref idref="notas" linear="no"/>
  </spine>
</package>"##
        )
        .as_bytes(),
    )?;
    epub.finish()
}

fn write_epub2(path: &Path) -> Result<()> {
    let mut epub = Epub::create(path)?;
    epub.add(
        "META-INF/container.xml",
        container("content.opf").as_bytes(),
    )?;
    epub.add("img/cubierta.jpg", &jpeg(500, 750, (200, 120, 40))?)?;
    let mut nav_points = String::new();
    let mut manifest = String::new();
    let mut spine = String::new();
    for n in 1..=3 {
        let body = format!("<h1>Parte {n}</h1>{}", paragraphs(15, n));
        let html = format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Parte {n}</title></head><body>{body}</body></html>"##
        );
        epub.add(&format!("parte{n}.html"), html.as_bytes())?;
        let _ = writeln!(
            manifest,
            r##"<item id="p{n}" href="parte{n}.html" media-type="application/xhtml+xml"/>"##
        );
        let _ = writeln!(spine, r##"<itemref idref="p{n}"/>"##);
        let _ = writeln!(
            nav_points,
            r##"<navPoint id="n{n}" playOrder="{n}"><navLabel><text>Parte {n}</text></navLabel><content src="parte{n}.html"/></navPoint>"##
        );
    }
    epub.add(
        "toc.ncx",
        format!(
            r##"<?xml version="1.0"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><navMap>{nav_points}</navMap></ncx>"##
        )
        .as_bytes(),
    )?;
    epub.add(
        "content.opf",
        format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:identifier id="uid">nebula-prueba-epub2</dc:identifier>
    <dc:title>Viaje de prueba (EPUB 2)</dc:title>
    <dc:creator opf:role="aut">Autora de Prueba</dc:creator>
    <meta name="cover" content="cubierta"/>
    <meta name="calibre:series" content="Saga de prueba"/>
    <meta name="calibre:series_index" content="2"/>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cubierta" href="img/cubierta.jpg" media-type="image/jpeg"/>
    {manifest}
  </manifest>
  <spine toc="ncx">{spine}</spine>
</package>"##
        )
        .as_bytes(),
    )?;
    epub.finish()
}
