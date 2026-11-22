//! Estado compartido de la aplicación.

use std::sync::{Arc, Mutex};

use nebula_core::{CoreResult, Library};

use crate::error::{CommandError, CommandResult};

/// Estado gestionado por Tauri.
///
/// La biblioteca va en `Arc<Mutex<_>>` porque `rusqlite::Connection` no es
/// `Sync` y las operaciones se ejecutan en hilos bloqueantes
/// (`spawn_blocking`), que necesitan poseer una referencia `'static`.
#[derive(Debug, Clone)]
pub struct AppState {
    library: Arc<Mutex<Library>>,
}

impl AppState {
    /// Envuelve una biblioteca abierta.
    pub fn new(library: Library) -> Self {
        Self {
            library: Arc::new(Mutex::new(library)),
        }
    }

    /// Ejecuta `f` con la biblioteca bloqueada en un hilo de E/S, sin bloquear
    /// el runtime asíncrono ni el hilo de la interfaz.
    pub async fn with_library<T, F>(&self, f: F) -> CommandResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Library) -> CoreResult<T> + Send + 'static,
    {
        let library = Arc::clone(&self.library);
        tauri::async_runtime::spawn_blocking(move || {
            let guard = library
                .lock()
                .map_err(|_| CommandError::internal("la biblioteca quedó en un estado inválido"))?;
            f(&guard).map_err(CommandError::from)
        })
        .await?
    }
}
