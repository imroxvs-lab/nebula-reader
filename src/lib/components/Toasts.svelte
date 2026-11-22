<script lang="ts">
  import { toasts, type ToastKind } from "#lib/stores/toasts.svelte.ts";

  const glyphs: Record<ToastKind, string> = {
    info: "\uE946",
    success: "\uE73E",
    error: "\uEA39",
    progress: "",
  };
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div class="toast {toast.kind}">
      {#if toast.kind === "progress"}
        <span class="spinner" aria-hidden="true"></span>
      {:else}
        <span class="icon glyph" aria-hidden="true">{glyphs[toast.kind]}</span>
      {/if}
      <div class="body">
        <strong>{toast.message}</strong>
        {#if toast.detail}<span>{toast.detail}</span>{/if}
      </div>
      {#if toast.kind !== "progress"}
        <button aria-label="Cerrar" onclick={() => toasts.dismiss(toast.id)}>
          <span class="icon" aria-hidden="true">&#xE711;</span>
        </button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: min(380px, calc(100vw - 40px));
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 8px 12px 14px;
    border-radius: var(--radius-md);
    border: 1px solid var(--card-stroke);
    background: var(--flyout);
    box-shadow: var(--shadow);
    animation: slide-in 250ms var(--ease);
  }

  .glyph {
    font-size: 16px;
    margin-top: 2px;
  }

  .info .glyph {
    color: var(--accent);
  }

  .success .glyph {
    color: var(--success);
  }

  .error .glyph {
    color: var(--danger);
  }

  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .body span {
    font-size: 12px;
    color: var(--text-secondary);
  }

  button {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    font-size: 10px;
    flex-shrink: 0;
  }

  button:hover {
    background: var(--subtle-hover);
  }

  .spinner {
    width: 16px;
    height: 16px;
    margin-top: 2px;
    border-radius: 50%;
    border: 2px solid var(--accent-soft);
    border-top-color: var(--accent);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }
</style>
