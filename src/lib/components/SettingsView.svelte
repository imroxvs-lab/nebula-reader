<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "#lib/api.ts";
  import { plural } from "#lib/format.ts";
  import { epubStyle } from "#lib/stores/epubStyle.svelte.ts";
  import { library } from "#lib/stores/library.svelte.ts";
  import { ACCENTS, prefs } from "#lib/stores/prefs.svelte.ts";
  import { theme } from "#lib/stores/theme.svelte.ts";
  import { toasts } from "#lib/stores/toasts.svelte.ts";
  import type { EpubTheme, FitMode, PageLayout, ReadingDirection, RescanReport, ThemePreference } from "#lib/types.ts";

  let version = $state("");
  let rescanning = $state(false);
  let lastReport = $state<RescanReport | null>(null);

  $effect(() => {
    void api.appVersion().then((v) => (version = v));
    void epubStyle.load();
  });

  const themes: { value: ThemePreference; label: string }[] = [
    { value: "system", label: "Usar el del sistema" },
    { value: "light", label: "Claro" },
    { value: "dark", label: "Oscuro" },
  ];
  const fits: { value: FitMode; label: string }[] = [
    { value: "page", label: "Ajustar a la ventana" },
    { value: "width", label: "Ajustar al ancho" },
    { value: "original", label: "Tamaño original" },
  ];
  const layouts: { value: PageLayout; label: string }[] = [
    { value: "single", label: "Una página" },
    { value: "double", label: "Doble página" },
    { value: "webtoon", label: "Desplazamiento vertical (webtoon)" },
  ];
  const directions: { value: ReadingDirection; label: string }[] = [
    { value: "ltr", label: "Occidental (de izquierda a derecha)" },
    { value: "rtl", label: "Manga (de derecha a izquierda)" },
  ];
  const epubThemes: { value: EpubTheme; label: string }[] = [
    { value: "paper", label: "Papel" },
    { value: "sepia", label: "Sepia" },
    { value: "dark", label: "Oscuro" },
  ];

  async function save(key: string, value: string, apply: () => void) {
    apply();
    try {
      await prefs.set(key, value);
    } catch (error) {
      toasts.show("error", "No se pudo guardar el ajuste", errorMessage(error));
    }
  }

  async function addFolder() {
    const selected = await open({ title: "Añadir carpeta a la biblioteca", directory: true, multiple: true });
    if (!selected) return;
    const folders = [...new Set([...prefs.folders, ...selected])];
    try {
      await prefs.setFolders(folders);
      // Se importa en el momento lo que haya en las carpetas nuevas.
      await library.importPaths(selected);
    } catch (error) {
      toasts.show("error", "No se pudo añadir la carpeta", errorMessage(error));
    }
  }

  async function removeFolder(folder: string) {
    try {
      await prefs.setFolders(prefs.folders.filter((f) => f !== folder));
    } catch (error) {
      toasts.show("error", "No se pudo quitar la carpeta", errorMessage(error));
    }
  }

  async function rescan() {
    rescanning = true;
    try {
      lastReport = await api.rescan();
      library.refresh();
    } catch (error) {
      toasts.show("error", "No se pudo revisar la biblioteca", errorMessage(error));
    } finally {
      rescanning = false;
    }
  }

  const reportText = $derived(
    lastReport
      ? [
          `${plural(lastReport.checked, "libro comprobado", "libros comprobados")}`,
          `${plural(lastReport.added, "nuevo", "nuevos")}`,
          lastReport.relocated > 0 && `${plural(lastReport.relocated, "movido", "movidos")} (se conserva su progreso)`,
          lastReport.restored > 0 && `${plural(lastReport.restored, "recuperado", "recuperados")}`,
          `${plural(lastReport.missing, "no encontrado", "no encontrados")}`,
          lastReport.failures.length > 0 && `${plural(lastReport.failures.length, "error", "errores")} de lectura`,
        ]
          .filter(Boolean)
          .join(" · ")
      : "",
  );
</script>

<h1>Ajustes</h1>

<section aria-labelledby="s-look">
  <h2 id="s-look">Apariencia</h2>
  <div class="row">
    <label for="theme">Tema</label>
    <select id="theme" value={theme.preference} onchange={(e) => theme.set((e.currentTarget as HTMLSelectElement).value as ThemePreference)}>
      {#each themes as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </div>
  <div class="row">
    <span id="accent-label">Color de acento</span>
    <div class="swatches" role="radiogroup" aria-labelledby="accent-label">
      {#each ACCENTS as accent (accent.value)}
        <button
          role="radio"
          aria-checked={prefs.accent === accent.value}
          aria-label={accent.label}
          title={accent.label}
          class="swatch"
          class:selected={prefs.accent === accent.value}
          style:--swatch={theme.resolved === "dark" ? accent.dark : accent.light}
          onclick={() => void prefs.setAccent(accent.value)}
        ></button>
      {/each}
    </div>
  </div>
</section>

<section aria-labelledby="s-read">
  <h2 id="s-read">Lectura</h2>
  <p class="note">Se aplican a los libros que todavía no tienen opciones propias.</p>
  <div class="row">
    <label for="fit">Ajuste de página (cómics y PDF)</label>
    <select id="fit" value={prefs.defaultFit} onchange={(e) => {
      const value = (e.currentTarget as HTMLSelectElement).value as FitMode;
      void save("readerFit", value, () => (prefs.defaultFit = value));
    }}>
      {#each fits as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </div>
  <div class="row">
    <label for="layout">Modo de los cómics</label>
    <select id="layout" value={prefs.defaultLayout} onchange={(e) => {
      const value = (e.currentTarget as HTMLSelectElement).value as PageLayout;
      void save("defaultLayout", value, () => (prefs.defaultLayout = value));
    }}>
      {#each layouts as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </div>
  <div class="row">
    <label for="direction">Sentido de lectura de los cómics</label>
    <select id="direction" value={prefs.defaultDirection} onchange={(e) => {
      const value = (e.currentTarget as HTMLSelectElement).value as ReadingDirection;
      void save("defaultDirection", value, () => (prefs.defaultDirection = value));
    }}>
      {#each directions as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </div>
  <div class="row">
    <label for="epub-theme">Tema de los libros (EPUB)</label>
    <select id="epub-theme" value={epubStyle.value.theme} onchange={(e) => epubStyle.update({ theme: (e.currentTarget as HTMLSelectElement).value as EpubTheme })}>
      {#each epubThemes as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </div>
</section>

<section aria-labelledby="s-lib">
  <h2 id="s-lib">Biblioteca</h2>
  <p class="note">
    Las carpetas de la biblioteca se revisan al pulsar «Revisar ahora» (o al iniciar, si lo activas):
    se añaden los libros nuevos, se detectan los que han desaparecido y se reconocen los movidos de
    sitio, que conservan su progreso. No se vigilan en segundo plano, para no gastar recursos.
  </p>
  <ul class="folders" aria-label="Carpetas de la biblioteca">
    {#each prefs.folders as folder (folder)}
      <li>
        <span class="icon" aria-hidden="true">&#xE8B7;</span>
        <span class="path" title={folder}>{folder}</span>
        <button class="btn subtle" aria-label="Quitar la carpeta {folder}" onclick={() => removeFolder(folder)}>Quitar</button>
      </li>
    {:else}
      <li class="empty">No hay carpetas. Los libros añadidos uno a uno también se comprueban al revisar.</li>
    {/each}
  </ul>
  <div class="actions">
    <button class="btn" onclick={addFolder}><span class="icon" aria-hidden="true">&#xE710;</span> Añadir carpeta…</button>
    <button class="btn accent" disabled={rescanning} onclick={rescan}>
      <span class="icon" aria-hidden="true">&#xE72C;</span> {rescanning ? "Revisando…" : "Revisar ahora"}
    </button>
  </div>
  {#if lastReport}
    <p class="report" role="status">{reportText}.</p>
  {/if}
  <label class="check">
    <input
      type="checkbox"
      checked={prefs.rescanOnStart}
      onchange={(e) => {
        const value = (e.currentTarget as HTMLInputElement).checked;
        void save("rescanOnStart", String(value), () => (prefs.rescanOnStart = value));
      }}
    />
    Revisar la biblioteca al iniciar Nebula Reader
  </label>
</section>

<section aria-labelledby="s-about">
  <h2 id="s-about">Acerca de</h2>
  <p>Nebula Reader {version} · Licencia MIT</p>
  <p class="note">
    Incluye PDFium (BSD-3-Clause / Apache-2.0), UnRAR (licencia de RARLAB) y sevenz-rust2 (Apache-2.0).
  </p>
</section>

<style>
  h1 {
    margin: 0 0 20px;
    font-family: var(--font-display);
    font-size: 28px;
    line-height: 36px;
    font-weight: 600;
  }

  section {
    max-width: 760px;
    margin-bottom: 16px;
    padding: 16px 20px;
    border-radius: var(--radius-md);
    border: 1px solid var(--card-stroke);
    background: var(--card);
  }

  h2 {
    margin: 0 0 8px;
    font-size: 16px;
    font-weight: 600;
  }

  p {
    margin: 4px 0;
  }

  .note {
    font-size: 12px;
    line-height: 18px;
    color: var(--text-secondary);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 44px;
    border-top: 1px solid var(--card-stroke);
  }

  .row:first-of-type {
    border-top: none;
  }

  select {
    min-width: 280px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-stroke);
    background: var(--control);
  }

  select option {
    background: var(--flyout);
    color: var(--text);
  }

  .swatches {
    display: flex;
    gap: 8px;
  }

  .swatch {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--swatch);
    box-shadow: inset 0 0 0 2px var(--flyout);
  }

  .swatch.selected {
    border-color: var(--text);
  }

  .folders {
    list-style: none;
    margin: 12px 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .folders li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: var(--subtle-pressed);
  }

  .folders .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }

  .folders .empty {
    background: none;
    color: var(--text-tertiary);
    font-size: 12px;
  }

  .btn.subtle {
    height: 28px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .report {
    margin-top: 10px;
    color: var(--text-secondary);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
  }

  .check input {
    width: 18px;
    height: 18px;
    accent-color: var(--accent);
  }
</style>
