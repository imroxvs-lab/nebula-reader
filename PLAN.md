# Nebula Reader — Plan de desarrollo

Lector de libros y cómics electrónicos para Windows 11, en Rust + Tauri 2.
Objetivo: diseño moderno (estilo Windows 11, Mica), bajo consumo de RAM y recursos.

Objetivos de recursos (medidos en release, memoria privada total = app + procesos WebView2):
- Reposo: no superar en más de ~30 MB la línea base de v0.1.0 (~152 MB, que corresponde casi entera a WebView2; el proceso Rust usa ~5 MB).
- Lectura: crecimiento acotado y estable al pasar páginas (sin fugas).
- Instalador NSIS: < 10 MB sin pdfium (v0.1.0: 1,12 MiB).

## Cómo usar este plan
Flujo de trabajo **por fases** (vigente desde la tarea 1.4):
1. El asistente implementa todas las tareas pendientes de la fase.
2. Cada tarea se marca `[x]` cuando está implementada y verificada por el asistente:
   - `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`,
     `cargo test --workspace`, `npm run check` y `npm run build` en verde;
   - si afecta a la interfaz: compilación release, apertura de la app, capturas y medición con
     `scripts/measure.ps1`.
3. Al terminar la fase, el asistente avisa con un informe: cambios, checks ejecutados, lo no
   verificado y los pasos exactos para que el usuario lo pruebe.
4. El usuario revisa la fase completa y pide cambios si hace falta. Si una tarea marcada no le
   convence, vuelve a `[ ]` con una nota de lo que hay que corregir.
5. La fase se marca ✅ y se crea su tag **solo** cuando el usuario la aprueba; entonces se pasa a la siguiente.
- Compilar/ejecutar la app con herramientas de **Windows** (`cargo.exe`, `npm`); `nebula-core` también se puede testear desde WSL.
- Las tareas 1.1–1.3 se hicieron con el flujo anterior (aprobación del usuario tarea a tarea).

## Arquitectura (decisiones fijas)
- Workspace Cargo: `crates/nebula-core` (lib sin Tauri: dominio, formatos, SQLite) + `src-tauri` (bin fino).
- Frontend: Svelte 5 + TypeScript + Vite con **SvelteKit 3** en modo SPA (`adapter-static` 4, `ssr = false`). En SvelteKit 3 no existe `svelte.config.js`: la configuración se pasa a `sveltekit({...})` en `vite.config.js`, y `tsconfig.json` extiende `$app/tsconfig`.
- Bytes de páginas y recursos servidos por el protocolo `nebula://` (nunca base64 por IPC).
- Despacho por enum (`OpenDocument`, `ArchiveBackend`), no `dyn`.
- SQLite con `rusqlite` (`bundled`) y migraciones numeradas vía `PRAGMA user_version`.
- Errores: `thiserror` en core, `CommandError` serializable en Tauri. `tracing` para logs. `#![forbid(unsafe_code)]` en core.
- Seguridad: capabilities mínimas, CSP estricta, toda la E/S en Rust.

---

## Fase 1 — Base, pantalla de inicio y SQLite (v0.1.0)

### Tareas
- [x] 1.1 Scaffold con `create-tauri-app` (SvelteKit 3 + TS) desde Windows; convertir a workspace; crear `crates/nebula-core`; lints, `[profile.release]` (lto, codegen-units=1, opt-level="s", strip, panic=abort), `.gitignore`, `git init`
- [x] 1.2 Dominio y errores: `BookId`, `BookFormat` (detección por extensión), `BookKind`, `Book`, `NewBook`, `SortBy`, `LibraryQuery`, `ImportReport`, `CoreError` + tests
- [x] 1.3 SQLite: `Library` (open / open_in_memory), migración 1 (`books`, `settings`, WAL, foreign_keys), CRUD, búsqueda/filtro/orden, settings + tests
- [ ] 1.4 Importación de archivos/carpetas (`walkdir`), filtro de formatos, duplicados, título desde nombre de archivo, `ImportReport` + tests
- [ ] 1.5 Capa Tauri: `AppState`, comandos `library_list/import/remove`, `settings_get/set`, `CommandError`, BD en AppData, capabilities y CSP, plugins dialog y window-state
- [ ] 1.6 Shell UI: TitleBar propia, Mica, Sidebar, tema claro/oscuro/sistema persistido
- [ ] 1.7 Pantalla de inicio y biblioteca: HomeView (recientes, estado vacío), LibraryView (búsqueda, orden, filtro), BookCard con portada provisional
- [ ] 1.8 Importación desde UI (diálogo de archivos/carpeta, drag&drop), toasts, quitar libro

### Validación
- [ ] Checks automáticos en verde
- [ ] Importar archivos mixtos y carpeta; los no soportados se informan
- [ ] Cerrar y reabrir: biblioteca, tema y ventana persisten
- [ ] Duplicados ignorados; búsqueda/orden/filtro correctos
- [ ] Línea base medida en release (RAM en reposo, tiempo de arranque) y anotada abajo
- [ ] Tag `v0.1.0`

## Fase 2 — Lector CBZ, portadas y progreso (v0.2.0)

### Tareas
- [ ] 2.1 `ComicArchive` con backend zip, orden natural, filtro de imágenes, límites anti zip-bomb + tests (proptest)
- [ ] 2.2 Miniaturas: portada desde la primera página, redimensionado con `image`, caché en `AppData/thumbs`
- [ ] 2.3 Migración 2: `page_count`, `cover_path`, tabla `progress`
- [ ] 2.4 Protocolo `nebula://` (páginas y miniaturas), comandos `book_open`, `progress_save`, LRU de documentos
- [ ] 2.5 UI: portadas reales lazy, `ComicReader` (encaje, zoom, teclado/rueda, precarga ±2), barra de progreso
- [ ] 2.6 "Continuar leyendo" en inicio y `last_opened_at`

### Validación
- [ ] Checks automáticos en verde
- [ ] Cómic de 200+ páginas fluido; progreso se restaura al reabrir
- [ ] Zip corrupto y rutas `..` manejados sin fallo
- [ ] RAM en lectura medida y anotada
- [ ] Tag `v0.2.0`

## Fase 3 — EPUB (v0.3.0)

### Tareas
- [ ] 3.1 Parser EPUB (container.xml → OPF → spine, TOC nav/NCX, metadatos, portada) con normalización segura de rutas + tests
- [ ] 3.2 Migración 3: `author`, `series`; importación con metadatos reales
- [ ] 3.3 Recursos EPUB por `nebula://` con CSP que bloquea scripts
- [ ] 3.4 `EpubReader`: iframe sandbox, paginación CSS columns, TOC, progreso
- [ ] 3.5 Ajustes tipográficos: fuente, tamaño, interlineado, márgenes, tema papel/sepia/oscuro

### Validación
- [ ] Checks automáticos en verde
- [ ] EPUB 2 y EPUB 3 correctos; EPUB malformado da un error amigable
- [ ] `<script>` dentro de un EPUB no se ejecuta
- [ ] Tag `v0.3.0`

## Fase 4 — PDF (v0.4.0)

### Tareas
- [ ] 4.1 `pdf.rs` con `pdfium-render` tras feature `pdf`; render bajo demanda a la anchura visible
- [ ] 4.2 Empaquetar `pdfium.dll`; si falta, PDF se deshabilita con mensaje claro
- [ ] 4.3 `PdfReader` reutilizando los controles del lector de cómics; zoom con re-render; portada desde la página 1

### Validación
- [ ] Checks automáticos en verde
- [ ] PDF de 500+ páginas con RAM estable al pasar páginas
- [ ] Tag `v0.4.0`

## Fase 5 — CBR, CB7 y modos de cómic avanzados (v0.5.0)

### Tareas
- [ ] 5.1 Backends `unrar` y `sevenz-rust2` en `ArchiveBackend`, índice al abrir y caché de páginas
- [ ] 5.2 Doble página
- [ ] 5.3 Lectura RTL (manga)
- [ ] 5.4 Scroll vertical continuo (webtoon) y opciones por libro

### Validación
- [ ] Checks automáticos en verde
- [ ] CBR/CB7 reales; archivos protegidos o corruptos dan un error amigable
- [ ] Tag `v0.5.0`

## Fase 6 — Organización y ajustes (v0.6.0)

### Tareas
- [ ] 6.1 Migración: `bookmarks`, `collections`, `collection_books`, estado leído/pendiente
- [ ] 6.2 Pantalla de Ajustes (tema, acento, comportamiento del lector, carpetas vigiladas opcionales)
- [ ] 6.3 Marcadores y colecciones/estanterías en la UI
- [ ] 6.4 Filtros por estado/autor/serie; reescaneo y detección de archivos movidos/eliminados
- [ ] 6.5 Accesibilidad: navegación por teclado, foco visible, contraste

### Validación
- [ ] Checks automáticos en verde
- [ ] Migraciones aplicadas sobre una BD de v0.5.0 sin pérdida de datos
- [ ] Tag `v0.6.0`

## Fase 7 — Distribución y rendimiento (v1.0.0)

### Tareas
- [ ] 7.1 Instalador NSIS e icono
- [ ] 7.2 Asociaciones de archivo (.cbz/.cbr/.cb7/.epub/.pdf), apertura por argumento, `single-instance`
- [ ] 7.3 `THIRD_PARTY_LICENSES` (unrar, pdfium, etc.)
- [ ] 7.4 Medición en release y ajuste (opt-level, tamaños de LRU, resolución de render, virtualización del grid si hace falta)
- [ ] 7.5 (Opcional) Actualizaciones automáticas con `tauri-plugin-updater`

### Validación
- [ ] Instalación limpia en Windows 11; doble clic en un archivo asociado lo abre en la instancia existente
- [ ] Métricas finales comparadas con la línea base
- [ ] Tag `v1.0.0`

---

## Métricas de rendimiento
| Versión | RAM reposo | RAM lectura | Arranque | Tamaño instalador |
|---------|-----------|-------------|----------|-------------------|
| v0.1.0 (scaffold 1.1, preliminar) | 152 MB privada (app 5 MB + WebView2 147 MB); 337 MB working set | — | ~250–380 ms hasta ventana visible | 1,12 MiB |

## Registro de cambios del plan
- 1.1: la plantilla oficial `create-tauri-app` (svelte-ts) usa SvelteKit en modo SPA (`adapter-static`, sin SSR) en vez de Svelte + Vite puro. Se mantiene por ser la opción recomendada por Tauri; el resultado sigue siendo HTML/JS estático.
- 1.1: `npm audit` informaba de vulnerabilidades en `@sveltejs/kit` 2.x y `cookie`. Por decisión del usuario se migra a **SvelteKit 3.4** + `adapter-static` 4. Se elimina `svelte.config.js`, se mueve la configuración a `vite.config.js` y `tsconfig.json` pasa a extender `$app/tsconfig`. Ahora `npm audit` da 0 vulnerabilidades.
- 1.1: el objetivo inicial de "< 120 MB de RAM" no es realista con WebView2: solo el runtime ya consume ~147 MB de memoria privada con una página vacía. Se sustituye por objetivos relativos a la línea base (ver arriba).
- 1.1: se añade la verificación automática de la interfaz (captura de pantalla + medición de RAM/arranque) antes de pedir revisión. Gracias a ella se detectó y corrigió una barra de scroll espuria en la página inicial.
- 1.2: además de lo previsto, se añaden `Timestamp` (segundos Unix) e `ImportFailure` (ruta + motivo), que es lo que guarda la lista `failures` de `ImportReport`. La variante `CoreError::Db` se pospone a la 1.3, cuando entra `rusqlite`. La detección de formato por magic bytes se aplaza a los lectores de cada formato (fases 2–5): para importar basta con la extensión.
- 1.3: se usa `rusqlite` 0.43, que exige Rust 1.89, así que el MSRV del workspace sube de 1.85 a 1.89 (el valor anterior era arbitrario; se compila con stable). `tracing` entra ya en `nebula-core` para registrar las migraciones.
- 1.3: `books.path` usa `COLLATE NOCASE` porque en Windows las rutas no distinguen mayúsculas. Las tablas son `STRICT` y `format` tiene un `CHECK` con los formatos válidos. Se añade `CoreError::NonUnicodePath` (rutas no almacenables) y `CoreError::SchemaTooNew` (base de datos de una versión futura de la app).
- 1.3: limitación conocida: la búsqueda usa `LIKE`, que solo ignora mayúsculas en ASCII (`cómic` no encuentra `CÓMIC`). Se valorará FTS5 o normalización de acentos en la fase 6 si molesta.
- Tras la 1.3: cambia el flujo de trabajo a revisión por fases (ver "Cómo usar este plan").
