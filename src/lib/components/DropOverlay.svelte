<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { library } from "#lib/stores/library.svelte.ts";

  let dragging = $state(false);

  $effect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (payload.type === "enter") dragging = true;
        else if (payload.type === "leave") dragging = false;
        else if (payload.type === "drop") {
          dragging = false;
          void library.importPaths(payload.paths);
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
</script>

{#if dragging}
  <div class="overlay" aria-hidden="true">
    <div class="panel">
      <span class="icon" aria-hidden="true">&#xE896;</span>
      <strong>Suelta para añadir a la biblioteca</strong>
      <span>Archivos EPUB, PDF, CBZ, CBR, CB7 o carpetas</span>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--accent-soft);
    backdrop-filter: blur(4px);
    animation: fade var(--duration) var(--ease);
  }

  .panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 100%;
    justify-content: center;
    border: 2px dashed var(--accent);
    border-radius: var(--radius-lg);
    color: var(--text-secondary);
  }

  .panel .icon {
    font-size: 40px;
    color: var(--accent);
    margin-bottom: 8px;
  }

  strong {
    font-family: var(--font-display);
    font-size: 20px;
    color: var(--text);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
