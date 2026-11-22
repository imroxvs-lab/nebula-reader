// Evita la consola adicional en Windows en release. NO ELIMINAR.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    match nebula_reader_lib::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        // `tracing` se configurará en la tarea 1.5; hasta entonces stderr es el único canal.
        #[allow(clippy::print_stderr)]
        Err(err) => {
            eprintln!("Nebula Reader no pudo iniciarse: {err}");
            std::process::ExitCode::FAILURE
        }
    }
}
