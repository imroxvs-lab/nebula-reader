// Tema claro/oscuro/sistema, persistido en la biblioteca y aplicado a la
// ventana (para que el efecto Mica coincida) y al documento.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "#lib/api.ts";
import type { ThemePreference } from "#lib/types.ts";

const SETTING_KEY = "theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
// Tema inicial síncrono para evitar un destello antes de leer el ajuste guardado.
document.documentElement.dataset.theme = media.matches ? "dark" : "light";

function isPreference(value: string | null): value is ThemePreference {
  return value === "system" || value === "light" || value === "dark";
}

class ThemeStore {
  preference = $state<ThemePreference>("system");
  systemDark = $state(media.matches);

  get resolved(): "light" | "dark" {
    if (this.preference === "system") return this.systemDark ? "dark" : "light";
    return this.preference;
  }

  async init(): Promise<void> {
    media.addEventListener("change", (event) => {
      this.systemDark = event.matches;
      this.#apply();
    });
    try {
      const stored = await api.getSetting(SETTING_KEY);
      if (isPreference(stored)) this.preference = stored;
    } catch {
      // Sin ajuste guardado: se queda el del sistema.
    }
    await this.#apply();
  }

  async set(preference: ThemePreference): Promise<void> {
    this.preference = preference;
    await this.#apply();
    await api.setSetting(SETTING_KEY, preference);
  }

  async #apply(): Promise<void> {
    document.documentElement.dataset.theme = this.resolved;
    try {
      await getCurrentWindow().setTheme(this.preference === "system" ? null : this.preference);
    } catch {
      // Fuera de Tauri (p. ej. vista previa en navegador) no hay ventana.
    }
  }
}

export const theme = new ThemeStore();
