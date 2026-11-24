<script lang="ts">
  import { tick, untrack } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, errorMessage, pageUrl, pdfUrl } from "#lib/api.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav } from "#lib/stores/nav.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book, FitMode, OpenedBook, PageLayout, ReaderOptions, ReadingDirection } from "#lib/types.ts";

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
  /** Ancho «1:1» de una página PDF: A4 (595 pt) a 96 ppp. */
  const PDF_BASE_WIDTH = 794;
  /** Escalón de resolución de render de PDF, en píxeles físicos. */
  const RENDER_STEP = 256;
  /** Proporción alto/ancho supuesta de una página de cómic aún no cargada. */
  const COMIC_RATIO = 1.5;
  /** Ancho máximo de la columna en modo vertical (webtoon), antes del zoom. */
  const WEBTOON_MAX_WIDTH = 900;
  /** Páginas que se montan por encima y por debajo de la visible en modo vertical. */
  const WEBTOON_WINDOW = 3;

  const fitModes: { value: FitMode; label: string; short: string }[] = [
    { value: "page", label: "Ajustar a la ventana", short: "Ventana" },
    { value: "width", label: "Ajustar al ancho", short: "Ancho" },
    { value: "original", label: "Tamaño original", short: "1:1" },
  ];
  const layouts: { value: PageLayout; label: string; glyph: string }[] = [
    { value: "single", label: "Una página", glyph: "\uE7C3" },
    { value: "double", label: "Doble página", glyph: "\uE8A9" },
    { value: "webtoon", label: "Desplazamiento vertical (webtoon)", glyph: "\uE8CB" },
  ];

  type Size = { width: number; height: number };

  let opened = $state<OpenedBook | null>(null);
  let index = $state(0);
  let fit = $state<FitMode>("page");
  let zoom = $state(1);
  let layout = $state<PageLayout>("single");
  let direction = $state<ReadingDirection>("ltr");
  let chromeVisible = $state(true);
  let viewer: HTMLDivElement | undefined = $state();
  let viewerWidth = $state(0);
  let viewerHeight = $state(0);
  /** Dimensiones reales de las páginas ya cargadas (en PDF, normalizadas al ancho base). */
  let sizes = $state<Record<number, Size>>({});
  /** Estado de carga de las imágenes mostradas. */
  let loaded = $state<Record<string, boolean>>({});
  let failed = $state<Record<number, boolean>>({});

  const pageCount = $derived(opened?.pageCount ?? 0);
  const isPdf = $derived(opened?.content.type === "pdf");
  const rtl = $derived(direction === "rtl" && layout !== "webtoon");

  /** Proporción alto/ancho para páginas sin dimensiones conocidas. */
  const fallbackRatio = $derived.by(() => {
    if (opened?.content.type === "pdf") {
      const ratio = opened.content.aspectRatio;
      return Number.isFinite(ratio) && ratio > 0 ? ratio : Math.SQRT2;
    }
    const known = Object.values(sizes);
    return known.length > 0 ? known[known.length - 1].height / known[known.length - 1].width : COMIC_RATIO;
  });

  function sizeOf(page: number): Size {
    const base = isPdf ? PDF_BASE_WIDTH : (sizes[page]?.width ?? 1000);
    return sizes[page] ?? { width: base, height: base * fallbackRatio };
  }

  function isWide(page: number): boolean {
    const size = sizes[page];
    return size !== undefined && size.width > size.height;
  }

  /**
   * Agrupación en pliegos para la doble página: la portada va sola, las
   * páginas apaisadas (dobles escaneadas) también, y el resto de dos en dos.
   * Se recalcula cuando se conocen nuevas dimensiones.
   */
  const spreads = $derived.by(() => {
    const result: number[][] = [];
    let page = 0;
    while (page < pageCount) {
      if (page === 0 || isWide(page) || page + 1 >= pageCount || isWide(page + 1)) {
        result.push([page]);
        page += 1;
      } else {
        result.push([page, page + 1]);
        page += 2;
      }
    }
    return result;
  });

  const spreadIndex = $derived(
    layout === "double" ? Math.max(0, spreads.findIndex((spread) => spread.includes(index))) : -1,
  );

  /** Páginas mostradas ahora, en orden de lectura. */
  const visiblePages = $derived(
    layout === "double" ? (spreads[spreadIndex] ?? [index]) : [index],
  );

  /** Tamaño en pantalla de cada página mostrada (todas a la misma altura). */
  const displaySizes = $derived.by(() => {
    if (viewerWidth === 0 || viewerHeight === 0 || pageCount === 0) return [] as Size[];
    const reference = sizeOf(visiblePages[0]).height;
    const scaled = visiblePages.map((page) => {
      const size = sizeOf(page);
      return { width: (size.width * reference) / size.height, height: reference };
    });
    const totalWidth = scaled.reduce((sum, size) => sum + size.width, 0);
    const byWidth = viewerWidth / totalWidth;
    const byHeight = viewerHeight / reference;
    const base = fit === "page" ? Math.min(byWidth, byHeight) : fit === "width" ? byWidth : 1;
    const scale = base * zoom;
    return scaled.map((size) => ({
      width: Math.round(size.width * scale),
      height: Math.round(size.height * scale),
    }));
  });

  /** Ancho de render de un PDF: el ancho en pantalla por la densidad de píxeles, en escalones. */
  function renderWidthFor(displayWidth: number): number {
    return Math.max(RENDER_STEP, Math.ceil((displayWidth * window.devicePixelRatio) / RENDER_STEP) * RENDER_STEP);
  }

  function urlFor(page: number, displayWidth: number): string {
    if (!opened) return "";
    return isPdf ? pdfUrl(opened.book, page, renderWidthFor(displayWidth)) : pageUrl(opened.book, page);
  }

  function rememberSize(page: number, image: HTMLImageElement) {
    if (image.naturalWidth === 0) return;
    const size = isPdf
      ? { width: PDF_BASE_WIDTH, height: (PDF_BASE_WIDTH * image.naturalHeight) / image.naturalWidth }
      : { width: image.naturalWidth, height: image.naturalHeight };
    const known = sizes[page];
    if (known && Math.abs(known.height / known.width - size.height / size.width) <= 0.001) return;
    // En modo vertical, si cambia el alto de una página que está por encima de
    // la vista, se compensa el desplazamiento para que el contenido no salte.
    const above =
      layout === "webtoon" && viewer !== undefined && (webtoonOffsets[page + 1] ?? 0) <= viewer.scrollTop;
    const before = above ? webtoonOffsets[page + 1] - webtoonOffsets[page] : 0;
    sizes[page] = size;
    if (above) {
      const delta = Math.round((webtoonWidth * size.height) / size.width) - before;
      void tick().then(() => viewer?.scrollBy({ top: delta }));
    }
  }

  const appWindow = getCurrentWindow();

  // ---------- Apertura, opciones y progreso ----------

  function parseOptions(raw: string | null): ReaderOptions {
    try {
      const value = raw ? (JSON.parse(raw) as Partial<ReaderOptions>) : {};
      return {
        layout: value.layout === "double" || value.layout === "webtoon" ? value.layout : "single",
        direction: value.direction === "rtl" ? "rtl" : "ltr",
      };
    } catch {
      return { layout: "single", direction: "ltr" };
    }
  }

  $effect(() => {
    const id = book.id;
    let cancelled = false;
    api
      .openBook(id)
      .then((result) => {
        if (cancelled) return;
        const options = parseOptions(result.readerOptions);
        layout = options.layout;
        direction = options.direction;
        opened = result;
        const locator = result.progress?.locator;
        const saved = locator?.type === "page" ? locator.index : 0;
        index = Math.min(Math.max(saved, 0), result.pageCount - 1);
        if (layout === "webtoon") requestAnimationFrame(() => scrollWebtoonTo(index));
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

  function saveOptions() {
    if (!opened) return;
    const options: ReaderOptions = { layout, direction };
    void api.setBookOptions(opened.book.id, JSON.stringify(options)).catch(() => {});
  }

  function setLayout(value: PageLayout) {
    if (value === layout) return;
    const current = index;
    layout = value;
    zoom = 1;
    saveOptions();
    requestAnimationFrame(() => {
      if (value === "webtoon") scrollWebtoonTo(current);
      else if (viewer) viewer.scrollTop = 0;
    });
  }

  function toggleDirection() {
    direction = direction === "rtl" ? "ltr" : "rtl";
    saveOptions();
  }

  let pendingSave: (() => void) | null = null;
  $effect(() => {
    if (!opened) return;
    const id = opened.book.id;
    // En doble página cuenta como leída la última página visible.
    const last = visiblePages[visiblePages.length - 1] ?? index;
    const position = index;
    const percent = (last + 1) / pageCount;
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

  $effect(() => {
    return () => {
      pendingSave?.();
      library.refresh();
    };
  });

  // ---------- Precarga (modos de página) ----------

  let preloaded: HTMLImageElement[] = [];
  $effect(() => {
    if (!opened || layout === "webtoon") return;
    const first = visiblePages[0];
    const last = visiblePages[visiblePages.length - 1];
    const width = displaySizes[0]?.width ?? 0;
    const images: HTMLImageElement[] = [];
    const ahead = layout === "double" ? PRELOAD_AHEAD * 2 : PRELOAD_AHEAD;
    for (let page = first - PRELOAD_BEHIND; page <= last + ahead; page++) {
      if (visiblePages.includes(page) || page < 0 || page >= pageCount) continue;
      if (isPdf && width === 0) continue;
      const image = new Image();
      image.decoding = "async";
      const target = page;
      // Conocer antes las dimensiones permite agrupar bien los pliegos.
      image.onload = () => untrack(() => rememberSize(target, image));
      image.src = urlFor(page, width);
      images.push(image);
    }
    preloaded = images;
    return () => {
      for (const image of preloaded) {
        image.onload = null;
        image.src = "";
      }
    };
  });

  // ---------- Navegación ----------

  function scrollToEdge(edge: "top" | "bottom") {
    requestAnimationFrame(() => {
      if (!viewer) return;
      viewer.scrollTop = edge === "top" ? 0 : viewer.scrollHeight;
      viewer.scrollLeft = rtl ? viewer.scrollWidth : 0;
    });
  }

  function goTo(target: number, edge: "top" | "bottom" = "top") {
    if (!opened) return;
    const page = Math.min(Math.max(target, 0), pageCount - 1);
    if (layout === "webtoon") {
      scrollWebtoonTo(page);
      return;
    }
    if (page === index) return;
    index = page;
    scrollToEdge(edge);
  }

  function next() {
    if (layout === "double") {
      const spread = spreads[spreadIndex + 1];
      if (spread) goTo(spread[0]);
    } else goTo(index + 1);
  }

  function previous(edge: "top" | "bottom" = "top") {
    if (layout === "double") {
      const spread = spreads[spreadIndex - 1];
      if (spread) goTo(spread[0], edge);
    } else goTo(index - 1, edge);
  }

  /** Avance según el lado físico (izquierda/derecha), que en manga se invierte. */
  const towardsLeft = () => (rtl ? next() : previous());
  const towardsRight = () => (rtl ? previous() : next());

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

  // ---------- Modo vertical (webtoon) ----------

  const webtoonWidth = $derived(Math.round(Math.min(viewerWidth, WEBTOON_MAX_WIDTH) * zoom));

  /** Posición vertical de cada página (y el final) en la columna. */
  const webtoonOffsets = $derived.by(() => {
    const offsets = new Array<number>(pageCount + 1);
    let top = 0;
    for (let page = 0; page < pageCount; page++) {
      offsets[page] = top;
      const size = sizeOf(page);
      top += Math.round((webtoonWidth * size.height) / size.width);
    }
    offsets[pageCount] = top;
    return offsets;
  });

  let webtoonScroll = $state(0);

  function scrollWebtoonTo(page: number) {
    if (!viewer) return;
    viewer.scrollTop = webtoonOffsets[page] ?? 0;
    webtoonScroll = viewer.scrollTop;
  }

  /** Página que ocupa el centro de la ventana. */
  function pageAt(y: number): number {
    let low = 0;
    let high = pageCount - 1;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if (webtoonOffsets[middle] <= y) low = middle;
      else high = middle - 1;
    }
    return low;
  }

  function onViewerScroll() {
    if (layout !== "webtoon" || !viewer) return;
    webtoonScroll = viewer.scrollTop;
    const page = pageAt(viewer.scrollTop + viewer.clientHeight / 2);
    if (page !== index) index = page;
  }

  /** Solo se montan las imágenes cercanas: el resto son huecos del mismo alto. */
  const webtoonMounted = $derived.by(() => {
    if (layout !== "webtoon" || pageCount === 0) return [] as number[];
    const first = pageAt(webtoonScroll);
    const last = pageAt(webtoonScroll + viewerHeight);
    const pages: number[] = [];
    for (let page = Math.max(0, first - WEBTOON_WINDOW); page <= Math.min(pageCount - 1, last + WEBTOON_WINDOW); page++)
      pages.push(page);
    return pages;
  });

  // ---------- Interfaz ----------

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
    else if (key === "F11" || key === "f") void toggleFullscreen();
    else if (key === "Escape") void (fullscreen ? toggleFullscreen() : back());
    else if (key === "Home") goTo(0);
    else if (key === "End") goTo(pageCount - 1);
    else if (layout === "webtoon") {
      if (!viewer) return;
      const step = viewer.clientHeight * 0.85;
      if (key === "ArrowDown" || key === "PageDown" || (key === " " && !event.shiftKey))
        viewer.scrollBy({ top: step, behavior: "smooth" });
      else if (key === "ArrowUp" || key === "PageUp" || (key === " " && event.shiftKey))
        viewer.scrollBy({ top: -step, behavior: "smooth" });
      else return;
    } else if (key === "ArrowRight") towardsRight();
    else if (key === "ArrowLeft") towardsLeft();
    else if (key === "PageDown" || (key === " " && !event.shiftKey)) next();
    else if (key === "PageUp" || (key === " " && event.shiftKey)) previous();
    else return;
    event.preventDefault();
  }

  let wheelOverflow = 0;
  let wheelLock = false;
  function onWheel(event: WheelEvent) {
    if (event.ctrlKey) {
      event.preventDefault();
      setZoom(event.deltaY < 0 ? zoom * ZOOM_STEP : zoom / ZOOM_STEP);
      return;
    }
    // En modo vertical la rueda desplaza con normalidad.
    if (layout === "webtoon" || !viewer || wheelLock) return;
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
    else previous("bottom");
  }

  function onViewerClick(event: MouseEvent) {
    if (!viewer) return;
    const rect = viewer.getBoundingClientRect();
    const x = (event.clientX - rect.left) / rect.width;
    if (layout !== "webtoon" && x < 0.3) towardsLeft();
    else if (layout !== "webtoon" && x > 0.7) towardsRight();
    else if (chromeVisible) {
      clearTimeout(hideTimer);
      chromeVisible = false;
    } else showChrome();
  }

  /** Páginas mostradas en orden físico (de izquierda a derecha). */
  const onScreen = $derived(
    visiblePages.map((page, position) => ({ page, size: displaySizes[position] as Size | undefined })),
  );
  const physical = $derived(rtl ? [...onScreen].reverse() : onScreen);
  const loading = $derived(
    layout !== "webtoon" &&
      opened !== null &&
      visiblePages.some((page) => !loaded[`${page}:${urlFor(page, displaySizes[0]?.width ?? 0)}`] && !failed[page]),
  );
  const pageLabel = $derived(
    visiblePages.length > 1 ? `Páginas ${visiblePages[0] + 1}–${visiblePages[1] + 1} de ${pageCount}` : `Página ${index + 1} de ${pageCount}`,
  );
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="reader"
  class:chrome-hidden={!chromeVisible}
  onmousemove={showChrome}
  role="application"
  aria-label={isPdf ? "Lector de PDF" : "Lector de cómics"}
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
        <span>{pageLabel}</span>
      {/if}
    </div>

    <div class="group" role="radiogroup" aria-label="Modo de lectura">
      {#each layouts as option (option.value)}
        <button
          class="tool"
          class:selected={layout === option.value}
          role="radio"
          aria-checked={layout === option.value}
          aria-label={option.label}
          title={option.label}
          onclick={() => setLayout(option.value)}
        >
          <span class="icon" aria-hidden="true">{option.glyph}</span>
        </button>
      {/each}
    </div>

    {#if !isPdf && layout !== "webtoon"}
      <div class="group">
        <button
          class="tool direction"
          class:selected={direction === "rtl"}
          aria-pressed={direction === "rtl"}
          onclick={toggleDirection}
          title={direction === "rtl"
            ? "Lectura manga (de derecha a izquierda). Pulsa para lectura occidental"
            : "Lectura occidental (de izquierda a derecha). Pulsa para lectura manga"}
        >
          {direction === "rtl" ? "← Manga" : "Occidental →"}
        </button>
      </div>
    {/if}

    {#if layout !== "webtoon"}
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
    {/if}

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
    class:webtoon={layout === "webtoon"}
    bind:this={viewer}
    bind:clientWidth={viewerWidth}
    bind:clientHeight={viewerHeight}
    onwheel={onWheel}
    onclick={onViewerClick}
    onscroll={onViewerScroll}
  >
    {#if opened && layout === "webtoon"}
      <div class="strip" style:width="{webtoonWidth}px" style:height="{webtoonOffsets[pageCount] ?? 0}px">
        {#each webtoonMounted as page (page)}
          {@const height = (webtoonOffsets[page + 1] ?? 0) - (webtoonOffsets[page] ?? 0)}
          <img
            class="strip-page"
            src={urlFor(page, webtoonWidth)}
            alt="Página {page + 1} de {pageCount}"
            draggable="false"
            decoding="async"
            style:top="{webtoonOffsets[page]}px"
            style:height="{height}px"
            onload={(e) => rememberSize(page, e.currentTarget as HTMLImageElement)}
          />
        {/each}
      </div>
    {:else if opened}
      <div class="spread">
        {#each physical as { page, size } (page)}
          {@const url = urlFor(page, size?.width ?? 0)}
          <img
            class="page"
            class:sized={size !== undefined && (sizes[page] !== undefined || isPdf)}
            src={url}
            alt="Página {page + 1} de {pageCount}"
            draggable="false"
            style:width={size ? `${size.width}px` : null}
            style:height={size ? `${size.height}px` : null}
            onload={(e) => {
              rememberSize(page, e.currentTarget as HTMLImageElement);
              loaded[`${page}:${url}`] = true;
              failed[page] = false;
            }}
            onerror={() => (failed[page] = true)}
          />
        {/each}
      </div>
      {#if visiblePages.some((page) => failed[page])}
        <div class="status error">
          <span class="icon" aria-hidden="true">&#xEB90;</span>
          No se pudo mostrar la página {(visiblePages.find((page) => failed[page]) ?? 0) + 1}.
        </div>
      {:else if loading}
        <div class="status"><span class="spinner" aria-hidden="true"></span></div>
      {/if}
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
      <button class="tool" onclick={towardsLeft} aria-label={rtl ? "Página siguiente" : "Página anterior"}>
        <span class="icon" aria-hidden="true">&#xE76B;</span>
      </button>
      <input
        type="range"
        min="0"
        max={pageCount - 1}
        value={index}
        dir={rtl ? "rtl" : "ltr"}
        oninput={(e) => goTo(Number((e.currentTarget as HTMLInputElement).value))}
        aria-label="Página"
        style:--progress="{pageCount > 1 ? (index / (pageCount - 1)) * 100 : 100}%"
      />
      <button class="tool" onclick={towardsRight} aria-label={rtl ? "Página anterior" : "Página siguiente"}>
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

  .spread {
    display: flex;
    align-items: center;
  }

  .strip {
    position: relative;
    margin: 0 auto;
    flex-shrink: 0;
  }

  .strip-page {
    position: absolute;
    left: 0;
    width: 100%;
    display: block;
    user-select: none;
  }

  .viewer.webtoon {
    display: block;
    overflow-x: auto;
    overflow-y: scroll;
  }

  .direction {
    font-size: 12px;
    min-width: 96px;
  }

  .pager input[type="range"][dir="rtl"] {
    background: linear-gradient(
      270deg,
      #b4a3ff var(--progress),
      rgba(255, 255, 255, 0.2) var(--progress)
    );
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
