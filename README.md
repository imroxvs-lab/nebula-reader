# Nebula Reader

Lector de libros (EPUB, PDF) y cómics (CBZ, CBR, CB7) para Windows 11, hecho con Rust y Tauri 2.

El plan de desarrollo por fases está en [PLAN.md](PLAN.md).

## Estructura
- `crates/nebula-core`: dominio, formatos y biblioteca (sin Tauri).
- `src-tauri`: capa de aplicación Tauri (comandos e IPC).
- `src`: interfaz con Svelte 5 + TypeScript (SvelteKit en modo SPA).

## Requisitos (Windows)
- Rust stable (MSVC) y Microsoft C++ Build Tools
- Node.js 20 o superior
- WebView2 (viene incluido en Windows 11)

## Desarrollo
```powershell
npm install
npm run tauri dev
```

## Comprobaciones
```powershell
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
npm run check
npm run build
```
