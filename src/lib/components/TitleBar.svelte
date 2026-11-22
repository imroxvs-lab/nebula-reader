<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Logo from "./Logo.svelte";

  let { title = null }: { title?: string | null } = $props();

  const appWindow = getCurrentWindow();
  let maximized = $state(false);

  $effect(() => {
    let unlisten: (() => void) | undefined;
    const sync = async () => (maximized = await appWindow.isMaximized());
    void sync();
    appWindow.onResized(sync).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <Logo size={16} />
    <span data-tauri-drag-region>
      {#if title}<span class="book" data-tauri-drag-region>{title}</span>{" · "}{/if}Nebula Reader
    </span>
  </div>

  <div class="controls">
    <button aria-label="Minimizar" title="Minimizar" onclick={() => appWindow.minimize()}>
      <span class="icon" aria-hidden="true">&#xE921;</span>
    </button>
    <button
      aria-label={maximized ? "Restaurar" : "Maximizar"}
      title={maximized ? "Restaurar" : "Maximizar"}
      onclick={() => appWindow.toggleMaximize()}
    >
      <span class="icon" aria-hidden="true">{maximized ? "\uE923" : "\uE922"}</span>
    </button>
    <button class="close" aria-label="Cerrar" title="Cerrar" onclick={() => appWindow.close()}>
      <span class="icon" aria-hidden="true">&#xE8BB;</span>
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: var(--titlebar-height);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-left: 16px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .book {
    color: var(--text);
  }

  .brand :global(svg) {
    pointer-events: none;
  }

  .controls {
    display: flex;
    height: 100%;
    align-self: flex-start;
  }

  .controls button {
    width: 46px;
    height: 32px;
    border: none;
    background: transparent;
    font-size: 10px;
    transition: background var(--duration) var(--ease);
  }

  .controls button:hover {
    background: var(--subtle-hover);
  }

  .controls button:active {
    background: var(--subtle-pressed);
  }

  .controls .close:hover {
    background: var(--close-hover);
    color: #fff;
  }
</style>
