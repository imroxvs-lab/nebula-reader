# Nebula Reader — Plan de desarrollo

Lector de libros y cómics electrónicos para Windows 11, en Rust + Tauri 2.
Objetivo: diseño moderno (estilo Windows 11, Mica), bajo consumo de RAM y recursos.

Objetivos de recursos (medidos en release con `scripts/measure.ps1`, suma de la app y de sus procesos WebView2):
- **Métrica principal: working set privado**, que es la columna «Memoria» del Administrador de tareas. El *commit* se anota como dato secundario: en el proceso GPU incluye reservas de Direct3D que no ocupan RAM física.
- Reposo: no superar en más de ~30 MB la línea base de v0.1.0 (~82 MB vacía, ~86 MB con 21 elementos).
- Lectura: crecimiento acotado y estable al pasar páginas (sin fugas).
- Instalador NSIS: < 10 MB sin pdfium (v0.1.0: 1,80 MiB).

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
6. Al aprobar una fase se publica en GitHub (`github.com/imroxvs-lab/nebula-reader`): `git push origin main` y `git push origin <tag>`.
   Solo se publican archivos del programa; los del asistente se excluyen localmente en `.git/info/exclude`.
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

## Publicación en GitHub ✅
- [x] Repositorio público `github.com/imroxvs-lab/nebula-reader` con `main`, tags y licencia MIT
- [x] Historial sin archivos del asistente
- [x] Sin coautoría del asistente: verificado por el usuario en GitHub

---

## Fase 1 — Base, pantalla de inicio y SQLite (v0.1.0) ✅

### Tareas
- [x] 1.1 Scaffold con `create-tauri-app` (SvelteKit 3 + TS) desde Windows; convertir a workspace; crear `crates/nebula-core`; lints, `[profile.release]` (lto, codegen-units=1, opt-level="s", strip, panic=abort), `.gitignore`, `git init`
- [x] 1.2 Dominio y errores: `BookId`, `BookFormat` (detección por extensión), `BookKind`, `Book`, `NewBook`, `SortBy`, `LibraryQuery`, `ImportReport`, `CoreError` + tests
- [x] 1.3 SQLite: `Library` (open / open_in_memory), migración 1 (`books`, `settings`, WAL, foreign_keys), CRUD, búsqueda/filtro/orden, settings + tests
- [x] 1.4 Importación de archivos/carpetas (`walkdir`), filtro de formatos, duplicados, título desde nombre de archivo, `ImportReport` + tests
- [x] 1.5 Capa Tauri: `AppState`, comandos `library_list/import/remove`, `settings_get/set`, `CommandError`, BD en AppData, capabilities y CSP, plugins dialog y window-state
- [x] 1.6 Shell UI: TitleBar propia, Mica, Sidebar, tema claro/oscuro/sistema persistido
- [x] 1.7 Pantalla de inicio y biblioteca: HomeView (recientes, estado vacío), LibraryView (búsqueda, orden, filtro), BookCard con portada provisional
- [x] 1.8 Importación desde UI (diálogo de archivos/carpeta, drag&drop), toasts, quitar libro

### Validación
- [x] Checks automáticos en verde
- [x] Importar archivos mixtos y carpeta; los no soportados se informan
- [x] Cerrar y reabrir: biblioteca, tema y ventana persisten
- [x] Duplicados ignorados; búsqueda/orden/filtro correctos
- [x] Línea base medida en release (RAM en reposo, tiempo de arranque) y anotada abajo
- [x] Tag `v0.1.0`

## Fase 2 — Lector CBZ, portadas y progreso (v0.2.0) ✅

### Tareas
- [x] 2.1 `ComicArchive` con backend zip, orden natural, filtro de imágenes, límites anti zip-bomb + tests (proptest)
- [x] 2.2 Miniaturas: portada desde la primera página, redimensionado con `image`, caché en `AppData/thumbs`
- [x] 2.3 Migración 2: `page_count`, `cover_status` (en lugar de `cover_path`, ver registro), tabla `progress`
- [x] 2.4 Protocolo `nebula://` (páginas y miniaturas), comandos `book_open`, `progress_save`, LRU de documentos
- [x] 2.5 UI: portadas reales lazy, `ComicReader` (encaje, zoom, teclado/rueda, precarga ±2), barra de progreso
- [x] 2.6 "Continuar leyendo" en inicio y `last_opened_at`

### Validación
- [x] Checks automáticos en verde
- [x] Cómic de 200+ páginas fluido; progreso se restaura al reabrir
- [x] Zip corrupto y rutas `..` manejados sin fallo
- [x] RAM en lectura medida y anotada
- [x] Tag `v0.2.0`

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
| v0.1.0 (scaffold 1.1, preliminar) | 152 MB *commit* (métrica antigua) | — | ~250–380 ms | 1,12 MiB |
| v0.1.0 (fase 1 completa) | **82 MB** privateWS vacía / **86 MB** con 21 elementos (*commit* 170 / 252 MB) | — | ~85–130 ms hasta ventana visible | 1,80 MiB |
| v0.2.0 (fase 2 completa) | **94 MB** privateWS con 23 elementos (*commit* 280 MB) | **110–115 MB** leyendo un CBZ de 220 páginas de 1600×2400 (1,3 MB/página); estable tras pasar 45 páginas y volver | ~100–120 ms | 2,06 MiB |

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
- Fase 1 (1.4–1.8):
  - **SvelteKit 3 eliminó el alias `$lib`.** Se usa `#lib` mediante `imports` en `package.json`, y los módulos `.ts` se importan con extensión explícita (`#lib/api.ts`, `#lib/stores/x.svelte.ts`).
  - **Identificador** cambiado de `com.nebulareader.app` a `com.nebulareader.reader`: Tauri avisa de que acabar en `.app` choca con macOS, y aún no había datos de usuario. Datos: `%LOCALAPPDATA%\com.nebulareader.reader\library.db`; estado de ventana en `%APPDATA%\com.nebulareader.reader`.
  - **Añadidos no previstos**: `LibraryQuery.limit`, `LibraryStats` y el comando `library_stats` (contadores de la barra lateral e inicio); `Library::add_scanned` separa el recorrido del disco (sin bloquear la biblioteca) de la inserción (una transacción); ejemplo de desarrollo `cargo run -p nebula-core --example import -- <db> <rutas>`.
  - **Importación**: en las carpetas, los archivos no soportados se ignoran sin contarlos (evita ruido por imágenes y notas); solo cuentan los seleccionados explícitamente. No se siguen enlaces simbólicos.
  - **Ajustes**: el frontend solo puede leer y escribir claves de una lista blanca (`theme`), con límite de tamaño.
  - **Métrica de RAM**: se pasa del *commit* al working set privado. Las sombras difuminadas permanentes y los degradados radiales de las portadas provisionales subían ~40–70 MB el *commit* del proceso GPU sin cambiar la RAM real; aun así se sustituyeron por degradados lineales y un borde (sombra solo al pasar el ratón) para gastar menos memoria GPU. `backdrop-filter` se evita en elementos repetidos.
  - **Accesibilidad**: los glifos de icono llevan `aria-hidden` y las tarjetas un `aria-label` conciso (detectado al inspeccionar el árbol de UI Automation).
  - **Pendiente o limitado**:
    - Pulsar una tarjeta muestra un aviso: el lector llega en la fase 2.
    - «Mostrar en carpeta» no se implementa todavía.
    - El arrastrar y soltar no se ha podido verificar de forma automática.
    - Si la biblioteca no se puede abrir al arrancar, en release el error solo se registra (no hay consola): se añadirá un archivo de log o un diálogo de error (fase 6/7).
- Fase 1 aprobada por el usuario; tag `v0.1.0` creado.
- Fase 2 (2.1–2.6):
  - **`cover_status` en lugar de `cover_path`**: la miniatura vive siempre en `thumbs/<id>.jpg` (ruta derivada del id), así que guardar la ruta solo podría quedar obsoleta. El estado (`pending`/`ready`/`failed`) evita reintentar archivos dañados. Si el fallo es de E/S (archivo no encontrado, p. ej. disco desconectado) no se marca como `failed`.
  - **Portadas bajo demanda**: no se generan al importar. Las genera el protocolo `nebula://` la primera vez que la interfaz pide una portada visible (`loading="lazy"`), con un máximo de 2 en paralelo (semáforo de `tokio`, solo la función `sync`). JPEG de 320 px de ancho, calidad 82 y límites de decodificación de 16 384 px y 512 MB. Solo CBZ por ahora; el resto de formatos muestran la portada provisional hasta su fase.
  - **Protocolo**: `/cover/{id}` y `/page/{id}/{index}`, con un parámetro `?v=<addedAt>` que permite cachear de forma inmutable aunque SQLite reutilice un id tras un borrado. Solo se sirven libros de la biblioteca por id; nunca rutas.
  - **Caché LRU de 3 documentos abiertos**, implementada a mano (sin el crate `lru`): bastan unas líneas.
  - **Precarga**: 2 páginas por delante y 1 por detrás (no ±2), porque retroceder es menos frecuente. No se reescalan las páginas en Rust: medido, la RAM en lectura se mantiene en 110–115 MB con páginas de 1600×2400. Se revisará en la fase 7 con escaneos mayores.
  - **Lector**: ajuste a la ventana, al ancho y 1:1, con el tamaño calculado en JS a partir de las dimensiones reales; zoom (Ctrl+rueda, Ctrl ±, Ctrl 0); teclado (flechas, RePág/AvPág, Espacio, Inicio/Fin, F11, Esc); rueda con cambio de página al llegar al borde; clic en los laterales; barra que se oculta sola; pantalla completa; progreso guardado con un retardo de 600 ms y al salir. El ajuste elegido se recuerda (`readerFit`).
  - **Errores comprensibles**: los errores de archivo se muestran con mensajes para el usuario («El archivo está dañado o no es un cómic válido.»); el detalle técnico queda en el registro.
  - **Accesibilidad**: la tarjeta pasa a ser un contenedor con dos botones hermanos (abrir y «Más opciones»). Antes, el botón interior quedaba oculto para lectores de pantalla porque estaba dentro de un `role="button"`.
  - **Herramienta de desarrollo**: `cargo run --release -p nebula-core --example make_comic -- <salida.cbz> [páginas] [ancho] [alto]` genera cómics de prueba con las páginas numeradas.
  - **Pendiente o limitado**:
    - Portadas AVIF: no hay decodificador AVIF (evita una dependencia nativa) y se usa la provisional; las páginas AVIF sí se muestran, porque las decodifica WebView2.
    - No se leen los metadatos `ComicInfo.xml` (título, serie, autor): candidato para la fase 3 junto con los metadatos de EPUB.
- Fase 2 aprobada por el usuario; versión subida a 0.2.0 (`Cargo.toml`, `tauri.conf.json`, `package.json`) y tag `v0.2.0` creado.
- Publicación en GitHub: repositorio público `nebula-reader`, con licencia MIT (`LICENSE`). Antes del primer push se reescribió el historial para que no contenga archivos del asistente (los hashes de commit anteriores a esta fecha ya no son válidos). Cada fase aprobada se publica junto con su tag.
- Publicación verificada por el usuario: no aparece coautoría del asistente en el repositorio.
