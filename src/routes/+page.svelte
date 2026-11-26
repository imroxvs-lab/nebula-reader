<script lang="ts">
  import ComicReader from "#lib/components/ComicReader.svelte";
  import EpubReader from "#lib/components/EpubReader.svelte";
  import DropOverlay from "#lib/components/DropOverlay.svelte";
  import HomeView from "#lib/components/HomeView.svelte";
  import LibraryView from "#lib/components/LibraryView.svelte";
  import SettingsView from "#lib/components/SettingsView.svelte";
  import Sidebar from "#lib/components/Sidebar.svelte";
  import TitleBar from "#lib/components/TitleBar.svelte";
  import Toasts from "#lib/components/Toasts.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorMessage } from "#lib/api.ts";
  import { plural } from "#lib/format.ts";
  import { collections } from "#lib/stores/collections.svelte.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { prefs } from "#lib/stores/prefs.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import { SECTION_KIND, nav } from "#lib/stores/nav.svelte.ts";
  import { theme } from "#lib/stores/theme.svelte.ts";

  const titles = { all: "Biblioteca", books: "Libros", comics: "Cómics" } as const;

  $effect(() => {
    void theme.init();
    void library.refreshStats();
    void prefs.load().then(async () => {
      // Archivo con el que se abrió la app (doble clic, «Abrir con»…).
      await openPaths(await api.startupFiles().catch(() => []));
      if (prefs.rescanOnStart) void rescanOnStart();
    });
  });

  // Archivos abiertos con la app ya en marcha (los reenvía la nueva instancia).
  $effect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen<string[]>("open-files", (event) => void openPaths(event.payload)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  /** Añade los archivos a la biblioteca si hace falta y abre el último. */
  async function openPaths(paths: string[]) {
    let last = null;
    for (const path of paths) {
      try {
        last = await api.bookForPath(path);
      } catch (error) {
        toasts.show("error", "No se pudo abrir el archivo", errorMessage(error));
      }
    }
    if (last) {
      library.refresh();
      nav.openReader(last);
    }
  }

  // Los contadores de las colecciones cambian al importar o quitar libros.
  $effect(() => {
    void library.revision;
    void collections.refresh();
  });

  /** Reescaneo silencioso al arrancar: solo avisa si ha cambiado algo. */
  async function rescanOnStart() {
    try {
      const report = await api.rescan();
      const changes = [
        report.added > 0 && plural(report.added, "nuevo", "nuevos"),
        report.relocated > 0 && plural(report.relocated, "movido", "movidos"),
        report.missing > 0 && plural(report.missing, "no encontrado", "no encontrados"),
      ].filter(Boolean);
      if (changes.length > 0) {
        toasts.show("info", "Biblioteca actualizada", changes.join(" · "));
        library.refresh();
      }
    } catch (error) {
      toasts.show("error", "No se pudo revisar la biblioteca", errorMessage(error));
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (nav.reading) return;
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
  {#if !nav.fullscreen}
    <TitleBar title={nav.reading?.title ?? null} />
  {/if}
  {#if nav.reading}
    {#key nav.reading.id}
      {#if nav.reading.format === "epub"}
        <EpubReader book={nav.reading} />
      {:else}
        <ComicReader book={nav.reading} />
      {/if}
    {/key}
  {:else}
  <div class="body">
    <Sidebar />
    <main class="content">
      {#if nav.section === "home"}
        <HomeView />
      {:else if nav.section === "settings"}
        <SettingsView />
      {:else if nav.section === "collection" && nav.collection}
        {#key nav.collection.id}
          <LibraryView kind={null} title={nav.collection.name} collection={nav.collection} />
        {/key}
      {:else if nav.section !== "collection"}
        {#key nav.section}
          <LibraryView kind={SECTION_KIND[nav.section]} title={titles[nav.section]} />
        {/key}
      {/if}
    </main>
  </div>
  {/if}
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
    overflow-anchor: none;
  }
</style>
