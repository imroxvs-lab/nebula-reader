//! Backend RAR (CBR) sobre la biblioteca oficial UnRAR.
//!
//! RAR solo admite lectura secuencial: para leer una entrada hay que recorrer
//! las cabeceras desde el principio (y, en archivos sólidos, descomprimir todo
//! lo anterior). Por eso [`read`] lee varias entradas de una sola pasada.

use std::collections::HashMap;
use std::path::Path;

use unrar::Archive;
use unrar::error::{Code, UnrarError};

use super::RawEntry;
use crate::error::{CoreError, CoreResult};

/// Lista las entradas en el orden en que aparecen en el archivo.
pub(super) fn list(path: &Path) -> CoreResult<Vec<RawEntry>> {
    let listing = Archive::new(path)
        .open_for_listing()
        .map_err(|e| rar_error(path, &e))?;
    let mut entries = Vec::new();
    for (ordinal, header) in listing.enumerate() {
        let header = header.map_err(|e| rar_error(path, &e))?;
        entries.push(RawEntry {
            ordinal,
            name: header.filename.to_string_lossy().replace('\\', "/"),
            is_file: header.is_file(),
            encrypted: header.is_encrypted(),
        });
    }
    Ok(entries)
}

/// Lee las entradas indicadas (por ordinal) en una sola pasada.
///
/// Devuelve los bytes de cada una; se detiene en cuanto las tiene todas.
pub(super) fn read(
    path: &Path,
    wanted: &[usize],
    limit: u64,
) -> CoreResult<HashMap<usize, Vec<u8>>> {
    let mut found = HashMap::with_capacity(wanted.len());
    let mut archive = Archive::new(path)
        .open_for_processing()
        .map_err(|e| rar_error(path, &e))?;
    let mut ordinal = 0usize;
    while found.len() < wanted.len() {
        let Some(header) = archive.read_header().map_err(|e| rar_error(path, &e))? else {
            break;
        };
        archive = if wanted.contains(&ordinal) {
            let size = header.entry().unpacked_size;
            if size > limit {
                return Err(CoreError::PageTooLarge {
                    index: ordinal,
                    limit,
                });
            }
            let (bytes, next) = header.read().map_err(|e| rar_error(path, &e))?;
            found.insert(ordinal, bytes);
            next
        } else {
            header.skip().map_err(|e| rar_error(path, &e))?
        };
        ordinal += 1;
    }
    Ok(found)
}

fn rar_error(path: &Path, error: &UnrarError) -> CoreError {
    match error.code {
        Code::MissingPassword | Code::BadPassword => CoreError::Protected(path.to_path_buf()),
        Code::EOpen if !path.exists() => CoreError::io(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "no existe"),
        ),
        _ => CoreError::Archive {
            path: path.to_path_buf(),
            reason: error.to_string(),
        },
    }
}
