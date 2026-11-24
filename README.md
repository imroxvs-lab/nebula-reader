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
# Motor PDF (una vez; descarga pdfium.dll con versión y hash fijados)
powershell -ExecutionPolicy Bypass -File scripts\fetch-pdfium.ps1
npm install
npm run tauri dev
```

## Datos de la aplicación
- Biblioteca: `%LOCALAPPDATA%\com.nebulareader.reader\library.db` (SQLite)
- Miniaturas de portada: `%LOCALAPPDATA%\com.nebulareader.reader\thumbs\<id>.jpg`
- Estado de la ventana: `%APPDATA%\com.nebulareader.reader\.window-state.json`

## Herramientas de desarrollo
```powershell
# Importar rutas en una biblioteca sin abrir la interfaz
cargo run -p nebula-core --example import -- "$env:LOCALAPPDATA\com.nebulareader.reader\library.db" C:\ruta\a\comics

# Generar un cómic de prueba (220 páginas de 1600×2400, numeradas)
cargo run --release -p nebula-core --example make_comic -- C:\ruta\prueba.cbz 220 1600 2400

# Generar EPUB de prueba (EPUB 3 con contenido hostil y EPUB 2)
cargo run -p nebula-core --example make_epub -- C:\ruta\epub

# Generar un PDF de prueba (500 páginas numeradas)
cargo run -p nebula-core --example make_pdf -- C:\ruta\prueba.pdf 500

# Tests con PDFium real
$env:NEBULA_PDFIUM = "src-tauri\resources\pdfium\pdfium.dll"
cargo test -p nebula-core --features pdf --test pdf

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

## Licencias de terceros
- PDFium (`pdfium.dll`): BSD-3-Clause / Apache-2.0, binarios de [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries). Se instala junto con su licencia en `pdfium\LICENSE`.

## Licencia
[MIT](LICENSE) © 2026 Roxvs
