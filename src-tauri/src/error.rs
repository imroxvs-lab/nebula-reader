//! Errores devueltos a la interfaz por los comandos IPC.

use nebula_core::CoreError;
use serde::Serialize;

/// Categoría estable del error, para que la interfaz decida cómo reaccionar
/// sin depender del texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// El recurso pedido no existe.
    NotFound,
    /// La entrada enviada por la interfaz no es válida.
    InvalidInput,
    /// Fallo de la base de datos.
    Database,
    /// Fallo de entrada/salida.
    Io,
    /// El archivo está dañado o no se puede leer.
    InvalidFile,
    /// El formato todavía no tiene lector.
    Unsupported,
    /// Error inesperado.
    Internal,
}

/// Error serializado hacia la interfaz como `{ code, message }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    code: ErrorCode,
    message: String,
}

impl CommandError {
    /// Crea un error con código y mensaje legible.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Entrada no válida enviada por la interfaz.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    /// Funcionalidad aún no disponible para este formato.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, message)
    }

    /// Código del error.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Error inesperado; se registra y se muestra con un mensaje genérico.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl From<CoreError> for CommandError {
    fn from(error: CoreError) -> Self {
        let code = match &error {
            CoreError::NotFound(_)
            | CoreError::PageOutOfRange { .. }
            | CoreError::NotFoundInArchive(_) => ErrorCode::NotFound,
            CoreError::InvalidData(_) => ErrorCode::InvalidInput,
            CoreError::Archive { .. }
            | CoreError::NoPages(_)
            | CoreError::PageTooLarge { .. }
            | CoreError::Image(_) => ErrorCode::InvalidFile,
            CoreError::Db(_) | CoreError::SchemaTooNew { .. } => ErrorCode::Database,
            CoreError::Io { .. } => ErrorCode::Io,
            CoreError::UnsupportedFormat { .. }
            | CoreError::UnknownFormat(_)
            | CoreError::NonUnicodePath(_) => ErrorCode::InvalidInput,
            _ => ErrorCode::Internal,
        };
        tracing::warn!(error = %error, "comando fallido");
        Self::new(code, user_message(&error))
    }
}

impl From<tauri::Error> for CommandError {
    fn from(error: tauri::Error) -> Self {
        tracing::error!(error = %error, "error de Tauri");
        Self::internal(error.to_string())
    }
}

/// Mensaje comprensible para el usuario. El detalle técnico (rutas, errores del
/// descompresor) queda en el registro.
fn user_message(error: &CoreError) -> String {
    match error {
        CoreError::Archive { .. } => {
            "El archivo está dañado o no tiene un formato válido.".to_owned()
        }
        CoreError::NoPages(_) => "El archivo no contiene ninguna página de imagen.".to_owned(),
        CoreError::PageTooLarge { .. } => {
            "Una de las páginas es demasiado grande para mostrarla.".to_owned()
        }
        CoreError::Image(_) => "Una imagen del archivo no se puede leer.".to_owned(),
        CoreError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            "No se encuentra el archivo. ¿Se ha movido, borrado o está en un disco desconectado?"
                .to_owned()
        }
        other => other.to_string(),
    }
}

/// Resultado de un comando IPC.
pub type CommandResult<T> = Result<T, CommandError>;

#[cfg(test)]
mod tests {
    use nebula_core::BookId;

    use super::*;

    #[test]
    fn not_found_maps_to_not_found_code() {
        let error = CommandError::from(CoreError::NotFound(BookId::new(1)));
        assert_eq!(error.code, ErrorCode::NotFound);
    }

    #[test]
    fn archive_errors_get_a_friendly_message() {
        let error = CommandError::from(CoreError::Archive {
            path: "C:/x.cbz".into(),
            reason: "invalid Zip archive: Could not find EOCD".to_owned(),
        });
        assert_eq!(
            (error.code, error.message.as_str()),
            (
                ErrorCode::InvalidFile,
                "El archivo está dañado o no tiene un formato válido."
            )
        );
    }

    #[test]
    fn serializes_as_code_and_message() {
        let error = CommandError::invalid_input("mal");
        assert_eq!(
            serde_json::to_string(&error).ok().as_deref(),
            Some(r#"{"code":"invalidInput","message":"mal"}"#)
        );
    }
}
