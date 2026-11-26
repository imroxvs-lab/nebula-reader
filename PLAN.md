# Nebula Reader — Plan de desarrollo

Lector de libros y cómics electrónicos para Windows 11, en Rust + Tauri 2.
Objetivo: diseño moderno (estilo Windows 11, Mica), bajo consumo de RAM y recursos.

Objetivos de recursos (medidos en release con `scripts/measure.ps1`, suma de la app y de sus procesos WebView2):
- **Métrica principal: working set privado**, que es la columna «Memoria» del Administrador de tareas. El *commit* se anota como dato secundario: en el proceso GPU incluye reservas de Direct3D que no ocupan RAM física.
- Reposo: no superar en más de ~30 MB la línea base de v0.1.0 (~82 MB vacía, ~86 MB con 21 elementos).
- Lectura: crecimiento acotado y estable al pasar páginas (sin fugas).
- Instalador NSIS: < 10 MB (v0.1.0: 1,80 MiB; con PDFium desde v0.4.0: 5,05 MiB).

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

## Fase 3 — EPUB (v0.3.0) ✅

### Tareas
- [x] 3.1 Parser EPUB (container.xml → OPF → spine, TOC nav/NCX, metadatos, portada) con normalización segura de rutas + tests
- [x] 3.2 Migración 3: `author`, `series`, `series_index`; importación con metadatos reales (EPUB y `ComicInfo.xml` de CBZ)
- [x] 3.3 Recursos EPUB por `nebula://` con CSP que bloquea scripts
- [x] 3.4 `EpubReader`: iframe sandbox, paginación CSS columns, TOC, progreso
- [x] 3.5 Ajustes tipográficos: fuente, tamaño, interlineado, márgenes, tema papel/sepia/oscuro

### Validación
- [x] Checks automáticos en verde
- [x] EPUB 2 y EPUB 3 correctos; EPUB malformado da un error amigable
- [x] `<script>` dentro de un EPUB no se ejecuta
- [x] Tag `v0.3.0`

## Fase 4 — PDF (v0.4.0) ✅

### Tareas
- [x] 4.1 `pdf.rs` con `pdfium-render` tras feature `pdf`; render bajo demanda a la anchura visible
- [x] 4.2 Empaquetar `pdfium.dll`; si falta, PDF se deshabilita con mensaje claro
- [x] 4.3 `PdfReader` reutilizando los controles del lector de cómics; zoom con re-render; portada desde la página 1

### Validación
- [x] Checks automáticos en verde
- [x] PDF de 500+ páginas con RAM estable al pasar páginas
- [x] Tag `v0.4.0`

## Fase 5 — CBR, CB7 y modos de cómic avanzados (v0.5.0) ✅

### Tareas
- [x] 5.1 Backends `unrar` y `sevenz-rust2` en `ArchiveBackend`, índice al abrir y caché de páginas
- [x] 5.2 Doble página
- [x] 5.3 Lectura RTL (manga)
- [x] 5.4 Scroll vertical continuo (webtoon) y opciones por libro

### Validación
- [x] Checks automáticos en verde
- [x] CBR/CB7 reales; archivos protegidos o corruptos dan un error amigable
- [x] Tag `v0.5.0`

## Fase 6 — Organización y ajustes (v0.6.0) ✅

### Tareas
- [x] 6.1 Migración: `bookmarks`, `collections`, `collection_books`, estado leído/pendiente
- [x] 6.2 Pantalla de Ajustes (tema, acento, comportamiento del lector, carpetas de la biblioteca con revisión manual o al iniciar)
- [x] 6.3 Marcadores y colecciones/estanterías en la UI
- [x] 6.4 Filtros por estado/autor/serie; reescaneo y detección de archivos movidos/eliminados
- [x] 6.5 Accesibilidad: navegación por teclado, foco visible, contraste

### Validación
- [x] Checks automáticos en verde
- [x] Migraciones aplicadas sobre una BD de v0.5.0 sin pérdida de datos
- [x] Tag `v0.6.0`

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
| v0.3.0 (fase 3 completa) | **94–96 MB** privateWS con 25 elementos | **105–116 MB** leyendo un EPUB; estable tras pasar 80 páginas (CPU en reposo ≈ 0) | ~75–250 ms | 2,12 MiB |
| v0.4.0 (fase 4 completa) | **94–99 MB** privateWS con 27 elementos | **107–112 MB** leyendo un PDF de 500 páginas; estable tras recorrer las 500 y con zoom | ~75–320 ms | 5,05 MiB (incluye `pdfium.dll`, 7,8 MB sin comprimir) |
| v0.5.0 (fase 5 completa) | **103–105 MB** privateWS con 31 elementos y portadas reales | **113–129 MB** (CBR sólido en doble página, webtoon de 40 páginas, CBR real de 83 páginas) | ~270–290 ms | 5,24 MiB |
| v0.6.0 (fase 6 completa) | **103–108 MB** privateWS con 34 elementos | **≈136 MB** máximo tras abrir un CBR en doble página y un EPUB en la misma sesión | ~80–290 ms | 5,28 MiB |

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
- Fase 3 (3.1–3.5):
  - **Parser EPUB propio** sobre `zip` + `quick-xml` 0.42 (sin el crate `epub`):
    - `container.xml` → OPF;
    - metadatos (título, autores, idioma, serie de EPUB 3 `belongs-to-collection`/`group-position` y de Calibre);
    - manifiesto y orden de lectura (`linear="no"` incluido);
    - índice desde `nav` de EPUB 3 o, si no hay, desde NCX de EPUB 2;
    - portada por `cover-image`, por `<meta name="cover">` o por una imagen cuyo nombre contenga «cover».

    Las rutas se resuelven con decodificación `%XX` y se rechazan las que salen de la raíz (con proptest). Es tolerante con XML descuidado (etiquetas sin cerrar, `&` sueltos) y con BOM.
  - **Metadatos al importar**: `ComicInfo.xml` de los CBZ (serie, número, guionista) además de EPUB, como se propuso en la fase 2. Un archivo ilegible se importa con el título del nombre. Los libros importados en versiones anteriores completan sus metadatos al generar la portada o al abrirlos. La búsqueda también encuentra por autor y serie.
  - **`OpenDocument { Comic, Epub }`** en el núcleo (despacho por enum); la caché LRU guarda documentos de cualquier formato.
  - **Seguridad del contenido EPUB** (validado con un EPUB de prueba con `<script>`, `onerror` y `onclick`: no se ejecuta nada):
    - el capítulo se descarga por `nebula://`, se sanea (fuera `script`, `iframe`, `object`, `embed`, `form`, `base`, `meta http-equiv`, atributos `on*` y URL `javascript:`) y se inserta en un `<iframe sandbox="allow-same-origin">` sin `allow-scripts`;
    - además, el protocolo sirve los recursos EPUB con `Content-Security-Policy: script-src 'none'` y `X-Content-Type-Options: nosniff`;
    - los enlaces externos se bloquean con un aviso.

    Se añaden a la CSP de la app `connect-src`, `style-src` y `font-src` para `nebula`.
  - **Lector**:
    - paginación con columnas CSS y dos columnas por página a partir de 1000 px de ancho (como un libro abierto);
    - índice anidado con el capítulo actual resaltado;
    - enlaces internos con anclas, teclado, rueda y clic en los laterales;
    - progreso global ponderado por el tamaño de cada documento;
    - posición guardada como `Locator::Epub { spine, position }`, independiente del tamaño de letra y de ventana.
  - **Ajustes tipográficos** (`epubStyle`, persistido y validado): temas papel, sepia y oscuro; fuente del libro, serif o sans; tamaño del 80 al 200 %; interlineado de 1,2 a 2,0; márgenes estrecho, normal o amplio.
  - **Fallo detectado y corregido** en la verificación: los `<style>` emiten un evento `load` al cambiar su contenido. El listener de `load` (pensado para imágenes) repaginaba, reescribía el estilo y volvía a dispararse: unas 10 000 repaginaciones por segundo, CPU al máximo y el renderizador por encima de 1 GB. Ahora el listener solo atiende imágenes, el estilo solo se reescribe si cambia y los listeners se registran una vez.
  - **Herramienta de desarrollo**: `cargo run -p nebula-core --example make_epub -- <carpeta>` genera un EPUB 3 de prueba (con contenido hostil) y un EPUB 2.
  - **Pendiente o limitado**:
    - Las fuentes con tamaño fijo en `px` dentro del CSS del libro no cambian con el ajuste de tamaño.
    - No hay búsqueda de texto dentro del libro ni subrayados (candidatos para la fase 6, junto con los marcadores).
    - EPUB con DRM o de diseño fijo (*fixed layout*) no están soportados.
- Fase 3 aprobada por el usuario; versión 0.3.0 y tag `v0.3.0` publicados.
- Fase 4 (4.1–4.3):
  - **PDFium**: build `chromium/8179` de `bblanchon/pdfium-binaries` (BSD-3/Apache-2.0), descargado por `scripts/fetch-pdfium.ps1` con versión y SHA-256 fijados. La DLL no se versiona en git (`/src-tauri/resources/pdfium/` en `.gitignore`); se incluye en el instalador como recurso (`pdfium/pdfium.dll` + `pdfium/LICENSE`). **Hay que ejecutar el script antes de compilar.**
  - **`pdfium-render` 0.9.6** (`pdfium_latest`, `thread_safe`, `image_latest`) tras la feature `pdf` de `nebula-core`. `#![forbid(unsafe_code)]` sigue vigente en el núcleo: el código `unsafe` está en la dependencia.
  - **Sin documentos PDF abiertos en memoria**: cada render reabre el archivo (PDFium carga el índice de forma perezosa). RAM estable incluso con 500 páginas.
  - **Render bajo demanda** en `/pdf/{id}/{página}?w={ancho}`: JPEG de calidad 90 al ancho en píxeles físicos (tamaño en pantalla × `devicePixelRatio`, en escalones de 256 px y acotado a 64–4096 px). El zoom vuelve a renderizar a más resolución.
  - **Lector**: reutiliza `ComicReader` (ajustes, zoom, teclado, rueda, progreso). Antes de cargar se usa la proporción de la primera página; el «1:1» de PDF es A4 a 96 ppp.
  - **Portadas**: primera página renderizada a 320 px. Al generarla o al abrir el PDF se completan título y autor (se descartan títulos genéricos como «Untitled» o «Microsoft Word - …»).
  - **Sin `pdfium.dll`** la app arranca normalmente y los PDF muestran «No se encontró el motor de PDF (pdfium.dll). Reinstala Nebula Reader para leer PDF.». La falta del motor no marca las portadas como fallidas.
  - **Errores nuevos**: `CoreError::Protected` (PDF con contraseña: mensaje específico) y `CoreError::PdfEngine`.
  - **Tests**: `crates/nebula-core/tests/pdf.rs` (7) con PDFium real; se ejecutan si `NEBULA_PDFIUM` apunta a la DLL y si no se omiten. Herramienta `make_pdf` para generar PDF de prueba.
  - **Fallo detectado y corregido** en la verificación: `rename_all` en un enum serde renombra variantes, no campos, y la interfaz recibía `aspect_ratio` en vez de `aspectRatio`, con lo que pedía `w=NaN`. Se usa `rename_all_fields` y hay un test que lo cubre. El protocolo registra ahora cada petición a nivel `debug`.
  - **Verificación del instalador**: instalación silenciosa en una carpeta temporal, `pdfium\pdfium.dll` presente, apertura de un PDF desde la copia instalada y desinstalación limpia.
  - **Pendiente o limitado**:
    - PDF con contraseña: solo se avisa; no se pide la contraseña.
    - No hay índice (marcadores del PDF), selección ni búsqueda de texto: candidatos para la fase 6.
- Fase 4 aprobada por el usuario; versión 0.4.0 y tag `v0.4.0` publicados.
- Fase 5 (5.1–5.4):
  - **Backends**: `ComicArchive` con `ArchiveBackend { Zip, Rar, SevenZ }`. **El formato se detecta por la firma del archivo**, no por la extensión (muchos `.cbr` son ZIP y viceversa). RAR con `unrar` 0.5.8 (biblioteca oficial UnRAR en C++, compilada con `cc`); 7z con `sevenz-rust2` 0.25 (solo descompresión: LZMA/LZMA2, BZip2, PPMd y Deflate). **El MSRV sube a 1.93** por `sevenz-rust2`.
  - **Lectura secuencial**: RAR solo se lee en orden (y en archivos sólidos hay que descomprimir lo anterior). Al pedir una página se leen también las 3 siguientes en la misma pasada y se guardan en una caché de 8 páginas por documento. Igual con los 7z sólidos; los 7z normales leen cada entrada directamente.
  - **Cifrado**: RAR con datos o cabeceras cifradas y 7z con contraseña dan `CoreError::Protected` («protegido con contraseña»).
  - **`ComicInfo.xml`** se lee ahora también en CBR y CB7.
  - **Enlazado**: `unrar_sys` usa funciones del registro de Windows sin declarar `advapi32`. Un `build.rs` en `nebula-core` la enlaza (no se puede usar un bloque `extern`: el núcleo prohíbe `unsafe`).
  - **Fixtures de RAR** (`crates/nebula-core/tests/fixtures/*.cbr`, menos de 1 KB cada uno): creados con WinRAR porque no hay forma libre de crear RAR (normal, sólido, datos cifrados y cabeceras cifradas). WinRAR ya no crea RAR4, así que ese formato no tiene fixture. Los 7z se generan en el test con el escritor de `sevenz-rust2` (dependencia de desarrollo).
  - **Migración 4**: `books.reader_options` (JSON con `layout` y `direction` por libro), comando `book_options_set` (JSON válido, máximo 2 KB) y validación en la interfaz.
  - **Lector** (`ComicReader`, también para PDF):
    - modos página, doble página y vertical (webtoon);
    - **doble página**: la portada va sola, las páginas apaisadas (dobles escaneadas) también, y el resto de dos en dos; los pliegos se recalculan al conocer las dimensiones de cada página, que se obtienen ya en la precarga;
    - **manga (derecha a izquierda)**: invierte el orden de los pliegos, las flechas, las zonas de clic y la barra de páginas;
    - **webtoon**: columna de hasta 900 px por el zoom; solo se montan las imágenes cercanas (±3 páginas) y el resto son huecos de altura estimada; se compensa el desplazamiento cuando cambia la altura de una página por encima de la vista.
  - **Portadas**: se generan para todos los formatos (antes la interfaz no las pedía para CBR y CB7).
  - **Herramienta**: `make_comic` admite salida `.cb7` o carpeta (para empaquetar con WinRAR) y páginas apaisadas.
  - **Verificación**: recorrido en release con un CBR sólido de 24 páginas (doble página, páginas apaisadas, manga), un CB7 de 30, un webtoon de 40 y un CBR real de 83 páginas (38 MB) que ya estaba en la biblioteca; opciones y progreso persistidos por libro.
  - **Pendiente o limitado**:
    - no se lee `Manga=YesAndRightToLeft` de `ComicInfo.xml` para activar el modo manga automáticamente;
    - no se piden contraseñas de archivos cifrados;
    - RAR multivolumen (`.part1.rar`) no probado;
    - en doble página no hay opción para desplazar los pliegos una página (cuando la portada no es una sola página).
- Fase 5 aprobada por el usuario; versión 0.5.0 y tag `v0.5.0` publicados.
- Fase 6 (6.1–6.5):
  - **Migración 5**: `books.finished_at` y `books.missing`, tablas `bookmarks`, `collections` (nombre único sin distinguir mayúsculas) y `collection_books` (borrado en cascada). Los libros con progreso ≥ 99,9 % se marcan como terminados al migrar. Probada sobre una copia de la base de datos real de v0.5.0 (31 libros y 10 progresos, sin pérdidas) y con un test que reconstruye el esquema v4.
  - **Estado de lectura** (`unread`/`reading`/`finished`): se calcula a partir del progreso y de `finished_at`. Llegar al final marca el libro como terminado. Se puede marcar a mano como «terminado» o «no leído» (esto último borra el progreso). Los terminados y los desaparecidos no salen en «Continuar leyendo».
  - **Filtros**: estado, autor y serie (valores reales de la biblioteca con su recuento, comando `library_facets`), colección, y órdenes nuevos por autor y por serie y número. La búsqueda ya incluía autor y serie.
  - **Carpetas de la biblioteca y revisión** (sustituye a las «carpetas vigiladas»): no hay vigilancia en segundo plano, por coherencia con el objetivo de bajo consumo. «Revisar ahora» (y, opcionalmente, al iniciar) hace lo siguiente:
    - marca los archivos desaparecidos y desmarca los que reaparecen;
    - reconoce los **movidos** por nombre y tamaño, y conservan progreso, marcadores y colecciones;
    - añade los nuevos.

    El disco se recorre sin bloquear la biblioteca (`plan_rescan` y `apply_rescan`) y solo se leen metadatos de los archivos nuevos. Los libros no encontrados se muestran en gris con «No encontrado»; con «Localizar archivo…» se indica la nueva ruta (mismo formato y sin duplicados). Al abrir uno desaparecido se marca y se explica qué hacer.
  - **Marcadores** en el lector de cómics y PDF y en el de EPUB: botón, panel y Ctrl+D para marcar o desmarcar. Las etiquetas son automáticas («Página 12», «Capítulo 3 · página 4»).
  - **Colecciones**: en la barra lateral, con su recuento. Se crean desde la barra lateral o desde el menú de un libro («Añadir a colección», con casillas). Se pueden renombrar y borrar (los libros se conservan).
  - **Ajustes**: tema; color de acento (6 colores, con variantes para tema claro y oscuro); ajuste, modo y sentido de lectura por defecto (para libros sin opciones propias); tema de los EPUB; carpetas de la biblioteca y revisión al iniciar; acerca de y licencias. **Cada clave se valida en Rust** (`settings.rs`) con su conjunto de valores permitidos.
  - **Accesibilidad**:
    - navegación con flechas, Inicio y Fin en la cuadrícula, con una sola parada de tabulación;
    - menús contextuales accesibles con teclado (Mayús+F10 o tecla de menú, flechas, submenús con → y ←, Esc devuelve el foco);
    - diálogo modal nativo (`<dialog>`) con foco atrapado;
    - foco visible en tarjetas y desplegables. Se corrigieron dos fallos: `content-visibility` recortaba el contorno de foco y `outline: none` lo eliminaba en los desplegables;
    - el texto terciario pasa a contraste AA (5,3:1 en claro, ≈6:1 en oscuro);
    - soporte de **alto contraste de Windows** (`forced-colors`);
    - etiquetas accesibles de las tarjetas con autor, estado y si falta el archivo.
  - **Fallo detectado y corregido**: con la flecha abajo, el foco saltaba hasta la última tarjeta porque `offsetTop` se medía respecto a la propia tarjeta.
  - **Pendiente o limitado**:
    - la búsqueda sigue sin ignorar acentos ni mayúsculas fuera de ASCII (`LIKE` de SQLite);
    - no hay selección múltiple para añadir varios libros a la vez a una colección;
    - el reconocimiento de archivos movidos usa nombre y tamaño (no un hash del contenido);
    - el libro de la biblioteca real «Diseño de aplicaciones mediante el uso intensivo de datos» aparece como no encontrado porque su PDF ya no está en `Descargas`.
- Fase 6 aprobada por el usuario; versión 0.6.0 y tag `v0.6.0` publicados.
