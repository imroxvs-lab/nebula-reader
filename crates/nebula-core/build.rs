//! `unrar_sys` usa funciones del registro de Windows (`RegOpenKeyExW`…) pero no
//! declara `advapi32`; los binarios de test no la enlazan por defecto.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
