<script lang="ts">
  import { untrack } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, epubBase, epubUrl, errorMessage } from "#lib/api.ts";
  import { FONT_SIZE, LINE_HEIGHT, MARGINS, epubStyle } from "#lib/stores/epubStyle.svelte.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav } from "#lib/stores/nav.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book, EpubStyle, EpubTheme, SpineItem, TocEntry } from "#lib/types.ts";

  let { book }: { book: Book } = $props();

  const SAVE_DEBOUNCE_MS = 600;
  const CHROME_HIDE_MS = 3000;
  const WHEEL_LOCK_MS = 250;
  const STYLE_ID = "nebula-reader-style";
  /** A partir de este ancho de página se muestran dos columnas (como un libro abierto). */
  const TWO_COLUMNS_FROM = 1000;
  /** Elementos que nunca deben llegar al documento mostrado. */
  const BLOCKED = "script, iframe, frame, object, embed, applet, form, meta[http-equiv], base";

  const themes: { value: EpubTheme; label: string }[] = [
    { value: "paper", label: "Papel" },
    { value: "sepia", label: "Sepia" },
    { value: "dark", label: "Oscuro" },
  ];
  const THEME_COLORS: Record<EpubTheme, { bg: string; fg: string; link: string }> = {
    paper: { bg: "#fbfaf7", fg: "#1f1f1f", link: "#5b3fc4" },
    sepia: { bg: "#f4ecd8", fg: "#4a3b28", link: "#8a4b17" },
    dark: { bg: "#1b1b1f", fg: "#dcdcdc", link: "#b4a3ff" },
  };
  const FONT_STACKS = {
    serif: '"Georgia", "Cambria", "Palatino Linotype", serif',
    sans: '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif',
  } as const;

  let spine = $state<SpineItem[]>([]);
  let toc = $state<TocEntry[]>([]);
  let chapter = $state(0);
  let page = $state(0);
  let pages = $state(1);
  let ready = $state(false);
  let loading = $state(true);
  let panel = $state<"toc" | "style" | null>(null);
  let chromeVisible = $state(true);
  let frame: HTMLIFrameElement | undefined = $state();

  const style = $derived(epubStyle.value);
  const colors = $derived(THEME_COLORS[style.theme]);
  const totalSize = $derived(spine.reduce((sum, item) => sum + Math.max(item.size, 1), 0));

  /** Fracción del libro leída al terminar la página actual. */
  const percent = $derived.by(() => {
    if (spine.length === 0 || totalSize === 0) return 0;
    const before = spine.slice(0, chapter).reduce((sum, item) => sum + Math.max(item.size, 1), 0);
    const current = Math.max(spine[chapter]?.size ?? 1, 1);
    return Math.min(1, (before + ((page + 1) / pages) * current) / totalSize);
  });

  /** Entrada del índice del capítulo actual (la última que apunta a él o antes). */
  const currentToc = $derived.by(() => {
    let found: TocEntry | null = null;
    for (const entry of toc) {
      const index = spine.findIndex((item) => item.href === entry.href);
      if (index !== -1 && index <= chapter) found = entry;
    }
    return found;
  });

  // ---------- Apertura y progreso ----------

  $effect(() => {
    const id = book.id;
    let cancelled = false;
    // Sin seguimiento: `load` lee `loaded` y reabriría el libro al cambiar.
    untrack(() => void epubStyle.load());
    api
      .openBook(id)
      .then(async (opened) => {
        if (cancelled) return;
        if (opened.content.type !== "epub") throw new Error("El libro no es un EPUB.");
        spine = opened.content.spine;
        toc = opened.content.toc;
        const locator = opened.progress?.locator;
        const start = locator?.type === "epub" ? locator : { spine: 0, position: 0 };
        await frameReady();
        if (cancelled) return;
        const doc = frameDoc();
        if (doc) attachHandlers(doc);
        await loadChapter(Math.min(start.spine, spine.length - 1), { position: start.position });
        ready = true;
      })
      .catch((error) => {
        toasts.show("error", `No se pudo abrir «${book.title}»`, errorMessage(error));
        nav.closeReader();
      });
    return () => {
      cancelled = true;
      void api.closeBook(id).catch(() => {});
    };
  });

  let pendingSave: (() => void) | null = null;
  $effect(() => {
    if (!ready) return;
    const id = book.id;
    const locator = { type: "epub" as const, spine: chapter, position: pages > 1 ? page / pages : 0 };
    const fraction = percent;
    const save = () => void api.saveProgress(id, locator, fraction).catch(() => {});
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

  // ---------- Documento del iframe ----------

  function frameDoc(): Document | null {
    return frame?.contentDocument ?? null;
  }

  /** Espera a que el iframe tenga su documento vacío (srcdoc) cargado. */
  function frameReady(): Promise<void> {
    return new Promise((resolve) => {
      const check = () => {
        const doc = frameDoc();
        if (doc && doc.readyState === "complete" && doc.body) resolve();
        else setTimeout(check, 16);
      };
      check();
    });
  }

  async function fetchChapter(href: string): Promise<Document> {
    const response = await fetch(epubUrl(book, href));
    if (!response.ok) throw new Error(await response.text());
    const text = await response.text();
    const type = response.headers.get("content-type") ?? "";
    const parser = new DOMParser();
    if (!type.includes("text/html")) {
      const xml = parser.parseFromString(text, "application/xhtml+xml");
      if (!xml.querySelector("parsererror")) return xml;
    }
    // XHTML mal formado (frecuente) o HTML: el analizador HTML es tolerante.
    return parser.parseFromString(text, "text/html");
  }

  /** Elimina todo lo que pueda ejecutar código o salir del libro. */
  function sanitize(doc: Document) {
    doc.querySelectorAll(BLOCKED).forEach((element) => element.remove());
    for (const element of doc.querySelectorAll("*")) {
      for (const attribute of [...element.attributes]) {
        const name = attribute.name.toLowerCase();
        const value = attribute.value.trim().toLowerCase();
        if (name.startsWith("on")) element.removeAttribute(attribute.name);
        else if ((name === "href" || name.endsWith(":href") || name === "src") && value.startsWith("javascript:"))
          element.removeAttribute(attribute.name);
      }
    }
  }

  function ensureHead(doc: Document): Element {
    const root = doc.documentElement;
    let head = root.querySelector(":scope > head");
    if (!head) {
      head = doc.createElementNS(root.namespaceURI, "head");
      root.insertBefore(head, root.firstChild);
    }
    return head;
  }

  /** Espera a las hojas de estilo y fuentes, con un tope para no bloquear la lectura. */
  async function settle(doc: Document) {
    const sheets = [...doc.querySelectorAll<HTMLLinkElement>('link[rel~="stylesheet"]')].map(
      (link) =>
        link.sheet
          ? Promise.resolve()
          : new Promise<void>((resolve) => {
              link.addEventListener("load", () => resolve(), { once: true });
              link.addEventListener("error", () => resolve(), { once: true });
            }),
    );
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 1500));
    await Promise.race([Promise.all(sheets), timeout]);
    await Promise.race([doc.fonts.ready, timeout]);
  }

  async function loadChapter(
    index: number,
    target: { position?: number; fragment?: string | null; last?: boolean } = {},
  ) {
    const doc = frameDoc();
    const item = spine[index];
    if (!doc || !item) return;
    loading = true;
    try {
      const source = await fetchChapter(item.href);
      sanitize(source);
      const head = ensureHead(source);
      const base = source.createElementNS(source.documentElement.namespaceURI, "base") as HTMLBaseElement;
      const directory = item.href.includes("/") ? item.href.slice(0, item.href.lastIndexOf("/") + 1) : "";
      base.setAttribute("href", epubBase(book) + directory.split("/").map(encodeURIComponent).join("/"));
      head.insertBefore(base, head.firstChild);

      const imported = doc.importNode(source.documentElement, true);
      doc.replaceChild(imported, doc.documentElement);
      applyStyle(doc);
      chapter = index;
      await settle(doc);
      paginate();

      if (target.last) page = pages - 1;
      else if (target.fragment) page = pageOfFragment(target.fragment);
      else page = Math.min(pages - 1, Math.floor((target.position ?? 0) * pages + 1e-6));
      scrollToPage();
    } catch (error) {
      toasts.show("error", "No se pudo mostrar el capítulo", errorMessage(error));
    } finally {
      loading = false;
    }
  }

  function styleSheet(current: EpubStyle): string {
    const margin = Math.round(current.margin);
    // n columnas por página: ancho = (W − 2·n·M) / n y hueco 2·M, de modo que el
    // paso entre páginas es exactamente el ancho de la ventana (W).
    const columns = pageWidth() >= TWO_COLUMNS_FROM ? 2 : 1;
    const font =
      current.font === "original"
        ? ""
        : `body, body *:not(code):not(pre):not(kbd):not(samp) { font-family: ${FONT_STACKS[current.font]} !important; }`;
    return `
      html {
        height: 100% !important; margin: 0 !important; padding: 0 !important;
        overflow: hidden !important; font-size: ${current.fontSize}% !important;
        background: ${colors.bg} !important;
      }
      body {
        margin: 0 !important; padding: 0 ${margin}px !important; box-sizing: border-box !important;
        height: 100vh !important; max-width: none !important; min-width: 0 !important;
        column-width: calc((100vw - ${margin * 2 * columns}px) / ${columns}) !important;
        column-gap: ${margin * 2}px !important; column-fill: auto !important;
        background: ${colors.bg} !important; color: ${colors.fg} !important;
        line-height: ${current.lineHeight} !important;
        overflow-wrap: break-word; hyphens: auto; -webkit-font-smoothing: antialiased;
      }
      body p, body li, body div, body blockquote, body dd { line-height: ${current.lineHeight} !important; }
      body *:not(svg):not(svg *) { color: inherit !important; background-color: transparent !important; }
      body a:link, body a:visited, body a:link *, body a:visited * { color: ${colors.link} !important; }
      img, svg, video, picture {
        max-width: 100% !important; max-height: calc(100vh - 2rem) !important;
        height: auto; object-fit: contain; break-inside: avoid;
      }
      pre { white-space: pre-wrap !important; }
      table { max-width: 100%; }
      ::selection { background: rgba(124, 92, 255, 0.35); }
      ${font}
    `;
  }

  function applyStyle(doc: Document) {
    const head = doc.head ?? doc.documentElement;
    let element = doc.getElementById(STYLE_ID);
    if (!element) {
      element = doc.createElement("style");
      element.id = STYLE_ID;
    }
    // Siempre al final, para tener prioridad sobre las hojas del libro.
    if (head.lastElementChild !== element) head.appendChild(element);
    const css = styleSheet(style);
    // Reescribir un <style> dispara su evento `load`: solo si cambia.
    if (element.textContent !== css) element.textContent = css;
  }

  // ---------- Paginación ----------

  function pageWidth(): number {
    return frame?.contentWindow?.innerWidth ?? 1;
  }

  function paginate() {
    const doc = frameDoc();
    const scroller = doc?.scrollingElement;
    if (!scroller) return;
    const width = pageWidth();
    pages = Math.max(1, Math.ceil((scroller.scrollWidth - 2) / width));
    page = Math.min(page, pages - 1);
  }

  function scrollToPage() {
    const scroller = frameDoc()?.scrollingElement;
    if (scroller) scroller.scrollLeft = page * pageWidth();
  }

  function pageOfFragment(fragment: string): number {
    const doc = frameDoc();
    const element = doc?.getElementById(fragment) ?? doc?.getElementsByName(fragment)[0];
    const scroller = doc?.scrollingElement;
    if (!element || !scroller) return 0;
    const left = element.getBoundingClientRect().left + scroller.scrollLeft;
    return Math.min(pages - 1, Math.max(0, Math.floor(left / pageWidth())));
  }

  /** Recalcula páginas conservando la posición relativa (cambio de tamaño o de estilo). */
  function relayout() {
    const doc = frameDoc();
    if (!doc || !ready) return;
    const position = pages > 1 ? page / pages : 0;
    applyStyle(doc);
    requestAnimationFrame(() => {
      paginate();
      page = Math.min(pages - 1, Math.floor(position * pages + 1e-6));
      scrollToPage();
    });
  }

  /** Agrupa varias repaginaciones seguidas (p. ej. muchas imágenes) en una. */
  let relayoutTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleRelayout() {
    clearTimeout(relayoutTimer);
    relayoutTimer = setTimeout(relayout, 80);
  }

  // Solo depende del estilo: `relayout` lee y escribe `page`/`pages`, y sin
  // `untrack` el efecto se volvería a disparar indefinidamente.
  $effect(() => {
    void style.theme;
    void style.font;
    void style.fontSize;
    void style.lineHeight;
    void style.margin;
    untrack(relayout);
  });

  $effect(() => {
    if (!frame) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(relayout, 120);
    });
    observer.observe(frame);
    return () => {
      observer.disconnect();
      clearTimeout(timer);
      clearTimeout(relayoutTimer);
    };
  });

  // ---------- Navegación ----------

  async function next() {
    if (loading) return;
    if (page < pages - 1) {
      page += 1;
      scrollToPage();
    } else if (chapter < spine.length - 1) {
      await loadChapter(chapter + 1);
    }
  }

  async function previous() {
    if (loading) return;
    if (page > 0) {
      page -= 1;
      scrollToPage();
    } else if (chapter > 0) {
      await loadChapter(chapter - 1, { last: true });
    }
  }

  async function goToHref(href: string, fragment: string | null) {
    const index = spine.findIndex((item) => item.href === href);
    if (index === -1) return;
    panel = null;
    if (index === chapter) {
      page = fragment ? pageOfFragment(fragment) : 0;
      scrollToPage();
    } else {
      await loadChapter(index, { fragment });
    }
  }

  /** Enlace pulsado dentro del libro: navegación interna o bloqueo si es externo. */
  function onLinkClick(event: MouseEvent, anchor: HTMLAnchorElement) {
    event.preventDefault();
    const raw = anchor.getAttribute("href") ?? anchor.getAttribute("xlink:href");
    if (!raw) return;
    const url = new URL(raw, anchor.baseURI);
    const prefix = new URL(epubBase(book)).pathname;
    if (url.origin !== new URL(epubBase(book)).origin || !url.pathname.startsWith(prefix)) {
      toasts.show("info", "Enlace externo", "Los enlaces a internet no se abren desde el lector.");
      return;
    }
    const href = url.pathname.slice(prefix.length).split("/").map(decodeURIComponent).join("/");
    void goToHref(href, url.hash ? decodeURIComponent(url.hash.slice(1)) : null);
  }

  let wheelLocked = false;
  /**
   * Se registra una sola vez: los capítulos sustituyen el `documentElement`, pero
   * el `Document` del iframe es siempre el mismo.
   */
  function attachHandlers(doc: Document) {
    doc.addEventListener("keydown", onKeydown);
    doc.addEventListener("mousemove", showChrome);
    doc.addEventListener(
      "load",
      (event) => {
        // Las imágenes cambian el tamaño del contenido: se repagina al cargarlas.
        // Otros elementos (como <style>) también emiten `load` y no deben hacerlo.
        const target = event.target;
        if (target instanceof Element && (target.localName === "img" || target.localName === "image"))
          scheduleRelayout();
      },
      true,
    );
    doc.addEventListener(
      "wheel",
      (event) => {
        event.preventDefault();
        if (wheelLocked || Math.abs(event.deltaY) < 4) return;
        wheelLocked = true;
        setTimeout(() => (wheelLocked = false), WHEEL_LOCK_MS);
        void (event.deltaY > 0 ? next() : previous());
      },
      { passive: false },
    );
    doc.addEventListener("click", (event) => {
      const target = event.target as Element | null;
      const anchor = target?.closest?.("a[href], a[*|href]") as HTMLAnchorElement | null;
      if (anchor) {
        onLinkClick(event, anchor);
        return;
      }
      if (doc.getSelection()?.toString()) return;
      const x = event.clientX / pageWidth();
      if (x < 0.25) void previous();
      else if (x > 0.75) void next();
      else toggleChrome();
    });
  }

  // ---------- Interfaz ----------

  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let pointerOverChrome = false;
  function showChrome() {
    chromeVisible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (!pointerOverChrome && panel === null) chromeVisible = false;
    }, CHROME_HIDE_MS);
  }

  function toggleChrome() {
    if (chromeVisible) {
      clearTimeout(hideTimer);
      chromeVisible = false;
      panel = null;
    } else showChrome();
  }

  $effect(() => {
    showChrome();
    return () => clearTimeout(hideTimer);
  });

  const appWindow = getCurrentWindow();
  async function toggleFullscreen() {
    const target = !nav.fullscreen;
    try {
      await appWindow.setFullscreen(target);
      nav.fullscreen = target;
    } catch (error) {
      toasts.show("error", "No se pudo cambiar a pantalla completa", errorMessage(error));
    }
  }

  async function back() {
    if (nav.fullscreen) await toggleFullscreen();
    nav.closeReader();
  }

  function onKeydown(event: KeyboardEvent) {
    const key = event.key;
    if (key === "ArrowRight" || key === "PageDown" || (key === " " && !event.shiftKey)) void next();
    else if (key === "ArrowLeft" || key === "PageUp" || (key === " " && event.shiftKey)) void previous();
    else if (key === "Home") void loadChapter(0);
    else if (key === "End") void loadChapter(spine.length - 1, { last: true });
    else if (key === "F11") void toggleFullscreen();
    else if (key === "Escape") {
      if (panel) panel = null;
      else void (nav.fullscreen ? toggleFullscreen() : back());
    } else return;
    event.preventDefault();
  }

  function togglePanel(which: "toc" | "style") {
    panel = panel === which ? null : which;
    showChrome();
  }

  const percentLabel = $derived(`${Math.round(percent * 100)} %`);
  const srcdoc = "<!DOCTYPE html><html><head></head><body></body></html>";
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="reader theme-{style.theme}"
  class:chrome-hidden={!chromeVisible}
  style:--page-bg={colors.bg}
  style:--page-fg={colors.fg}
  onmousemove={showChrome}
  role="application"
  aria-label="Lector de libros"
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
      <span>{currentToc?.title ?? book.author ?? ""}</span>
    </div>
    <button
      class="tool"
      class:selected={panel === "toc"}
      onclick={() => togglePanel("toc")}
      aria-label="Índice"
      title="Índice"
      disabled={toc.length === 0}
    >
      <span class="icon" aria-hidden="true">&#xE8FD;</span>
    </button>
    <button
      class="tool"
      class:selected={panel === "style"}
      onclick={() => togglePanel("style")}
      aria-label="Aspecto del texto"
      title="Aspecto del texto"
    >
      <span class="text-icon" aria-hidden="true">Aa</span>
    </button>
    <button
      class="tool"
      onclick={toggleFullscreen}
      aria-label={nav.fullscreen ? "Salir de pantalla completa" : "Pantalla completa"}
      title="Pantalla completa (F11)"
    >
      <span class="icon" aria-hidden="true">{nav.fullscreen ? "\uE73F" : "\uE740"}</span>
    </button>
  </div>

  {#if panel === "toc"}
    <div class="panel toc" role="dialog" aria-label="Índice">
      <h2>Índice</h2>
      <ul>
        {#each toc as entry, index (index)}
          <li>
            <button
              class:current={entry === currentToc}
              style:padding-left="{12 + entry.level * 16}px"
              onclick={() => goToHref(entry.href, entry.fragment)}
            >
              {entry.title}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {:else if panel === "style"}
    <div class="panel style" role="dialog" aria-label="Aspecto del texto">
      <h2>Aspecto</h2>
      <div class="row themes" role="radiogroup" aria-label="Tema">
        {#each themes as theme (theme.value)}
          <button
            role="radio"
            aria-checked={style.theme === theme.value}
            class="swatch swatch-{theme.value}"
            class:selected={style.theme === theme.value}
            onclick={() => epubStyle.update({ theme: theme.value })}
          >
            {theme.label}
          </button>
        {/each}
      </div>

      <label class="row">
        <span>Fuente</span>
        <select
          value={style.font}
          onchange={(e) => epubStyle.update({ font: (e.currentTarget as HTMLSelectElement).value as EpubStyle["font"] })}
        >
          <option value="original">Del libro</option>
          <option value="serif">Serif (Georgia)</option>
          <option value="sans">Sans (Segoe UI)</option>
        </select>
      </label>

      <div class="row">
        <span>Tamaño</span>
        <div class="stepper">
          <button
            aria-label="Reducir texto"
            disabled={style.fontSize <= FONT_SIZE.min}
            onclick={() => epubStyle.update({ fontSize: style.fontSize - FONT_SIZE.step })}>A−</button
          >
          <output>{style.fontSize} %</output>
          <button
            aria-label="Ampliar texto"
            disabled={style.fontSize >= FONT_SIZE.max}
            onclick={() => epubStyle.update({ fontSize: style.fontSize + FONT_SIZE.step })}>A+</button
          >
        </div>
      </div>

      <div class="row">
        <span>Interlineado</span>
        <div class="stepper">
          <button
            aria-label="Reducir interlineado"
            disabled={style.lineHeight <= LINE_HEIGHT.min + 1e-9}
            onclick={() => epubStyle.update({ lineHeight: Math.round((style.lineHeight - LINE_HEIGHT.step) * 10) / 10 })}
            >−</button
          >
          <output>{style.lineHeight.toFixed(1)}</output>
          <button
            aria-label="Aumentar interlineado"
            disabled={style.lineHeight >= LINE_HEIGHT.max - 1e-9}
            onclick={() => epubStyle.update({ lineHeight: Math.round((style.lineHeight + LINE_HEIGHT.step) * 10) / 10 })}
            >+</button
          >
        </div>
      </div>

      <div class="row" role="radiogroup" aria-label="Márgenes">
        <span>Márgenes</span>
        <div class="segmented">
          {#each MARGINS as option (option.value)}
            <button
              role="radio"
              aria-checked={style.margin === option.value}
              class:selected={style.margin === option.value}
              onclick={() => epubStyle.update({ margin: option.value })}>{option.label}</button
            >
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <div class="page-area">
    <iframe
      bind:this={frame}
      title="Contenido de {book.title}"
      sandbox="allow-same-origin"
      {srcdoc}
    ></iframe>
    {#if loading}
      <div class="status"><span class="spinner" aria-hidden="true"></span></div>
    {/if}
  </div>

  <div
    class="footer"
    role="toolbar"
    tabindex="-1"
    aria-label="Navegación"
    onmouseenter={() => (pointerOverChrome = true)}
    onmouseleave={() => (pointerOverChrome = false)}
  >
    <button class="tool" onclick={previous} aria-label="Página anterior">
      <span class="icon" aria-hidden="true">&#xE76B;</span>
    </button>
    <div class="progress" aria-hidden="true"><span style:width="{percent * 100}%"></span></div>
    <button class="tool" onclick={next} aria-label="Página siguiente">
      <span class="icon" aria-hidden="true">&#xE76C;</span>
    </button>
    <span class="counter" title="Página {page + 1} de {pages} en este capítulo">
      {page + 1}/{pages} · {percentLabel}
    </span>
  </div>
</div>

<style>
  .reader {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--page-bg);
    color: var(--page-fg);
    --chrome-bg: color-mix(in srgb, var(--page-bg) 92%, var(--page-fg) 8%);
    --chrome-hover: color-mix(in srgb, var(--page-fg) 10%, transparent);
    --chrome-line: color-mix(in srgb, var(--page-fg) 12%, transparent);
    --chrome-muted: color-mix(in srgb, var(--page-fg) 60%, transparent);
  }

  .toolbar,
  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: var(--chrome-bg);
    flex-shrink: 0;
    transition: opacity 200ms var(--ease);
  }

  .toolbar {
    border-bottom: 1px solid var(--chrome-line);
  }

  .footer {
    border-top: 1px solid var(--chrome-line);
  }

  /* La barra se atenúa pero conserva su espacio: así la paginación no cambia al ocultarla. */
  .chrome-hidden .toolbar,
  .chrome-hidden .footer {
    opacity: 0;
  }

  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 18px;
  }

  .title strong,
  .title span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title span {
    font-size: 12px;
    color: var(--chrome-muted);
  }

  .tool {
    min-width: 36px;
    height: 34px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font-size: 14px;
  }

  .tool:hover {
    background: var(--chrome-hover);
  }

  .tool.selected {
    background: var(--chrome-hover);
    color: var(--accent);
  }

  .tool:disabled {
    opacity: 0.35;
  }

  .text-icon {
    font-family: Georgia, serif;
    font-weight: 600;
  }

  .page-area {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 28px 0;
  }

  iframe {
    display: block;
    width: 100%;
    height: 100%;
    border: none;
    background: var(--page-bg);
    color-scheme: normal;
  }

  .status {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    background: var(--page-bg);
  }

  .spinner {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid var(--chrome-line);
    border-top-color: var(--accent);
    animation: spin 800ms linear infinite;
  }

  .progress {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--chrome-line);
    overflow: hidden;
  }

  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .counter {
    min-width: 96px;
    text-align: right;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--chrome-muted);
  }

  .panel {
    position: absolute;
    top: 48px;
    right: 12px;
    z-index: 3;
    width: 340px;
    max-height: calc(100% - 110px);
    overflow-y: auto;
    padding: 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--chrome-line);
    background: var(--chrome-bg);
    color: var(--page-fg);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.22);
  }

  .panel h2 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 600;
  }

  .toc ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .toc button {
    width: 100%;
    padding: 7px 12px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    text-align: left;
    line-height: 18px;
  }

  .toc button:hover {
    background: var(--chrome-hover);
  }

  .toc button.current {
    color: var(--accent);
    font-weight: 600;
  }

  .style .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 12px;
  }

  .themes {
    display: grid !important;
    grid-template-columns: repeat(3, 1fr);
  }

  .swatch {
    height: 44px;
    border-radius: var(--radius-md);
    border: 2px solid transparent;
    font-size: 13px;
  }

  .swatch.selected {
    border-color: var(--accent);
  }

  .swatch-paper {
    background: #fbfaf7;
    color: #1f1f1f;
  }

  .swatch-sepia {
    background: #f4ecd8;
    color: #4a3b28;
  }

  .swatch-dark {
    background: #1b1b1f;
    color: #dcdcdc;
  }

  .style select,
  .stepper button,
  .segmented button {
    height: 30px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--chrome-line);
    background: transparent;
    color: inherit;
  }

  .style select option {
    color: #000;
  }

  .stepper,
  .segmented {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .stepper output {
    min-width: 52px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .stepper button:disabled {
    opacity: 0.35;
  }

  .segmented button.selected {
    border-color: var(--accent);
    color: var(--accent);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
