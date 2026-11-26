<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "#lib/api.ts";
  import { plural } from "#lib/format.ts";
  import { collections } from "#lib/stores/collections.svelte.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book, BookKind, Collection, LibraryFacets, ReadStatus, SortBy } from "#lib/types.ts";
  import BookGrid from "./BookGrid.svelte";
  import EmptyState from "./EmptyState.svelte";
  import PromptDialog from "./PromptDialog.svelte";

  let {
    kind,
    title,
    collection = null,
  }: { kind: BookKind | null; title: string; collection?: Collection | null } = $props();

  const SEARCH_DEBOUNCE_MS = 200;
  const sortOptions: { value: SortBy; label: string }[] = [
    { value: "addedAt", label: "Añadidos recientemente" },
    { value: "title", label: "Título (A-Z)" },
    { value: "author", label: "Autor" },
    { value: "series", label: "Serie y número" },
    { value: "lastOpened", label: "Leídos recientemente" },
  ];
  const statusOptions: { value: ReadStatus | null; label: string }[] = [
    { value: null, label: "Todos" },
    { value: "unread", label: "Sin leer" },
    { value: "reading", label: "Leyendo" },
    { value: "finished", label: "Terminados" },
  ];

  let searchInput = $state("");
  let search = $state("");
  let sort = $state<SortBy>("addedAt");
  let status = $state<ReadStatus | null>(null);
  let author = $state("");
  let series = $state("");
  let facets = $state<LibraryFacets>({ authors: [], series: [] });
  let books = $state<Book[]>([]);
  let loaded = $state(false);
  let renaming = $state(false);

  // Retrasa la búsqueda mientras se escribe.
  $effect(() => {
    const value = searchInput;
    const timer = setTimeout(() => (search = value), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void library.revision;
    api
      .facets()
      .then((result) => (facets = result))
      .catch(() => {});
  });

  $effect(() => {
    void library.revision;
    const query = {
      kind,
      sort,
      search: search.trim() || null,
      status,
      author: author || null,
      series: series || null,
      collection: collection?.id ?? null,
    };
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

  const filtered = $derived(Boolean(search || status || author || series));
  const emptyLibrary = $derived(library.statsLoaded && library.total === 0 && !collection);
  const noun = $derived(
    kind === "comic" ? ["cómic", "cómics"] : kind === "book" ? ["libro", "libros"] : ["elemento", "elementos"],
  );

  function clearFilters() {
    searchInput = "";
    search = "";
    status = null;
    author = "";
    series = "";
  }

  async function deleteCollection() {
    if (!collection) return;
    const confirmed = await ask(
      `Se borrará la colección «${collection.name}». Los libros seguirán en la biblioteca.`,
      { title: "Borrar colección", kind: "warning", okLabel: "Borrar", cancelLabel: "Cancelar" },
    );
    if (confirmed) await collections.remove(collection);
  }
</script>

<header class="head">
  <div class="heading">
    <h1>{title}</h1>
    {#if loaded && !emptyLibrary}
      <p aria-live="polite">{plural(books.length, noun[0], noun[1])}{filtered ? " con los filtros actuales" : ""}</p>
    {/if}
  </div>
  {#if collection}
    <div class="collection-actions">
      <button class="btn" onclick={() => (renaming = true)}>
        <span class="icon" aria-hidden="true">&#xE8AC;</span> Cambiar nombre
      </button>
      <button class="btn danger" onclick={deleteCollection}>
        <span class="icon" aria-hidden="true">&#xE74D;</span> Borrar colección
      </button>
    </div>
  {/if}
</header>

{#if !emptyLibrary}
  <div class="toolbar" role="search">
    <label class="search">
      <span class="icon" aria-hidden="true">&#xE721;</span>
      <input
        type="search"
        placeholder="Buscar por título, autor o serie"
        aria-label="Buscar por título, autor o serie"
        maxlength="200"
        spellcheck="false"
        autocomplete="off"
        bind:value={searchInput}
      />
    </label>
    <div class="segmented" role="radiogroup" aria-label="Estado de lectura">
      {#each statusOptions as option (option.label)}
        <button
          role="radio"
          aria-checked={status === option.value}
          class:selected={status === option.value}
          onclick={() => (status = option.value)}>{option.label}</button
        >
      {/each}
    </div>
    {#if facets.authors.length > 0}
      <label class="select">
        <span class="visually-hidden">Autor</span>
        <select bind:value={author} aria-label="Filtrar por autor">
          <option value="">Todos los autores</option>
          {#each facets.authors as facet (facet.value)}
            <option value={facet.value}>{facet.value} ({facet.count})</option>
          {/each}
        </select>
      </label>
    {/if}
    {#if facets.series.length > 0}
      <label class="select">
        <span class="visually-hidden">Serie</span>
        <select bind:value={series} aria-label="Filtrar por serie">
          <option value="">Todas las series</option>
          {#each facets.series as facet (facet.value)}
            <option value={facet.value}>{facet.value} ({facet.count})</option>
          {/each}
        </select>
      </label>
    {/if}
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

{#if emptyLibrary}
  <EmptyState />
{:else if loaded && books.length === 0}
  <div class="no-results">
    <span class="icon" aria-hidden="true">{filtered ? "\uE71C" : "\uE8F1"}</span>
    {#if filtered}
      <p>Ningún {noun[0]} cumple los filtros.</p>
      <button class="btn" onclick={clearFilters}>Quitar filtros</button>
    {:else if collection}
      <p>Esta colección está vacía. Añade libros con clic derecho → «Añadir a colección».</p>
    {:else}
      <p>Todavía no hay {noun[1]} en tu biblioteca.</p>
    {/if}
  </div>
{:else}
  <BookGrid {books} {collection} label={title} />
{/if}

{#if renaming && collection}
  {@const current = collection}
  <PromptDialog
    title="Cambiar nombre"
    label="Nombre de la colección"
    initial={current.name}
    confirm="Guardar"
    onsubmit={(name) => {
      renaming = false;
      void collections.rename(current.id, name);
    }}
    oncancel={() => (renaming = false)}
  />
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 16px;
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
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 20px;
  }

  .heading {
    min-width: 0;
  }

  .collection-actions {
    display: flex;
    gap: 8px;
  }

  .btn.danger {
    color: var(--danger);
  }

  .segmented {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-md);
    background: var(--subtle-pressed);
  }

  .segmented button {
    height: 28px;
    padding: 0 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
  }

  .segmented button:hover {
    background: var(--subtle-hover);
  }

  .segmented button.selected {
    background: var(--card-hover);
    color: var(--text);
    font-weight: 600;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }

  .select select {
    max-width: 220px;
    padding-left: 10px;
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
  }

  input:focus {
    outline: none;
  }

  select:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }

  input {
    width: 280px;
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
