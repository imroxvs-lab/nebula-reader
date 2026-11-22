<script lang="ts">
  import { api, errorMessage } from "#lib/api.ts";
  import { plural } from "#lib/format.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book, BookKind, SortBy } from "#lib/types.ts";
  import BookGrid from "./BookGrid.svelte";
  import EmptyState from "./EmptyState.svelte";

  let { kind, title }: { kind: BookKind | null; title: string } = $props();

  const SEARCH_DEBOUNCE_MS = 200;
  const sortOptions: { value: SortBy; label: string }[] = [
    { value: "addedAt", label: "Añadidos recientemente" },
    { value: "title", label: "Título (A-Z)" },
    { value: "lastOpened", label: "Leídos recientemente" },
  ];

  let searchInput = $state("");
  let search = $state("");
  let sort = $state<SortBy>("addedAt");
  let books = $state<Book[]>([]);
  let loaded = $state(false);

  // Retrasa la búsqueda mientras se escribe.
  $effect(() => {
    const value = searchInput;
    const timer = setTimeout(() => (search = value), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void library.revision;
    const query = { kind, sort, search: search.trim() || null };
    let cancelled = false;
    api
      .listBooks(query)
      .then((result) => {
        if (cancelled) return;
        books = result;
        loaded = true;
      })
      .catch((error) => toasts.show("error", "No se pudo cargar la biblioteca", errorMessage(error)));
    return () => (cancelled = true);
  });

  const emptyLibrary = $derived(library.statsLoaded && library.total === 0);
  const noun = $derived(kind === "comic" ? ["cómic", "cómics"] : kind === "book" ? ["libro", "libros"] : ["elemento", "elementos"]);
</script>

<header class="head">
  <div>
    <h1>{title}</h1>
    {#if loaded && !emptyLibrary}
      <p>{plural(books.length, noun[0], noun[1])}{search ? ` para «${search}»` : ""}</p>
    {/if}
  </div>

  {#if !emptyLibrary}
    <div class="toolbar">
      <label class="search">
        <span class="icon" aria-hidden="true">&#xE721;</span>
        <input
          type="search"
          placeholder="Buscar por título"
          aria-label="Buscar por título"
          maxlength="200"
          spellcheck="false"
          autocomplete="off"
          bind:value={searchInput}
        />
      </label>
      <label class="sort">
        <span class="icon" aria-hidden="true">&#xE8CB;</span>
        <select bind:value={sort} aria-label="Ordenar">
          {#each sortOptions as option (option.value)}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </label>
    </div>
  {/if}
</header>

{#if emptyLibrary}
  <EmptyState />
{:else if loaded && books.length === 0}
  <div class="no-results">
    <span class="icon" aria-hidden="true">&#xE721;</span>
    {#if search}
      <p>No hay resultados para «{search}».</p>
    {:else}
      <p>Todavía no hay {noun[1]} en tu biblioteca.</p>
    {/if}
  </div>
{:else}
  <BookGrid {books} />
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 28px;
    line-height: 36px;
    font-weight: 600;
  }

  .head p {
    margin: 2px 0 0;
    color: var(--text-secondary);
  }

  .toolbar {
    display: flex;
    gap: 8px;
  }

  .search,
  .sort {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search .icon,
  .sort .icon {
    position: absolute;
    left: 10px;
    font-size: 12px;
    color: var(--text-secondary);
    pointer-events: none;
  }

  input,
  select {
    height: 32px;
    padding: 0 10px 0 32px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-stroke);
    border-bottom-color: var(--text-tertiary);
    background: var(--control);
    outline: none;
  }

  input {
    width: 260px;
    user-select: text;
  }

  input:focus {
    border-bottom: 2px solid var(--accent);
    background: var(--card-hover);
  }

  select option {
    background: var(--flyout);
    color: var(--text);
  }

  .no-results {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    margin-top: 64px;
    color: var(--text-secondary);
  }

  .no-results .icon {
    font-size: 32px;
    color: var(--text-tertiary);
  }

  .no-results p {
    margin: 0;
  }
</style>
