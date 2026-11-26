//! Apertura de archivos desde fuera de la app: doble clic en un archivo
//! asociado, «Abrir con» o arrastrar sobre el icono.
//!
//! - Primer arranque: las rutas llegan como argumentos y se guardan hasta que
//!   la interfaz las pide (`startup_files`).
//! - Con la app ya abierta: `single-instance` reenvía los argumentos a la
//!   instancia existente, que los emite como evento `open-files`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nebula_core::{Book, BookFormat, Timestamp, scan_paths};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Evento que recibe la interfaz con las rutas a abrir.
pub const OPEN_FILES_EVENT: &str = "open-files";
/// Rutas como máximo por apertura (protege frente a argumentos abusivos).
const MAX_FILES: usize = 100;

/// Archivos recibidos al arrancar, pendientes de abrir.
#[derive(Debug, Default)]
pub struct StartupFiles(Mutex<Vec<PathBuf>>);

impl StartupFiles {
    pub fn new(files: Vec<PathBuf>) -> Self {
        Self(Mutex::new(files))
    }
}

/// Rutas de libros entre los argumentos de la línea de órdenes. Las relativas
/// se resuelven respecto a `cwd`; el primer argumento (el ejecutable) se omite.
pub fn files_from_args<I, S>(args: I, cwd: &Path) -> Vec<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .skip(1)
        .map(|arg| PathBuf::from(arg.as_ref()))
        .filter(|path| BookFormat::from_path(path).is_some())
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .take(MAX_FILES)
        .collect()
}

/// Callback de `single-instance`: trae la ventana al frente y pasa los archivos.
// La firma la impone el plugin.
#[allow(clippy::needless_pass_by_value)]
pub fn on_second_instance<R: Runtime>(app: &AppHandle<R>, args: Vec<String>, cwd: String) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let files = files_from_args(args, Path::new(&cwd));
    if !files.is_empty()
        && let Err(error) = app.emit(OPEN_FILES_EVENT, files)
    {
        tracing::warn!(%error, "no se pudieron reenviar los archivos");
    }
}

/// Archivos recibidos al arrancar (solo la primera vez que se piden).
// Tauri entrega el estado por valor.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn startup_files(files: State<'_, StartupFiles>) -> Vec<PathBuf> {
    files
        .0
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

/// Devuelve el libro de esa ruta, añadiéndolo a la biblioteca si no estaba.
#[tauri::command]
pub async fn book_for_path(state: State<'_, AppState>, path: PathBuf) -> CommandResult<Book> {
    if BookFormat::from_path(&path).is_none() {
        return Err(CommandError::invalid_input(
            "No es un libro ni un cómic compatible.",
        ));
    }
    let path = std::path::absolute(&path)
        .map_err(|e| CommandError::from(nebula_core::CoreError::io(&path, e)))?;
    if let Some(book) = state
        .with_library({
            let path = path.clone();
            move |lib| lib.find_by_path(&path)
        })
        .await?
    {
        return Ok(book);
    }
    let scan = tauri::async_runtime::spawn_blocking({
        let path = path.clone();
        move || scan_paths(&[path])
    })
    .await?;
    if let Some(failure) = scan.failures.first() {
        return Err(CommandError::new(
            crate::error::ErrorCode::Io,
            format!("No se puede abrir el archivo: {}", failure.reason),
        ));
    }
    state
        .with_library(move |lib| {
            lib.add_scanned(scan, Timestamp::now())?;
            lib.find_by_path(&path)?
                .ok_or_else(|| nebula_core::CoreError::InvalidData("no se pudo añadir".to_owned()))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_and_unsupported_args_are_ignored() {
        let files = files_from_args(
            ["nebula-reader.exe", "C:\\a.cbz", "--flag", "notas.txt"],
            Path::new("C:\\"),
        );
        assert_eq!(files, [PathBuf::from("C:\\a.cbz")]);
    }

    #[test]
    fn relative_paths_use_cwd() {
        let files = files_from_args(["app", "Saga.epub"], Path::new("C:\\Libros"));
        assert_eq!(files, [Path::new("C:\\Libros").join("Saga.epub")]);
    }
}
