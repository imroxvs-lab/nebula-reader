//! Herramienta de desarrollo: importa rutas en una biblioteca SQLite.
//!
//! ```text
//! cargo run -p nebula-core --example import -- <library.db> <ruta>...
//! ```
#![allow(clippy::print_stdout, clippy::print_stderr)] // Herramienta de consola.

use std::path::PathBuf;
use std::process::ExitCode;

use nebula_core::{Library, Timestamp};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let (Some(db), paths) = (args.next(), args.collect::<Vec<_>>()) else {
        eprintln!("uso: import <library.db> <ruta>...");
        return ExitCode::FAILURE;
    };
    if paths.is_empty() {
        eprintln!("uso: import <library.db> <ruta>...");
        return ExitCode::FAILURE;
    }
    let result = Library::open(&db).and_then(|lib| lib.import_paths(&paths, Timestamp::now()));
    match result {
        Ok(report) => {
            println!("{report:#?}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
