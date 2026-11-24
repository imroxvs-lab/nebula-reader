<script lang="ts">
  import { coverUrl, hasCover } from "#lib/api.ts";
  import { formatPercent, formatSize, hueFromText, kindOf } from "#lib/format.ts";
  import type { Book } from "#lib/types.ts";

  let {
    book,
    onopen,
    onmenu,
  }: {
    book: Book;
    onopen: (book: Book) => void;
    onmenu: (book: Book, x: number, y: number) => void;
  } = $props();

  const hue = $derived(hueFromText(book.title));
  const comic = $derived(kindOf(book.format) === "comic");
  let coverLoaded = $state(false);
  let coverFailed = $state(false);
  const showCover = $derived(hasCover(book) && !coverFailed);
  const subtitle = $derived(
    book.progress !== null
      ? `${formatPercent(book.progress)} leído`
      : (book.author ?? `${comic ? "Cómic" : "Libro"} · ${formatSize(book.fileSize)}`),
  );

  function contextMenu(event: MouseEvent) {
    event.preventDefault();
    onmenu(book, event.clientX, event.clientY);
  }

  function moreButton(event: MouseEvent) {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onmenu(book, rect.left, rect.bottom + 4);
  }
</script>

<!-- Dos botones hermanos (abrir y más opciones): un botón no puede contener a
     otro sin que este quede oculto para los lectores de pantalla. -->
<div class="card" oncontextmenu={contextMenu} role="presentation">
  <button
    class="open"
    title={book.path}
    aria-label="{book.title}, {comic ? 'cómic' : 'libro'} {book.format.toUpperCase()}{book.progress !== null
      ? `, ${formatPercent(book.progress)} leído`
      : ''}"
    onclick={() => onopen(book)}
    onkeydown={(e) => {
      if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
        e.preventDefault();
        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
        onmenu(book, rect.left + 12, rect.top + 12);
      }
    }}
  >
    <span class="cover" class:comic style:--hue={hue} aria-hidden="true">
      <span class="cover-title">{book.title}</span>
      {#if showCover}
        <img
          class="cover-image"
          class:loaded={coverLoaded}
          src={coverUrl(book)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable="false"
          onload={() => (coverLoaded = true)}
          onerror={() => (coverFailed = true)}
        />
      {/if}
      <span class="badge">{book.format.toUpperCase()}</span>
      {#if book.progress !== null}
        <span class="progress"><span style:width="{book.progress * 100}%"></span></span>
      {/if}
    </span>
    <span class="meta" aria-hidden="true">
      <span class="title">{book.title}</span>
      <span class="sub">{subtitle}</span>
    </span>
  </button>
  <button class="more" aria-label="Más opciones de {book.title}" title="Más opciones" onclick={moreButton}>
    <span class="icon" aria-hidden="true">&#xE712;</span>
  </button>
</div>

<style>
  .card {
    position: relative;
    border-radius: var(--radius-md);
    transition: background var(--duration) var(--ease);
    content-visibility: auto;
    contain-intrinsic-size: auto 290px;
  }

  .card:hover {
    background: var(--subtle-hover);
  }

  .card:has(.open:active) {
    background: var(--subtle-pressed);
  }

  .open {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    padding: 8px 8px 10px;
    border: none;
    border-radius: var(--radius-md);
    background: transparent;
    text-align: left;
  }

  .cover,
  .meta,
  .title,
  .sub {
    display: block;
  }

  /*
   * Portada provisional. Se evitan sombras difuminadas permanentes y degradados
   * radiales: en WebView2 cada uno multiplicaba la memoria del proceso GPU
   * (~70 MB y ~45 MB con 21 tarjetas). La sombra solo aparece al pasar el ratón.
   */
  .cover {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius-sm) var(--radius-md) var(--radius-md) var(--radius-sm);
    border: 1px solid rgba(0, 0, 0, 0.12);
    overflow: hidden;
    display: flex;
    align-items: flex-end;
    padding: 14px 12px;
    background:
      linear-gradient(90deg, rgba(0, 0, 0, 0.22) 0 4px, rgba(255, 255, 255, 0.12) 4px 6px, transparent 6px),
      linear-gradient(160deg, hsl(var(--hue) 60% 55%), hsl(calc(var(--hue) + 35) 60% 24%));
    transition: transform var(--duration) var(--ease);
  }

  .cover.comic {
    background:
      linear-gradient(0deg, rgba(0, 0, 0, 0.35), transparent 45%),
      linear-gradient(200deg, hsl(calc(var(--hue) + 40) 85% 62%), hsl(var(--hue) 70% 45%) 45%, hsl(calc(var(--hue) - 40) 65% 26%));
  }

  .card:hover .cover {
    transform: translateY(-2px);
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.2);
  }

  .cover-title {
    color: #fff;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 15px;
    line-height: 19px;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .cover-image {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 200ms var(--ease);
  }

  .cover-image.loaded {
    opacity: 1;
  }

  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: rgba(0, 0, 0, 0.45);
  }

  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .badge {
    position: absolute;
    top: 8px;
    right: 8px;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: #fff;
    /* Sin backdrop-filter: cada uno crea una superficie GPU por tarjeta. */
    background: rgba(0, 0, 0, 0.42);
  }

  .meta {
    min-width: 0;
    padding-right: 24px;
  }

  .title {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sub {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .more {
    position: absolute;
    right: 6px;
    bottom: 12px;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    font-size: 14px;
    opacity: 0;
    transition: opacity var(--duration) var(--ease);
  }

  .card:hover .more,
  .card:focus-within .more {
    opacity: 1;
  }

  .more:hover {
    background: var(--subtle-hover);
  }
</style>
