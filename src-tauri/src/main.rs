// Evita la consola adicional en Windows en release. NO ELIMINAR.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    match nebula_reader_lib::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "Nebula Reader no pudo iniciarse");
            std::process::ExitCode::FAILURE
        }
    }
}
