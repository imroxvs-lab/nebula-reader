<script lang="ts">
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
    if (!target.classList.contains("open") || !grid) return;
    const cards = [...grid.querySelectorAll<HTMLButtonElement>("button.open")];
    const current = cards.indexOf(target as HTMLButtonElement);
    if (current === -1) return;
    // Columnas = tarjetas en la primera fila (se mide en pantalla: `offsetTop`
    // es relativo a la propia tarjeta y no serviría).
    const top = cards[0]?.getBoundingClientRect().top ?? 0;
    const columns = Math.max(
      1,
      cards.filter((card) => Math.abs(card.getBoundingClientRect().top - top) < 2).length,
    );
    const deltas: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: columns,
      ArrowUp: -columns,
    };
    let next: number;
    if (event.key in deltas) next = current + deltas[event.key];
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = cards.length - 1;
    else return;
    event.preventDefault();
    next = Math.min(Math.max(next, 0), cards.length - 1);
    active = next;
    cards[next]?.focus();
    cards[next]?.scrollIntoView({ block: "nearest" });
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="grid" role="list" aria-label={label} bind:this={grid} onkeydown={onKeydown}>
  {#each books as book, index (book.id)}
    <BookCard
      {book}
      tabbable={index === active}
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
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    gap: 8px 12px;
  }
</style>
