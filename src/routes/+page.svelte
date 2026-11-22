<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let version = $state<string | null>(null);

  $effect(() => {
    invoke<string>("app_version")
      .then((v) => (version = v))
      .catch(() => (version = null));
  });
</script>

<main>
  <h1>Nebula Reader</h1>
  <p>Base del proyecto lista.</p>
  {#if version}
    <p class="muted">Núcleo v{version}</p>
  {/if}
</main>

<style>
  :global(:root) {
    font-family: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    color-scheme: light dark;
  }

  main {
    min-height: 100vh;
    display: grid;
    place-content: center;
    text-align: center;
  }

  .muted {
    opacity: 0.6;
  }
</style>
