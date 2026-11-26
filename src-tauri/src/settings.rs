//! Ajustes que la interfaz puede leer y escribir, con validación por clave.
//!
//! Solo se aceptan claves conocidas y valores con la forma esperada: así un
//! fallo (o un abuso) de la interfaz no puede guardar datos arbitrarios.

use crate::error::{CommandError, CommandResult};

/// Clave de las carpetas de la biblioteca (lista JSON de rutas).
pub const LIBRARY_FOLDERS: &str = "libraryFolders";
/// Número máximo de carpetas de la biblioteca.
const MAX_FOLDERS: usize = 50;
/// Longitud máxima de un valor.
const MAX_LEN: usize = 16 * 1024;

/// Valida un ajuste antes de guardarlo.
///
/// # Errors
/// [`CommandError`] con código `invalidInput` si la clave no existe o el valor
/// no tiene el formato esperado.
pub fn validate(key: &str, value: &str) -> CommandResult<()> {
    if value.len() > MAX_LEN {
        return Err(CommandError::invalid_input(
            "valor de ajuste demasiado largo",
        ));
    }
    let one_of = |options: &[&str]| options.contains(&value);
    let valid = match key {
        "theme" => one_of(&["system", "light", "dark"]),
        "accent" => one_of(&["violet", "blue", "teal", "green", "orange", "pink"]),
        "readerFit" => one_of(&["page", "width", "original"]),
        "defaultLayout" => one_of(&["single", "double", "webtoon"]),
        "defaultDirection" => one_of(&["ltr", "rtl"]),
        "rescanOnStart" => one_of(&["true", "false"]),
        "epubStyle" => {
            value.len() <= 1024
                && matches!(
                    serde_json::from_str::<serde_json::Value>(value),
                    Ok(serde_json::Value::Object(_))
                )
        }
        LIBRARY_FOLDERS => parse_folders(value).is_some(),
        _ => {
            return Err(CommandError::invalid_input(format!(
                "ajuste desconocido: {key}"
            )));
        }
    };
    if valid {
        Ok(())
    } else {
        Err(CommandError::invalid_input(format!(
            "valor no válido para {key}"
        )))
    }
}

/// Comprueba que una clave existe (para leerla).
///
/// # Errors
/// `invalidInput` si la clave no existe.
pub fn validate_key(key: &str) -> CommandResult<()> {
    match key {
        "theme" | "accent" | "readerFit" | "defaultLayout" | "defaultDirection"
        | "rescanOnStart" | "epubStyle" | LIBRARY_FOLDERS => Ok(()),
        _ => Err(CommandError::invalid_input(format!(
            "ajuste desconocido: {key}"
        ))),
    }
}

/// Interpreta la lista de carpetas; `None` si no es una lista válida de rutas absolutas.
pub fn parse_folders(value: &str) -> Option<Vec<std::path::PathBuf>> {
    let folders: Vec<String> = serde_json::from_str(value).ok()?;
    let valid = folders.len() <= MAX_FOLDERS
        && folders
            .iter()
            .all(|f| !f.is_empty() && f.len() <= 1024 && std::path::Path::new(f).is_absolute());
    valid.then(|| folders.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_are_accepted() {
        assert!(
            validate("theme", "dark").is_ok()
                && validate("accent", "teal").is_ok()
                && validate("epubStyle", r#"{"theme":"sepia"}"#).is_ok()
                && validate(LIBRARY_FOLDERS, r#"["C:\\Comics"]"#).is_ok()
        );
    }

    #[test]
    fn unknown_key_is_rejected() {
        assert!(validate("../../etc", "x").is_err() && validate_key("x").is_err());
    }

    #[test]
    fn wrong_value_is_rejected() {
        assert!(validate("theme", "purple").is_err() && validate("epubStyle", "[1]").is_err());
    }

    #[test]
    fn relative_folders_are_rejected() {
        assert!(validate(LIBRARY_FOLDERS, r#"["Comics"]"#).is_err());
    }
}
