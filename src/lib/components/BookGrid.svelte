<script lang="ts">
  import { tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "#lib/api.ts";
  import { collections } from "#lib/stores/collections.svelte.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav } from "#lib/stores/nav.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book, Collection, ReadStatus } from "#lib/types.ts";
  import BookCard from "./BookCard.svelte";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";
  import PromptDialog from "./PromptDialog.svelte";

  let {
    books,
    collection = null,
    label = "Libros",
  }: { books: Book[]; collection?: Collection | null; label?: string } = $props();

  let menu = $state<{ book: Book; x: number; y: number; items: MenuItem[] } | null>(null);
  let creatingFor = $state<Book | null>(null);
  /** Tarjeta que recibe el foco con Tab (navegación «roving tabindex»). */
  let active = $state(0);
  let grid: HTMLDivElement | undefined = $state();

  // ---------- Virtualización ----------
  // Con bibliotecas grandes solo se dibujan las filas visibles (y unas pocas
  // más): con 2000 tarjetas en el DOM el renderizador pasaba de 24 a 119 MB.
  const VIRTUAL_FROM = 150;
  const MIN_CARD = 156;
  const GAP_X = 12;
  const GAP_Y = 8;
  const OVERSCAN_ROWS = 3;

  let width = $state(0);
  let scrollTop = $state(0);
  let viewport = $state(0);
  let gridTop = $state(0);
  let measuredRow = $state(0);
  /** Ancho para el que se midió la fila: solo se vuelve a medir si cambia. */
  let measuredFor = 0;
  let scroller: HTMLElement | null = null;

  const virtual = $derived(books.length > VIRTUAL_FROM);
  // Mismo cálculo que `repeat(auto-fill, minmax(156px, 1fr))`.
  const columns = $derived(Math.max(1, Math.floor((width + GAP_X) / (MIN_CARD + GAP_X))));
  const rowHeight = $derived.by(() => {
    if (measuredRow > 0) return measuredRow + GAP_Y;
    const card = (width - GAP_X * (columns - 1)) / columns;
    return card * 1.5 + 70 + GAP_Y;
  });
  const rows = $derived(Math.ceil(books.length / columns));
  const firstRow = $derived(
    virtual ? Math.max(0, Math.floor((scrollTop - gridTop) / rowHeight) - OVERSCAN_ROWS) : 0,
  );
  const lastRow = $derived(
    virtual
      ? Math.min(rows - 1, Math.ceil((scrollTop + viewport - gridTop) / rowHeight) + OVERSCAN_ROWS)
      : rows - 1,
  );
  const first = $derived(firstRow * columns);
  const visible = $derived(books.slice(first, (lastRow + 1) * columns));
  const activeVisible = $derived(active >= first && active < first + visible.length);

  function measure() {
    if (!grid || !scroller) return;
    scrollTop = scroller.scrollTop;
    viewport = scroller.clientHeight;
    gridTop = grid.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    // La altura de la fila solo se mide cuando cambia el ancho y se toma la
    // máxima de las tarjetas dibujadas. Medir «la primera tarjeta» en cada
    // cambio realimentaba el cálculo: al llegar al final de una lista grande,
    // la altura alternaba entre dos valores y el renderizador se colgaba.
    if (measuredFor !== width && width > 0) {
      const cards = [...grid.querySelectorAll<HTMLElement>(".card")].slice(0, 24);
      const tallest = Math.max(0, ...cards.map((card) => card.offsetHeight));
      if (tallest > 0) {
        measuredRow = tallest;
        measuredFor = width;
      }
    }
  }

  $effect(() => {
    if (!grid) return;
    scroller = grid.closest<HTMLElement>(".content") ?? document.scrollingElement as HTMLElement;
    const target = scroller;
    const onScroll = () => {
      // Se ignoran cambios de menos de un píxel para no realimentar el cálculo.
      if (Math.abs(target.scrollTop - scrollTop) >= 1) scrollTop = target.scrollTop;
    };
    const observer = new ResizeObserver(() => {
      width = grid?.clientWidth ?? 0;
      measure();
    });
    observer.observe(grid);
    observer.observe(target);
    target.addEventListener("scroll", onScroll, { passive: true });
    measure();
    return () => {
      observer.disconnect();
      target.removeEventListener("scroll", onScroll);
    };
  });

  // La altura real de la fila se mide cuando hay tarjetas dibujadas.
  $effect(() => {
    void visible.length;
    void tick().then(measure);
  });

  /** Lleva una tarjeta a la vista (aunque no esté dibujada) y le da el foco. */
  async function focusCard(index: number) {
    active = index;
    if (virtual && scroller) {
      const row = Math.floor(index / columns);
      const top = gridTop + row * rowHeight;
      if (top < scroller.scrollTop) scroller.scrollTop = top;
      else if (top + rowHeight > scroller.scrollTop + scroller.clientHeight)
        scroller.scrollTop = top + rowHeight - scroller.clientHeight;
      scrollTop = scroller.scrollTop;
    }
    await tick();
    const button = grid?.querySelector<HTMLButtonElement>(`button.open[data-index="${index}"]`);
    button?.focus();
    button?.scrollIntoView({ block: "nearest" });
  }

  $effect(() => {
    if (active >= books.length) active = Math.max(0, books.length - 1);
  });

  function openBook(book: Book) {
    nav.openReader(book);
  }

  async function setStatus(book: Book, status: ReadStatus) {
    try {
      await api.setStatus(book.id, status);
      library.refresh();
    } catch (error) {
      toasts.show("error", "No se pudo cambiar el estado", errorMessage(error));
    }
  }

  async function relocate(book: Book) {
    const selected = await open({
      title: `Localizar «${book.title}»`,
      filters: [{ name: book.format.toUpperCase(), extensions: [book.format] }],
    });
    if (typeof selected !== "string") return;
    try {
      await api.relocateBook(book.id, selected);
      toasts.show("success", `Se ha vuelto a encontrar «${book.title}»`);
      library.refresh();
    } catch (error) {
      toasts.show("error", "No se pudo localizar el archivo", errorMessage(error));
    }
  }

  async function showMenu(book: Book, x: number, y: number) {
    const member = new Set(await api.bookCollections(book.id).catch(() => []));
    const items: MenuItem[] = [{ label: "Abrir", glyph: "\uE8E5", action: () => openBook(book) }];
    if (book.status !== "finished")
      items.push({
        label: "Marcar como terminado",
        glyph: "\uE73E",
        separated: true,
        action: () => void setStatus(book, "finished"),
      });
    if (book.status !== "unread")
      items.push({
        label: "Marcar como no leído",
        glyph: "\uE7A7",
        separated: book.status === "finished",
        action: () => void setStatus(book, "unread"),
      });
    items.push({
      label: "Añadir a colección",
      glyph: "\uE8F1",
      submenu: [
        ...collections.items.map((c) => ({
          label: c.name,
          checked: member.has(c.id),
          action: () => void collections.toggle(c, book, member.has(c.id)),
        })),
        {
          label: "Nueva colección…",
          glyph: "\uE710",
          separated: collections.items.length > 0,
          action: () => (creatingFor = book),
        },
      ],
    });
    if (collection)
      items.push({
        label: `Quitar de «${collection.name}»`,
        glyph: "\uE738",
        action: () => void collections.toggle(collection, book, true),
      });
    if (book.missing)
      items.push({ label: "Localizar archivo…", glyph: "\uE721", action: () => void relocate(book) });
    items.push({
      label: "Quitar de la biblioteca",
      glyph: "\uE74D",
      danger: true,
      separated: true,
      action: () => void library.remove(book),
    });
    menu = { book, x, y, items };
  }

  /** Flechas para moverse entre tarjetas; Inicio/Fin al principio y al final. */
  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (!target.classList.contains("open")) return;
    const current = Number(target.dataset.index);
    if (!Number.isFinite(current)) return;
    const deltas: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: columns,
      ArrowUp: -columns,
    };
    let next: number;
    if (event.key in deltas) next = current + deltas[event.key];
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = books.length - 1;
    else return;
    event.preventDefault();
    void focusCard(Math.min(Math.max(next, 0), books.length - 1));
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="grid"
  role="list"
  aria-label={label}
  bind:this={grid}
  onkeydown={onKeydown}
  style:padding-top={virtual ? `${firstRow * rowHeight}px` : null}
  style:padding-bottom={virtual ? `${Math.max(0, rows - lastRow - 1) * rowHeight}px` : null}
>
  {#each visible as book, offset (book.id)}
    {@const index = first + offset}
    <BookCard
      {book}
      {index}
      total={books.length}
      tabbable={index === active || (!activeVisible && offset === 0)}
      onfocus={() => (active = index)}
      onopen={openBook}
      onmenu={(b, x, y) => void showMenu(b, x, y)}
    />
  {/each}
</div>

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    items={menu.items}
    label="Opciones de {menu.book.title}"
    onclose={() => (menu = null)}
  />
{/if}

{#if creatingFor}
  {@const book = creatingFor}
  <PromptDialog
    title="Nueva colección"
    label="Nombre"
    confirm="Crear"
    onsubmit={(name) => {
      creatingFor = null;
      void collections.create(name, [book]);
    }}
    oncancel={() => (creatingFor = null)}
  />
{/if}

<style>
  .grid {
    /* Sin anclaje de desplazamiento: al cambiar los rellenos de la lista
       virtual, Chromium corregía el desplazamiento y provocaba un bucle. */
    overflow-anchor: none;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    gap: 8px 12px;
  }
</style>
