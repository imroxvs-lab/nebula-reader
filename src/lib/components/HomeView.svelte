<script lang="ts">
  import { api, errorMessage } from "#lib/api.ts";
  import { greeting, plural } from "#lib/format.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav } from "#lib/stores/nav.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { Book } from "#lib/types.ts";
  import BookGrid from "./BookGrid.svelte";
  import EmptyState from "./EmptyState.svelte";

  const RECENT_LIMIT = 12;
  let recent = $state<Book[]>([]);

  $effect(() => {
    void library.revision;
    api
      .listBooks({ sort: "addedAt", limit: RECENT_LIMIT })
      .then((books) => (recent = books))
      .catch((error) => toasts.show("error", "No se pudo cargar la biblioteca", errorMessage(error)));
  });

  const summary = $derived(
    `${plural(library.stats.books, "libro")} · ${plural(library.stats.comics, "cómic", "cómics")}`,
  );
</script>

<header class="hero">
  <h1>{greeting()}</h1>
  {#if library.statsLoaded && library.total > 0}
    <p>{summary} en tu biblioteca</p>
  {/if}
</header>

{#if library.statsLoaded && library.total === 0}
  <EmptyState />
{:else if recent.length > 0}
  <section>
    <div class="section-head">
      <h2>Añadidos recientemente</h2>
      {#if library.total > recent.length}
        <button class="link" onclick={() => nav.go("all")}>
          Ver todo <span class="icon" aria-hidden="true">&#xE76C;</span>
        </button>
      {/if}
    </div>
    <BookGrid books={recent} />
  </section>
{/if}

<style>
  .hero {
    margin-bottom: 28px;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 28px;
    line-height: 36px;
    font-weight: 600;
  }

  .hero p {
    margin: 4px 0 0;
    color: var(--text-secondary);
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 20px;
    line-height: 28px;
    font-weight: 600;
  }

  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--accent);
  }

  .link:hover {
    background: var(--subtle-hover);
  }

  .link .icon {
    font-size: 10px;
  }
</style>
