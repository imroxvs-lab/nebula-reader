<script lang="ts">
  import { library } from "#lib/stores/library.svelte.ts";
  import Logo from "./Logo.svelte";
</script>

<section class="empty">
  <div class="art"><Logo size={96} /></div>
  <h2>Tu biblioteca está vacía</h2>
  <p>
    Añade tus libros y cómics para empezar. Nebula Reader admite
    <strong>EPUB</strong>, <strong>PDF</strong>, <strong>CBZ</strong>, <strong>CBR</strong> y
    <strong>CB7</strong>.
  </p>
  <div class="actions">
    <button class="btn accent" disabled={library.importing} onclick={() => library.pickFiles()}>
      <span class="icon" aria-hidden="true">&#xE8E5;</span> Añadir archivos
    </button>
    <button class="btn" disabled={library.importing} onclick={() => library.pickFolder()}>
      <span class="icon" aria-hidden="true">&#xE8B7;</span> Añadir carpeta
    </button>
  </div>
  <p class="hint"><span class="icon" aria-hidden="true">&#xE896;</span> También puedes arrastrarlos a esta ventana.</p>
</section>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 440px;
    margin: 48px auto 0;
  }

  .art {
    margin-bottom: 20px;
    filter: drop-shadow(0 8px 24px rgba(106, 77, 216, 0.35));
  }

  h2 {
    margin: 0 0 8px;
    font-family: var(--font-display);
    font-size: 20px;
    line-height: 28px;
    font-weight: 600;
  }

  p {
    margin: 0;
    color: var(--text-secondary);
  }

  .actions {
    display: flex;
    gap: 8px;
    margin: 24px 0 16px;
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-tertiary);
  }
</style>
