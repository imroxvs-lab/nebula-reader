<script lang="ts">
  import DropOverlay from "#lib/components/DropOverlay.svelte";
  import HomeView from "#lib/components/HomeView.svelte";
  import LibraryView from "#lib/components/LibraryView.svelte";
  import Sidebar from "#lib/components/Sidebar.svelte";
  import TitleBar from "#lib/components/TitleBar.svelte";
  import Toasts from "#lib/components/Toasts.svelte";
  import { library } from "#lib/stores/library.svelte.ts";
  import { SECTION_KIND, nav } from "#lib/stores/nav.svelte.ts";
  import { theme } from "#lib/stores/theme.svelte.ts";

  const titles = { all: "Biblioteca", books: "Libros", comics: "Cómics" } as const;

  $effect(() => {
    void theme.init();
    void library.refreshStats();
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.ctrlKey && event.key.toLowerCase() === "o") {
      event.preventDefault();
      void (event.shiftKey ? library.pickFolder() : library.pickFiles());
    }
  }

  // Sin menú contextual del navegador fuera de los campos de texto.
  function onContextMenu(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest("input, textarea")) event.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={onContextMenu} />

<div class="app">
  <TitleBar />
  <div class="body">
    <Sidebar />
    <main class="content">
      {#if nav.section === "home"}
        <HomeView />
      {:else}
        {#key nav.section}
          <LibraryView kind={SECTION_KIND[nav.section]} title={titles[nav.section]} />
        {/key}
      {/if}
    </main>
  </div>
</div>

<DropOverlay />
<Toasts />

<style>
  .app {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 28px 36px 36px;
    background: var(--layer);
    border-top: 1px solid var(--layer-stroke);
    border-left: 1px solid var(--layer-stroke);
    border-top-left-radius: var(--radius-md);
    scrollbar-gutter: stable;
  }
</style>
