//! Protocolo `nebula://` para servir imágenes a la interfaz sin pasar por IPC.
//!
//! En Windows, WebView2 lo expone como `http://nebula.localhost/…`.
//!
//! Rutas:
//! - `/cover/{id}`: miniatura de portada (se genera bajo demanda y se cachea).
//! - `/page/{id}/{index}`: página de un cómic, tal cual está en el archivo.
//! - `/epub/{id}/{ruta}`: recurso de un EPUB (capítulos, CSS, imágenes, fuentes).
//!   Se sirve con una CSP que prohíbe ejecutar scripts.
//! - `/pdf/{id}/{index}?w={ancho}`: página de un PDF renderizada como JPEG al
//!   ancho pedido (en píxeles físicos).
//!
//! Solo se sirven recursos de libros de la biblioteca, identificados por su id;
//! nunca rutas arbitrarias del disco.

use std::borrow::Cow;

use nebula_core::cover::{cover_path, write_comic_cover, write_cover};
use nebula_core::epub::path as epub_path;
use nebula_core::{
    BookFormat, BookId, ComicArchive, CoreError, CoverStatus, EpubBook, ImageType, OpenDocument,
    metadata,
};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::error::{CommandError, CommandResult, ErrorCode};
use crate::state::AppState;

/// Nombre del esquema.
pub const SCHEME: &str = "nebula";

/// Las URL llevan un parámetro de versión (`?v=`) que cambia si el recurso
/// cambia, así que el navegador puede cachearlas sin revalidar.
const CACHE_IMMUTABLE: &str = "private, max-age=31536000, immutable";

type HttpResponse = Response<Cow<'static, [u8]>>;

/// Contenido de un EPUB: ni scripts, ni plugins, ni formularios.
const EPUB_CSP: &str = "script-src 'none'; object-src 'none'; form-action 'none'";

/// Recurso pedido.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Route {
    Cover(BookId),
    Page(BookId, usize),
    Epub(BookId, String),
    Pdf(BookId, u32, u32),
}

/// Ancho de render por defecto si la URL no lo indica.
const DEFAULT_PDF_WIDTH: u32 = 1200;

/// Punto de entrada registrado en el `Builder`.
// La firma la impone `register_asynchronous_uri_scheme_protocol`.
#[allow(clippy::needless_pass_by_value)]
pub fn handle<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let Some(state) = context.app_handle().try_state::<AppState>() else {
        responder.respond(text(
            StatusCode::SERVICE_UNAVAILABLE,
            "la aplicación aún no está lista",
        ));
        return;
    };
    let state = state.inner().clone();
    let route = parse_route(request.uri().path(), request.uri().query());
    let uri = request.uri().to_string();

    tauri::async_runtime::spawn(async move {
        let response = match route {
            Some(Route::Cover(id)) => serve_cover(&state, id).await,
            Some(Route::Page(id, index)) => {
                state
                    .blocking(move |state| serve_page(state, id, index))
                    .await
            }
            Some(Route::Epub(id, href)) => {
                state
                    .blocking(move |state| serve_epub(state, id, &href))
                    .await
            }
            Some(Route::Pdf(id, index, width)) => {
                state
                    .blocking(move |state| serve_pdf(state, id, index, width))
                    .await
            }
            None => Err(CommandError::new(ErrorCode::NotFound, "ruta desconocida")),
        };
        let response = response.unwrap_or_else(|error| {
            tracing::debug!(%uri, %error, "recurso no servido");
            error_response(&error)
        });
        tracing::debug!(%uri, status = response.status().as_u16(), "nebula://");
        responder.respond(response);
    });
}

fn parse_route(path: &str, query: Option<&str>) -> Option<Route> {
    if let Some(rest) = path.trim_start_matches('/').strip_prefix("pdf/") {
        let (id, index) = rest.trim_end_matches('/').split_once('/')?;
        let width = query
            .into_iter()
            .flat_map(|q| q.split('&'))
            .find_map(|pair| pair.strip_prefix("w="))
            .and_then(|w| w.parse().ok())
            .unwrap_or(DEFAULT_PDF_WIDTH);
        return Some(Route::Pdf(
            BookId::new(id.parse().ok()?),
            index.parse().ok()?,
            width,
        ));
    }
    if let Some(rest) = path.trim_start_matches('/').strip_prefix("epub/") {
        let (id, href) = rest.split_once('/')?;
        let href = epub_path::normalize(&epub_path::percent_decode(href)?)?;
        return Some(Route::Epub(BookId::new(id.parse().ok()?), href));
    }
    let mut parts = path.trim_matches('/').split('/');
    let route = match (parts.next()?, parts.next(), parts.next()) {
        ("cover", Some(id), None) => Route::Cover(BookId::new(id.parse().ok()?)),
        ("page", Some(id), Some(index)) => {
            Route::Page(BookId::new(id.parse().ok()?), index.parse().ok()?)
        }
        _ => return None,
    };
    parts.next().is_none().then_some(route)
}

fn serve_page(state: &AppState, id: BookId, index: usize) -> CommandResult<HttpResponse> {
    let (_, document) = state.document(id)?;
    let mut document = document
        .lock()
        .map_err(|_| CommandError::internal("documento en estado inválido"))?;
    let OpenDocument::Comic(comic) = &mut *document else {
        return Err(CommandError::new(ErrorCode::NotFound, "no es un cómic"));
    };
    let page = comic.read_page(index)?;
    Ok(image(page.image_type, page.bytes))
}

fn serve_pdf(state: &AppState, id: BookId, index: u32, width: u32) -> CommandResult<HttpResponse> {
    let book = state.library_blocking(|lib| lib.get(id))?;
    if book.format != BookFormat::Pdf {
        return Err(CommandError::new(ErrorCode::NotFound, "no es un PDF"));
    }
    let page = state.pdf()?.render(&book.path, index, width)?;
    Ok(image(ImageType::Jpeg, page.jpeg))
}

fn serve_epub(state: &AppState, id: BookId, href: &str) -> CommandResult<HttpResponse> {
    let (_, document) = state.document(id)?;
    let mut document = document
        .lock()
        .map_err(|_| CommandError::internal("documento en estado inválido"))?;
    let OpenDocument::Epub(book) = &mut *document else {
        return Err(CommandError::new(ErrorCode::NotFound, "no es un EPUB"));
    };
    let resource = book.read_resource(href)?;
    let mut response = response(
        StatusCode::OK,
        &resource.media_type,
        CACHE_IMMUTABLE,
        resource.bytes,
    );
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static(EPUB_CSP),
    );
    // La interfaz descarga los capítulos con `fetch` y las fuentes se cargan
    // desde la hoja de estilos: ambos necesitan CORS (el origen de la app es otro).
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        header::HeaderValue::from_static("*"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// Sirve la miniatura; si no existe, la genera (con concurrencia limitada).
async fn serve_cover(state: &AppState, id: BookId) -> CommandResult<HttpResponse> {
    let path = cover_path(state.thumbs_dir(), id);
    if let Some(response) = read_cover_file(&path).await {
        return Ok(response);
    }

    // Se limita cuántas portadas se generan a la vez; la espera es asíncrona.
    let _permit = state
        .cover_permits()
        .acquire()
        .await
        .map_err(|_| CommandError::internal("generador de portadas cerrado"))?;

    state
        .blocking(move |state| {
            // Otra petición pudo generarla mientras se esperaba el permiso.
            if let Ok(bytes) = std::fs::read(&path) {
                return Ok(image(ImageType::Jpeg, bytes));
            }
            generate_cover(state, id, &path)?;
            let bytes = std::fs::read(&path)
                .map_err(|e| CommandError::from(nebula_core::CoreError::io(&path, e)))?;
            Ok(image(ImageType::Jpeg, bytes))
        })
        .await
}

async fn read_cover_file(path: &std::path::Path) -> Option<HttpResponse> {
    let path = path.to_path_buf();
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(path))
        .await
        .ok()?
        .ok()?;
    Some(image(ImageType::Jpeg, bytes))
}

fn generate_cover(state: &AppState, id: BookId, path: &std::path::Path) -> CommandResult<()> {
    let book = state.library_blocking(|lib| lib.get(id))?;
    if book.cover == CoverStatus::Failed {
        return Err(CommandError::new(ErrorCode::NotFound, "sin portada"));
    }

    // Se abre aparte (no en la caché de lectura) para no expulsar el libro que
    // se está leyendo al recorrer la biblioteca.
    let result = match book.format {
        BookFormat::Cbz => ComicArchive::open(&book.path).and_then(|mut archive| {
            let pages = u32::try_from(archive.page_count()).unwrap_or(u32::MAX);
            write_comic_cover(&mut archive, path).map(|()| pages)
        }),
        BookFormat::Epub => epub_cover(state, id, &book.path, path),
        BookFormat::Pdf => pdf_cover(state, id, &book.path, path),
        BookFormat::Cbr | BookFormat::Cb7 => {
            return Err(CommandError::new(ErrorCode::NotFound, "sin portada"));
        }
    };

    match result {
        Ok(units) => {
            state.library_blocking(|lib| {
                lib.set_page_count(id, units)?;
                lib.set_cover_status(id, CoverStatus::Ready)
            })?;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(%id, error = %error, "no se pudo generar la portada");
            // Un archivo inexistente puede reaparecer (p. ej. disco externo), así
            // que solo se descarta definitivamente si el contenido no es válido.
            // Tampoco si falta el motor PDF: no es culpa del archivo.
            if !matches!(
                error,
                nebula_core::CoreError::Io { .. } | nebula_core::CoreError::PdfEngine(_)
            ) {
                state.library_blocking(|lib| lib.set_cover_status(id, CoverStatus::Failed))?;
            }
            Err(error.into())
        }
    }
}

/// Portada de un PDF: la primera página. Completa también título y autor.
fn pdf_cover(
    state: &AppState,
    id: BookId,
    book_path: &std::path::Path,
    destination: &std::path::Path,
) -> Result<u32, CoreError> {
    let engine = state
        .pdf()
        .map_err(|_| CoreError::PdfEngine("no disponible".to_owned()))?;
    let info = engine.info(book_path)?;
    if let Ok(metadata) = engine.metadata(book_path)
        && !metadata.is_empty()
        && let Err(error) = state.library_blocking(|lib| lib.update_metadata(id, &metadata))
    {
        tracing::warn!(%id, %error, "no se pudieron guardar los metadatos");
    }
    let page = engine.render(book_path, 0, nebula_core::cover::COVER_WIDTH)?;
    write_cover(&page.jpeg, destination)?;
    Ok(info.page_count)
}

/// Portada de un EPUB. De paso, completa los metadatos de libros importados
/// antes de que se leyeran (v0.2.0 y anteriores).
fn epub_cover(
    state: &AppState,
    id: BookId,
    book_path: &std::path::Path,
    destination: &std::path::Path,
) -> Result<u32, CoreError> {
    let mut book = EpubBook::open(book_path)?;
    let metadata = metadata::from_epub(&book);
    if !metadata.is_empty()
        && let Err(error) = state.library_blocking(|lib| lib.update_metadata(id, &metadata))
    {
        tracing::warn!(%id, %error, "no se pudieron guardar los metadatos");
    }
    let units = u32::try_from(book.spine().len()).unwrap_or(u32::MAX);
    let cover = book
        .read_cover()?
        .ok_or_else(|| CoreError::Image("el libro no declara portada".to_owned()))?;
    write_cover(&cover, destination)?;
    Ok(units)
}

fn image(image_type: ImageType, bytes: Vec<u8>) -> HttpResponse {
    response(StatusCode::OK, image_type.mime(), CACHE_IMMUTABLE, bytes)
}

fn text(status: StatusCode, message: &str) -> HttpResponse {
    response(
        status,
        "text/plain; charset=utf-8",
        "no-store",
        message.as_bytes().to_vec(),
    )
}

fn error_response(error: &CommandError) -> HttpResponse {
    let status = match error.code() {
        ErrorCode::NotFound | ErrorCode::Unsupported => StatusCode::NOT_FOUND,
        ErrorCode::InvalidFile | ErrorCode::InvalidInput => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::Io | ErrorCode::Database | ErrorCode::Internal => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    text(status, &error.to_string())
}

fn response(
    status: StatusCode,
    content_type: &str,
    cache_control: &str,
    body: Vec<u8>,
) -> HttpResponse {
    let mut response = Response::new(Cow::Owned(body));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    if let Ok(value) = header::HeaderValue::from_str(content_type) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = header::HeaderValue::from_str(cache_control) {
        headers.insert(header::CACHE_CONTROL, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cover_route() {
        assert_eq!(
            parse_route("/cover/12", None),
            Some(Route::Cover(BookId::new(12)))
        );
    }

    #[test]
    fn parses_page_route() {
        assert_eq!(
            parse_route("/page/3/41", None),
            Some(Route::Page(BookId::new(3), 41))
        );
    }

    #[test]
    fn rejects_extra_segments() {
        assert_eq!(parse_route("/page/3/41/x", None), None);
    }

    #[test]
    fn rejects_non_numeric_ids() {
        assert_eq!(parse_route("/cover/../../etc", None), None);
    }

    #[test]
    fn rejects_negative_page() {
        assert_eq!(parse_route("/page/3/-1", None), None);
    }

    #[test]
    fn parses_epub_route_with_encoded_path() {
        assert_eq!(
            parse_route("/epub/7/OEBPS/Text/cap%201.xhtml", None),
            Some(Route::Epub(
                BookId::new(7),
                "OEBPS/Text/cap 1.xhtml".to_owned()
            ))
        );
    }

    #[test]
    fn epub_route_rejects_traversal() {
        assert_eq!(parse_route("/epub/7/../../secreto", None), None);
    }

    #[test]
    fn parses_pdf_route_with_width() {
        assert_eq!(
            parse_route("/pdf/4/12", Some("v=9&w=1600")),
            Some(Route::Pdf(BookId::new(4), 12, 1600))
        );
    }

    #[test]
    fn pdf_route_has_default_width() {
        assert_eq!(
            parse_route("/pdf/4/0", None),
            Some(Route::Pdf(BookId::new(4), 0, DEFAULT_PDF_WIDTH))
        );
    }

    #[test]
    fn rejects_unknown_resource() {
        assert_eq!(parse_route("/file/C:/Windows/win.ini", None), None);
    }
}
