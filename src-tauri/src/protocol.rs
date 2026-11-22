//! Protocolo `nebula://` para servir imágenes a la interfaz sin pasar por IPC.
//!
//! En Windows, WebView2 lo expone como `http://nebula.localhost/…`.
//!
//! Rutas:
//! - `/cover/{id}`: miniatura de portada (se genera bajo demanda y se cachea).
//! - `/page/{id}/{index}`: página de un cómic, tal cual está en el archivo.
//!
//! Solo se sirven recursos de libros de la biblioteca, identificados por su id;
//! nunca rutas arbitrarias del disco.

use std::borrow::Cow;

use nebula_core::cover::{cover_path, write_comic_cover};
use nebula_core::{BookFormat, BookId, ComicArchive, CoverStatus, ImageType};
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

/// Recurso pedido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Cover(BookId),
    Page(BookId, usize),
}

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
    let route = parse_route(request.uri().path());

    tauri::async_runtime::spawn(async move {
        let response = match route {
            Some(Route::Cover(id)) => serve_cover(&state, id).await,
            Some(Route::Page(id, index)) => {
                state
                    .blocking(move |state| serve_page(state, id, index))
                    .await
            }
            None => Err(CommandError::new(ErrorCode::NotFound, "ruta desconocida")),
        };
        responder.respond(response.unwrap_or_else(|error| error_response(&error)));
    });
}

fn parse_route(path: &str) -> Option<Route> {
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
    let (_, archive) = state.comic(id)?;
    let page = archive
        .lock()
        .map_err(|_| CommandError::internal("documento en estado inválido"))?
        .read_page(index)?;
    Ok(image(page.image_type, page.bytes))
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
    if book.format != BookFormat::Cbz || book.cover == CoverStatus::Failed {
        return Err(CommandError::new(ErrorCode::NotFound, "sin portada"));
    }

    // Se abre aparte (no en la caché de lectura) para no expulsar el libro que
    // se está leyendo al recorrer la biblioteca.
    let result = ComicArchive::open(&book.path).and_then(|mut archive| {
        let pages = u32::try_from(archive.page_count()).unwrap_or(u32::MAX);
        write_comic_cover(&mut archive, path).map(|()| pages)
    });

    match result {
        Ok(pages) => {
            state.library_blocking(|lib| {
                lib.set_page_count(id, pages)?;
                lib.set_cover_status(id, CoverStatus::Ready)
            })?;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(%id, error = %error, "no se pudo generar la portada");
            // Un archivo inexistente puede reaparecer (p. ej. disco externo), así
            // que solo se descarta definitivamente si el contenido no es válido.
            if !matches!(error, nebula_core::CoreError::Io { .. }) {
                state.library_blocking(|lib| lib.set_cover_status(id, CoverStatus::Failed))?;
            }
            Err(error.into())
        }
    }
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
            parse_route("/cover/12"),
            Some(Route::Cover(BookId::new(12)))
        );
    }

    #[test]
    fn parses_page_route() {
        assert_eq!(
            parse_route("/page/3/41"),
            Some(Route::Page(BookId::new(3), 41))
        );
    }

    #[test]
    fn rejects_extra_segments() {
        assert_eq!(parse_route("/page/3/41/x"), None);
    }

    #[test]
    fn rejects_non_numeric_ids() {
        assert_eq!(parse_route("/cover/../../etc"), None);
    }

    #[test]
    fn rejects_negative_page() {
        assert_eq!(parse_route("/page/3/-1"), None);
    }

    #[test]
    fn rejects_unknown_resource() {
        assert_eq!(parse_route("/file/C:/Windows/win.ini"), None);
    }
}
