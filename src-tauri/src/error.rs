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

    /// Error inesperado; se registra y se muestra con un mensaje genérico.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl From<CoreError> for CommandError {
    fn from(error: CoreError) -> Self {
        let code = match &error {
            CoreError::NotFound(_) => ErrorCode::NotFound,
            CoreError::Db(_) | CoreError::SchemaTooNew { .. } => ErrorCode::Database,
            CoreError::Io { .. } => ErrorCode::Io,
            CoreError::UnsupportedFormat { .. }
            | CoreError::UnknownFormat(_)
            | CoreError::NonUnicodePath(_) => ErrorCode::InvalidInput,
            _ => ErrorCode::Internal,
        };
        tracing::warn!(error = %error, "comando fallido");
        Self::new(code, error.to_string())
    }
}

impl From<tauri::Error> for CommandError {
    fn from(error: tauri::Error) -> Self {
        tracing::error!(error = %error, "error de Tauri");
        Self::internal(error.to_string())
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
    fn serializes_as_code_and_message() {
        let error = CommandError::invalid_input("mal");
        assert_eq!(
            serde_json::to_string(&error).ok().as_deref(),
            Some(r#"{"code":"invalidInput","message":"mal"}"#)
        );
    }
}
