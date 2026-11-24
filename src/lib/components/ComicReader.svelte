<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, errorMessage, pageUrl } from "#lib/api.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav } from "#lib/stores/nav.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book, FitMode, OpenedBook } from "#lib/types.ts";

  let { book }: { book: Book } = $props();

  const PRELOAD_AHEAD = 2;
  const PRELOAD_BEHIND = 1;
  const SAVE_DEBOUNCE_MS = 600;
  const CHROME_HIDE_MS = 2500;
  const ZOOM_MIN = 0.25;
  const ZOOM_MAX = 4;
  const ZOOM_STEP = 1.2;
  /** Desplazamiento extra de rueda en un borde antes de cambiar de página. */
  const WHEEL_PAGE_THRESHOLD = 240;
  const FIT_SETTING = "readerFit";

  const fitModes: { value: FitMode; label: string; short: string }[] = [
    { value: "page", label: "Ajustar a la ventana", short: "Ventana" },
    { value: "width", label: "Ajustar al ancho", short: "Ancho" },
    { value: "original", label: "Tamaño original", short: "1:1" },
  ];

  let opened = $state<OpenedBook | null>(null);
  let index = $state(0);
  let fit = $state<FitMode>("page");
  let zoom = $state(1);
  let loading = $state(true);
  let pageError = $state(false);
  let chromeVisible = $state(true);
  let viewer: HTMLDivElement | undefined = $state();

  let natural = $state<{ width: number; height: number } | null>(null);
  let viewerWidth = $state(0);
  let viewerHeight = $state(0);

  const pageCount = $derived(opened?.pageCount ?? 0);

  /** Tamaño en pantalla de la página según el ajuste y el zoom. */
  const pageSize = $derived.by(() => {
    if (!natural || viewerWidth === 0 || viewerHeight === 0) return null;
    const byWidth = viewerWidth / natural.width;
    const byHeight = viewerHeight / natural.height;
    const base = fit === "page" ? Math.min(byWidth, byHeight) : fit === "width" ? byWidth : 1;
    const scale = base * zoom;
    return {
      width: Math.round(natural.width * scale),
      height: Math.round(natural.height * scale),
    };
  });
  const currentUrl = $derived(opened ? pageUrl(opened.book, index) : "");
  const appWindow = getCurrentWindow();

  // Abrir el libro y restaurar la posición.
  $effect(() => {
    const id = book.id;
    let cancelled = false;
    api
      .openBook(id)
      .then((result) => {
        if (cancelled) return;
        opened = result;
        const locator = result.progress?.locator;
        const saved = locator?.type === "page" ? locator.index : 0;
        index = Math.min(Math.max(saved, 0), result.pageCount - 1);
      })
      .catch((error) => {
        toasts.show("error", `No se pudo abrir «${book.title}»`, errorMessage(error));
        nav.closeReader();
      });
    api
      .getSetting(FIT_SETTING)
      .then((value) => {
        if (value === "page" || value === "width" || value === "original") fit = value;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      void api.closeBook(id).catch(() => {});
    };
  });

  // Guardar el progreso (con retardo para no escribir en cada pulsación).
  let pendingSave: (() => void) | null = null;
  $effect(() => {
    if (!opened) return;
    const id = opened.book.id;
    const position = index;
    const percent = (position + 1) / pageCount;
    const save = () => {
      void api.saveProgress(id, { type: "page", index: position }, percent).catch(() => {});
    };
    pendingSave = save;
    const timer = setTimeout(() => {
      save();
      pendingSave = null;
    }, SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  // Al salir se guarda lo pendiente y se refresca la biblioteca.
  $effect(() => {
    return () => {
      pendingSave?.();
      library.refresh();
    };
  });

  // Precarga de las páginas vecinas. Se guardan las referencias para que no se
  // cancelen las descargas; el navegador conserva después las imágenes en caché.
  let preloaded: HTMLImageElement[] = [];
  $effect(() => {
    if (!opened) return;
    const current = opened.book;
    const images: HTMLImageElement[] = [];
    for (let i = index - PRELOAD_BEHIND; i <= index + PRELOAD_AHEAD; i++) {
      if (i === index || i < 0 || i >= pageCount) continue;
      const image = new Image();
      image.decoding = "async";
      image.src = pageUrl(current, i);
      images.push(image);
    }
    preloaded = images;
    return () => {
      // Cancela las precargas que ya no hacen falta si se salta lejos.
      for (const image of preloaded) image.src = "";
    };
  });

  // Indicador de carga por página.
  $effect(() => {
    void currentUrl;
    loading = true;
    pageError = false;
    natural = null;
  });

  function goTo(target: number, scrollTo: "top" | "bottom" = "top") {
    if (!opened) return;
    const next = Math.min(Math.max(target, 0), pageCount - 1);
    if (next === index) return;
    index = next;
    requestAnimationFrame(() => {
      if (!viewer) return;
      viewer.scrollTop = scrollTo === "top" ? 0 : viewer.scrollHeight;
      viewer.scrollLeft = 0;
    });
  }

  const next = () => goTo(index + 1);
  const previous = () => goTo(index - 1);

  function setFit(mode: FitMode) {
    fit = mode;
    zoom = 1;
    void api.setSetting(FIT_SETTING, mode).catch(() => {});
  }

  function setZoom(value: number) {
    zoom = Math.min(Math.max(value, ZOOM_MIN), ZOOM_MAX);
  }

  const fullscreen = $derived(nav.fullscreen);

  async function toggleFullscreen() {
    const target = !nav.fullscreen;
    try {
      await appWindow.setFullscreen(target);
      nav.fullscreen = target;
    } catch (error) {
      toasts.show("error", "No se pudo cambiar a pantalla completa", errorMessage(error));
    }
    showChrome();
  }

  async function back() {
    if (fullscreen) await toggleFullscreen();
    nav.closeReader();
  }

  // Barra de herramientas que se oculta sola mientras se lee.
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let pointerOverChrome = false;
  function showChrome() {
    chromeVisible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (!pointerOverChrome) chromeVisible = false;
    }, CHROME_HIDE_MS);
  }

  $effect(() => {
    showChrome();
    return () => clearTimeout(hideTimer);
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement && event.target.type !== "range") return;
    const key = event.key;
    if (event.ctrlKey && (key === "+" || key === "=")) setZoom(zoom * ZOOM_STEP);
    else if (event.ctrlKey && key === "-") setZoom(zoom / ZOOM_STEP);
    else if (event.ctrlKey && key === "0") setZoom(1);
    else if (key === "ArrowRight" || key === "PageDown" || (key === " " && !event.shiftKey)) next();
    else if (key === "ArrowLeft" || key === "PageUp" || (key === " " && event.shiftKey)) previous();
    else if (key === "Home") goTo(0);
    else if (key === "End") goTo(pageCount - 1);
    else if (key === "F11" || key === "f") void toggleFullscreen();
    else if (key === "Escape") void (fullscreen ? toggleFullscreen() : back());
    else return;
    event.preventDefault();
  }

  // Rueda: desplaza la página; en los bordes, tras un pequeño margen, cambia de página.
  let wheelOverflow = 0;
  let wheelLock = false;
  function onWheel(event: WheelEvent) {
    if (event.ctrlKey) {
      event.preventDefault();
      setZoom(event.deltaY < 0 ? zoom * ZOOM_STEP : zoom / ZOOM_STEP);
      return;
    }
    if (!viewer || wheelLock) return;
    const atTop = viewer.scrollTop <= 0;
    const atBottom = viewer.scrollTop + viewer.clientHeight >= viewer.scrollHeight - 1;
    const scrollable = viewer.scrollHeight > viewer.clientHeight + 1;
    const goingDown = event.deltaY > 0;

    if ((goingDown && !atBottom) || (!goingDown && !atTop)) {
      wheelOverflow = 0;
      return;
    }
    event.preventDefault();
    wheelOverflow += Math.abs(event.deltaY);
    if (scrollable && wheelOverflow < WHEEL_PAGE_THRESHOLD) return;
    wheelOverflow = 0;
    wheelLock = true;
    setTimeout(() => (wheelLock = false), 200);
    if (goingDown) next();
    else goTo(index - 1, "bottom");
  }

  // Clic en los laterales para pasar página; en el centro, mostrar/ocultar la barra.
  function onViewerClick(event: MouseEvent) {
    if (!viewer) return;
    const rect = viewer.getBoundingClientRect();
    const x = (event.clientX - rect.left) / rect.width;
    if (x < 0.3) previous();
    else if (x > 0.7) next();
    else if (chromeVisible) {
      clearTimeout(hideTimer);
      chromeVisible = false;
    } else showChrome();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="reader"
  class:chrome-hidden={!chromeVisible}
  onmousemove={showChrome}
  role="application"
  aria-label="Lector de cómics"
>
  <div
    class="toolbar"
    role="toolbar"
    tabindex="-1"
    aria-label="Herramientas del lector"
    onmouseenter={() => (pointerOverChrome = true)}
    onmouseleave={() => (pointerOverChrome = false)}
  >
    <button class="tool" onclick={back} aria-label="Volver a la biblioteca" title="Volver (Esc)">
      <span class="icon" aria-hidden="true">&#xE72B;</span>
    </button>
    <div class="title">
      <strong>{book.title}</strong>
      {#if opened}
        <span>Página {index + 1} de {pageCount}</span>
      {/if}
    </div>

    <div class="group" role="radiogroup" aria-label="Ajuste de página">
      {#each fitModes as mode (mode.value)}
        <button
          class="tool"
          class:selected={fit === mode.value}
          role="radio"
          aria-checked={fit === mode.value}
          aria-label={mode.label}
          title={mode.label}
          onclick={() => setFit(mode.value)}
        >
          {mode.short}
        </button>
      {/each}
    </div>

    <div class="group">
      <button class="tool" onclick={() => setZoom(zoom / ZOOM_STEP)} aria-label="Alejar" title="Alejar (Ctrl -)">
        <span class="icon" aria-hidden="true">&#xE71F;</span>
      </button>
      <button class="tool zoom-label" onclick={() => setZoom(1)} title="Restablecer zoom (Ctrl 0)">
        {Math.round(zoom * 100)} %
      </button>
      <button class="tool" onclick={() => setZoom(zoom * ZOOM_STEP)} aria-label="Acercar" title="Acercar (Ctrl +)">
        <span class="icon" aria-hidden="true">&#xE8A3;</span>
      </button>
    </div>

    <button
      class="tool"
      onclick={toggleFullscreen}
      aria-label={fullscreen ? "Salir de pantalla completa" : "Pantalla completa"}
      title="Pantalla completa (F11)"
    >
      <span class="icon" aria-hidden="true">{fullscreen ? "\uE73F" : "\uE740"}</span>
    </button>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="viewer"
    bind:this={viewer}
    bind:clientWidth={viewerWidth}
    bind:clientHeight={viewerHeight}
    onwheel={onWheel}
    onclick={onViewerClick}
  >
    {#if opened}
      <img
        class="page"
        class:sized={pageSize !== null}
        src={currentUrl}
        alt="Página {index + 1} de {pageCount}"
        draggable="false"
        style:width={pageSize ? `${pageSize.width}px` : null}
        style:height={pageSize ? `${pageSize.height}px` : null}
        onload={(e) => {
          const image = e.currentTarget as HTMLImageElement;
          natural = { width: image.naturalWidth, height: image.naturalHeight };
          loading = false;
        }}
        onerror={() => {
          loading = false;
          pageError = true;
        }}
      />
    {/if}
    {#if loading && !pageError}
      <div class="status"><span class="spinner" aria-hidden="true"></span></div>
    {:else if pageError}
      <div class="status error">
        <span class="icon" aria-hidden="true">&#xEB90;</span>
        No se pudo mostrar la página {index + 1}.
      </div>
    {/if}
  </div>

  {#if opened}
    <div
      class="pager"
      role="toolbar"
      tabindex="-1"
      aria-label="Navegación de páginas"
      onmouseenter={() => (pointerOverChrome = true)}
      onmouseleave={() => (pointerOverChrome = false)}
    >
      <button class="tool" onclick={previous} disabled={index === 0} aria-label="Página anterior">
        <span class="icon" aria-hidden="true">&#xE76B;</span>
      </button>
      <input
        type="range"
        min="0"
        max={pageCount - 1}
        value={index}
        oninput={(e) => goTo(Number((e.currentTarget as HTMLInputElement).value))}
        aria-label="Página"
        style:--progress="{pageCount > 1 ? (index / (pageCount - 1)) * 100 : 100}%"
      />
      <button class="tool" onclick={next} disabled={index >= pageCount - 1} aria-label="Página siguiente">
        <span class="icon" aria-hidden="true">&#xE76C;</span>
      </button>
      <span class="counter">{index + 1} / {pageCount}</span>
    </div>
  {/if}
</div>

<style>
  .reader {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: #101014;
    color: #fff;
    /* El lector siempre es oscuro: la página es la protagonista. */
    --reader-chrome: rgba(24, 24, 30, 0.88);
    --reader-hover: rgba(255, 255, 255, 0.08);
  }

  .toolbar,
  .pager {
    position: absolute;
    left: 0;
    right: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: var(--reader-chrome);
    transition:
      opacity 200ms var(--ease),
      transform 200ms var(--ease);
  }

  .toolbar {
    top: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .pager {
    bottom: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .chrome-hidden .toolbar {
    opacity: 0;
    transform: translateY(-100%);
    pointer-events: none;
  }

  .chrome-hidden .pager {
    opacity: 0;
    transform: translateY(100%);
    pointer-events: none;
  }

  .chrome-hidden {
    cursor: none;
  }

  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 18px;
  }

  .title strong {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title span {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.65);
  }

  .group {
    display: flex;
    gap: 2px;
    padding-left: 8px;
    border-left: 1px solid rgba(255, 255, 255, 0.1);
  }

  .tool {
    min-width: 36px;
    height: 34px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: #fff;
    font-size: 14px;
    transition: background var(--duration) var(--ease);
  }

  .tool:hover {
    background: var(--reader-hover);
  }

  .tool.selected {
    background: rgba(180, 163, 255, 0.22);
    color: #c9bdff;
  }

  .tool:disabled {
    opacity: 0.35;
  }

  .group .tool:not(:has(.icon)) {
    font-size: 12px;
  }

  .zoom-label {
    font-size: 12px;
    min-width: 56px;
    font-variant-numeric: tabular-nums;
  }

  .viewer {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    /* `safe` evita recortar el inicio de una página ampliada. */
    place-items: safe center;
    /* La barra se superpone a la página, así que esta no cambia de tamaño al ocultarla. */
    /* Hueco fijo para la barra de desplazamiento: si apareciera y desapareciera,
       el ancho útil cambiaría sin que lo detecte `bind:clientWidth`. */
    scrollbar-gutter: stable both-edges;
    scrollbar-color: rgba(255, 255, 255, 0.3) transparent;
  }

  .page {
    display: block;
    max-width: none;
    user-select: none;
    box-shadow: 0 0 24px rgba(0, 0, 0, 0.5);
    /* Hasta conocer su tamaño real no se muestra, para evitar un salto. */
    visibility: hidden;
  }

  .page.sized {
    visibility: visible;
  }

  .status {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    color: rgba(255, 255, 255, 0.7);
  }

  .status.error {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 8px;
  }

  .status .icon {
    font-size: 28px;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid rgba(255, 255, 255, 0.15);
    border-top-color: #b4a3ff;
    animation: spin 800ms linear infinite;
  }

  .pager input[type="range"] {
    flex: 1;
    height: 4px;
    appearance: none;
    border-radius: 2px;
    background: linear-gradient(
      90deg,
      #b4a3ff var(--progress),
      rgba(255, 255, 255, 0.2) var(--progress)
    );
    outline-offset: 6px;
  }

  .pager input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    border: 4px solid #b4a3ff;
  }

  .counter {
    min-width: 72px;
    text-align: right;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.7);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
