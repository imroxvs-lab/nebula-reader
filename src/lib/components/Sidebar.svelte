<script lang="ts">
  import { collections } from "#lib/stores/collections.svelte.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { nav, type Section } from "#lib/stores/nav.svelte.ts";
  import { theme } from "#lib/stores/theme.svelte.ts";
  import type { ThemePreference } from "#lib/types.ts";
  import ImportMenu from "./ImportMenu.svelte";
  import PromptDialog from "./PromptDialog.svelte";

  let creating = $state(false);

  const items: { section: Exclude<Section, "collection" | "settings">; label: string; glyph: string; count?: () => number }[] = [
    { section: "home", label: "Inicio", glyph: "\uE80F" },
    { section: "all", label: "Biblioteca", glyph: "\uE8F1", count: () => library.total },
    { section: "books", label: "Libros", glyph: "\uE736", count: () => library.stats.books },
    { section: "comics", label: "Cómics", glyph: "\uE8B9", count: () => library.stats.comics },
  ];

  const themes: { value: ThemePreference; label: string; glyph: string }[] = [
    { value: "system", label: "Tema del sistema", glyph: "\uE770" },
    { value: "light", label: "Tema claro", glyph: "\uE706" },
    { value: "dark", label: "Tema oscuro", glyph: "\uE708" },
  ];
</script>

<aside class="sidebar">
  <div class="primary">
    <ImportMenu />
  </div>

  <nav aria-label="Secciones">
    {#each items as item (item.section)}
      <button
        class="nav-item"
        class:selected={nav.section === item.section}
        aria-current={nav.section === item.section ? "page" : undefined}
        onclick={() => nav.go(item.section)}
      >
        <span class="icon" aria-hidden="true">{item.glyph}</span>
        <span class="label">{item.label}</span>
        {#if item.count && library.statsLoaded}
          <span class="count">{item.count()}</span>
        {/if}
      </button>
    {/each}

    <div class="group-head">
      <h2 id="collections-head">Colecciones</h2>
      <button class="add" aria-label="Nueva colección" title="Nueva colección" onclick={() => (creating = true)}>
        <span class="icon" aria-hidden="true">&#xE710;</span>
      </button>
    </div>
    <ul class="collections" aria-labelledby="collections-head">
      {#each collections.items as collection (collection.id)}
        {@const selected = nav.section === "collection" && nav.collection?.id === collection.id}
        <li>
          <button
            class="nav-item"
            class:selected
            aria-current={selected ? "page" : undefined}
            onclick={() => nav.goToCollection(collection)}
          >
            <span class="icon" aria-hidden="true">&#xE8B7;</span>
            <span class="label">{collection.name}</span>
            <span class="count">{collection.count}</span>
          </button>
        </li>
      {:else}
        <li class="hint">Agrupa tus libros: clic derecho en uno → «Añadir a colección».</li>
      {/each}
    </ul>
  </nav>

  <div class="footer">
    <button
      class="nav-item settings"
      class:selected={nav.section === "settings"}
      aria-current={nav.section === "settings" ? "page" : undefined}
      onclick={() => nav.go("settings")}
    >
      <span class="icon" aria-hidden="true">&#xE713;</span>
      <span class="label">Ajustes</span>
    </button>
    <div class="theme" role="radiogroup" aria-label="Tema">
      {#each themes as option (option.value)}
        <button
          role="radio"
          aria-checked={theme.preference === option.value}
          aria-label={option.label}
          title={option.label}
          class:selected={theme.preference === option.value}
          onclick={() => theme.set(option.value)}
        >
          <span class="icon" aria-hidden="true">{option.glyph}</span>
        </button>
      {/each}
    </div>
  </div>
</aside>

{#if creating}
  <PromptDialog
    title="Nueva colección"
    label="Nombre"
    confirm="Crear"
    onsubmit={(name) => {
      creating = false;
      void collections.create(name).then((created) => created && nav.goToCollection(created));
    }}
    oncancel={() => (creating = false)}
  />
{/if}

<style>
  .sidebar {
    width: var(--sidebar-width);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: 4px 4px 12px;
  }

  .primary {
    padding: 4px 8px 12px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 16px 8px 2px 16px;
  }

  .group-head h2 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .add {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    font-size: 12px;
  }

  .add:hover {
    background: var(--subtle-hover);
  }

  .collections {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .collections .nav-item {
    width: calc(100% - 8px);
  }

  .hint {
    padding: 4px 16px;
    font-size: 12px;
    line-height: 16px;
    color: var(--text-tertiary);
  }

  .settings {
    margin: 0 0 8px;
    width: 100%;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 16px;
    height: 36px;
    padding: 0 12px 0 16px;
    margin: 0 4px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    transition: background var(--duration) var(--ease);
  }

  .nav-item:hover {
    background: var(--subtle-hover);
  }

  .nav-item:active {
    background: var(--subtle-pressed);
  }

  .nav-item.selected {
    background: var(--subtle-hover);
  }

  .nav-item.selected::before {
    content: "";
    position: absolute;
    left: 0;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }

  .nav-item .icon {
    font-size: 16px;
  }

  .label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count {
    font-size: 12px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
  }

  .footer {
    padding: 0 8px;
  }

  .theme {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-md);
    background: var(--subtle-pressed);
    width: fit-content;
  }

  .theme button {
    width: 36px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: transparent;
    font-size: 14px;
    color: var(--text-secondary);
  }

  .theme button:hover {
    background: var(--subtle-hover);
  }

  .theme button.selected {
    background: var(--card-hover);
    color: var(--accent);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }
</style>
