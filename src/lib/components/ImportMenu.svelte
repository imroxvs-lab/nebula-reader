<script lang="ts">
  import { library } from "#lib/stores/library.svelte.ts";

  let { variant = "full" }: { variant?: "full" | "compact" } = $props();
  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  function choose(action: () => Promise<void>) {
    open = false;
    void action();
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (open && root && !root.contains(event.target as Node)) open = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} onkeydown={onKeydown} />

<div class="import" class:compact={variant === "compact"} bind:this={root}>
  <button
    class="btn accent trigger"
    aria-haspopup="menu"
    aria-expanded={open}
    disabled={library.importing}
    onclick={() => (open = !open)}
  >
    <span class="icon" aria-hidden="true">&#xE710;</span>
    <span>{library.importing ? "Añadiendo…" : "Añadir"}</span>
  </button>

  {#if open}
    <div class="menu" role="menu">
      <button role="menuitem" onclick={() => choose(() => library.pickFiles())}>
        <span class="icon" aria-hidden="true">&#xE8E5;</span>
        <span class="text">Archivos…</span>
        <kbd>Ctrl+O</kbd>
      </button>
      <button role="menuitem" onclick={() => choose(() => library.pickFolder())}>
        <span class="icon" aria-hidden="true">&#xE8B7;</span>
        <span class="text">Carpeta…</span>
        <kbd>Ctrl+Mayús+O</kbd>
      </button>
    </div>
  {/if}
</div>

<style>
  .import {
    position: relative;
  }

  .trigger {
    width: 100%;
  }

  .compact .trigger {
    width: auto;
  }

  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 20;
    min-width: 220px;
    padding: 4px;
    border-radius: var(--radius-md);
    border: 1px solid var(--card-stroke);
    background: var(--flyout);
    box-shadow: var(--shadow);
    animation: drop-in var(--duration) var(--ease);
  }

  .menu button {
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
  }

  .menu button:hover {
    background: var(--subtle-hover);
  }

  .text {
    flex: 1;
  }

  kbd {
    font-family: inherit;
    font-size: 11px;
    color: var(--text-tertiary);
  }

  @keyframes drop-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
