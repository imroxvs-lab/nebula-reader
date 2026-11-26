<script lang="ts">
  import { tick, untrack } from "svelte";

  let {
    title,
    label,
    initial = "",
    confirm = "Aceptar",
    maxlength = 80,
    onsubmit,
    oncancel,
  }: {
    title: string;
    label: string;
    initial?: string;
    confirm?: string;
    maxlength?: number;
    onsubmit: (value: string) => void;
    oncancel: () => void;
  } = $props();

  let value = $state(untrack(() => initial));
  let dialog: HTMLDialogElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();
  const previousFocus = document.activeElement as HTMLElement | null;

  // `showModal` da foco atrapado, fondo inerte y cierre con Esc de forma nativa.
  $effect(() => {
    dialog?.showModal();
    void tick().then(() => input?.select());
    return () => previousFocus?.focus?.();
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const trimmed = value.trim();
    if (trimmed) onsubmit(trimmed);
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="prompt-title"
  oncancel={(e) => {
    e.preventDefault();
    oncancel();
  }}
>
  <form onsubmit={submit}>
    <h2 id="prompt-title">{title}</h2>
    <label>
      <span>{label}</span>
      <input bind:this={input} bind:value {maxlength} spellcheck="false" autocomplete="off" required />
    </label>
    <div class="actions">
      <button type="submit" class="btn accent" disabled={!value.trim()}>{confirm}</button>
      <button type="button" class="btn" onclick={oncancel}>Cancelar</button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    width: min(420px, calc(100vw - 48px));
    padding: 0;
    border: 1px solid var(--card-stroke);
    border-radius: var(--radius-lg);
    background: var(--flyout);
    color: var(--text);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.35);
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.35);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-secondary);
  }

  input {
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-stroke);
    border-bottom: 2px solid var(--accent);
    background: var(--control);
    color: var(--text);
    user-select: text;
    outline: none;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .actions .btn {
    min-width: 96px;
  }
</style>
