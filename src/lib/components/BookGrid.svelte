<script lang="ts">
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav } from "#lib/stores/nav.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book } from "#lib/types.ts";
  import BookCard from "./BookCard.svelte";

  let { books }: { books: Book[] } = $props();

  let menu = $state<{ book: Book; x: number; y: number } | null>(null);

  const UPCOMING: Partial<Record<Book["format"], string>> = {
    cbr: "Los cómics CBR se podrán leer en una próxima versión.",
    cb7: "Los cómics CB7 se podrán leer en una próxima versión.",
  };

  function openBook(book: Book) {
    const upcoming = UPCOMING[book.format];
    if (upcoming) toasts.show("info", `«${book.title}»`, upcoming);
    else nav.openReader(book);
  }

  function showMenu(book: Book, x: number, y: number) {
    const width = 230;
    const height = 48;
    menu = {
      book,
      x: Math.min(x, window.innerWidth - width - 8),
      y: Math.min(y, window.innerHeight - height - 8),
    };
  }

  function remove() {
    if (!menu) return;
    const { book } = menu;
    menu = null;
    void library.remove(book);
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (menu && !(e.target as HTMLElement).closest(".context-menu")) menu = null;
  }}
  onkeydown={(e) => {
    if (e.key === "Escape") menu = null;
  }}
  onblur={() => (menu = null)}
/>

<div class="grid">
  {#each books as book (book.id)}
    <BookCard {book} onopen={openBook} onmenu={showMenu} />
  {/each}
</div>

{#if menu}
  <div class="context-menu" role="menu" style:left="{menu.x}px" style:top="{menu.y}px">
    <button role="menuitem" onclick={remove}>
      <span class="icon" aria-hidden="true">&#xE74D;</span>
      Quitar de la biblioteca
    </button>
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    gap: 8px 12px;
  }

  .context-menu {
    position: fixed;
    z-index: 30;
    min-width: 230px;
    padding: 4px;
    border-radius: var(--radius-md);
    border: 1px solid var(--card-stroke);
    background: var(--flyout);
    box-shadow: var(--shadow);
  }

  .context-menu button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    color: var(--danger);
  }

  .context-menu button:hover {
    background: var(--subtle-hover);
  }
</style>
