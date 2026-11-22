# Nebula Reader

Lector de libros (EPUB, PDF) y cómics (CBZ, CBR, CB7) para Windows 11, hecho con Rust y Tauri 2.

El plan de desarrollo por fases está en [PLAN.md](PLAN.md).

## Estructura
- `crates/nebula-core`: dominio, formatos y biblioteca (sin Tauri).
- `src-tauri`: capa de aplicación Tauri (comandos e IPC).
- `src`: interfaz con Svelte 5 + TypeScript (SvelteKit 3 en modo SPA).

## Requisitos (Windows)
- Rust stable (MSVC) y Microsoft C++ Build Tools
- Node.js 20 o superior
- WebView2 (viene incluido en Windows 11)

## Desarrollo
```powershell
npm install
npm run tauri dev
```

## Datos de la aplicación
- Biblioteca: `%LOCALAPPDATA%\com.nebulareader.reader\library.db` (SQLite)
- Estado de la ventana: `%APPDATA%\com.nebulareader.reader\.window-state.json`

## Herramientas de desarrollo
```powershell
# Importar rutas en una biblioteca sin abrir la interfaz
cargo run -p nebula-core --example import -- "$env:LOCALAPPDATA\com.nebulareader.reader\library.db" C:\ruta\a\comics

# Medir arranque y memoria de la build release (y guardar una captura)
npm run tauri build -- --no-bundle
powershell -ExecutionPolicy Bypass -File scripts\measure.ps1
```

## Comprobaciones
```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
npm run check
npm run build
```
