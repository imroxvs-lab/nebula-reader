//! Backend 7z (CB7).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use sevenz_rust2::{ArchiveReader, Error, Password};

use super::RawEntry;
use crate::error::{CoreError, CoreResult};

/// Archivo 7z abierto.
pub(super) struct SevenZ {
    reader: ArchiveReader<File>,
}

impl std::fmt::Debug for SevenZ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SevenZ")
            .field("solid", &self.reader.archive().is_solid)
            .finish_non_exhaustive()
    }
}

impl SevenZ {
    pub(super) fn open(path: &Path) -> CoreResult<(Self, Vec<RawEntry>)> {
        let reader =
            ArchiveReader::open(path, Password::empty()).map_err(|e| sevenz_error(path, e))?;
        let entries = reader
            .archive()
            .files
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| RawEntry {
                ordinal,
                name: entry.name.replace('\\', "/"),
                is_file: !entry.is_directory && entry.has_stream,
                encrypted: false,
            })
            .collect();
        Ok((Self { reader }, entries))
    }

    /// Lee las entradas indicadas (por nombre). En archivos sólidos se hace en
    /// una sola pasada, porque cada lectura descomprime desde el inicio del bloque.
    pub(super) fn read(
        &mut self,
        path: &Path,
        wanted: &[&str],
        limit: u64,
    ) -> CoreResult<HashMap<String, Vec<u8>>> {
        let mut found = HashMap::with_capacity(wanted.len());
        if !self.reader.archive().is_solid {
            for name in wanted {
                let size = self
                    .reader
                    .archive()
                    .files
                    .iter()
                    .find(|entry| entry.name == *name)
                    .map_or(0, |entry| entry.size);
                if size > limit {
                    return Err(CoreError::PageTooLarge { index: 0, limit });
                }
                let bytes = self
                    .reader
                    .read_file(name)
                    .map_err(|e| sevenz_error(path, e))?;
                found.insert((*name).to_owned(), bytes);
            }
            return Ok(found);
        }

        let mut too_large = false;
        let result = self.reader.for_each_entries(|entry, data| {
            if !wanted.contains(&entry.name.as_str()) {
                return Ok(true);
            }
            if entry.size > limit {
                too_large = true;
                return Ok(false);
            }
            let mut bytes = Vec::with_capacity(usize::try_from(entry.size).unwrap_or_default());
            data.take(limit + 1).read_to_end(&mut bytes)?;
            found.insert(entry.name.clone(), bytes);
            Ok(found.len() < wanted.len())
        });
        if too_large {
            return Err(CoreError::PageTooLarge { index: 0, limit });
        }
        result.map_err(|e| sevenz_error(path, e))?;
        Ok(found)
    }
}

fn sevenz_error(path: &Path, error: Error) -> CoreError {
    match error {
        Error::PasswordRequired | Error::MaybeBadPassword(_) => {
            CoreError::Protected(path.to_path_buf())
        }
        other => CoreError::Archive {
            path: path.to_path_buf(),
            reason: other.to_string(),
        },
    }
}
