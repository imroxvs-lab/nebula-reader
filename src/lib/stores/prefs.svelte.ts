// Preferencias generales: color de acento, opciones por defecto del lector y
// carpetas de la biblioteca. Se guardan en la biblioteca (tabla `settings`).
import { api } from "#lib/api.ts";
import type { Accent, FitMode, PageLayout, ReadingDirection } from "#lib/types.ts";

export const ACCENTS: { value: Accent; label: string; light: string; dark: string }[] = [
  { value: "violet", label: "Violeta", light: "#6a4dd8", dark: "#b4a3ff" },
  { value: "blue", label: "Azul", light: "#0063b1", dark: "#7cc2ff" },
  { value: "teal", label: "Turquesa", light: "#00787a", dark: "#5ed3cf" },
  { value: "green", label: "Verde", light: "#107c10", dark: "#7fd67f" },
  { value: "orange", label: "Naranja", light: "#b34700", dark: "#ffab70" },
  { value: "pink", label: "Rosa", light: "#b3236b", dark: "#ff8cc6" },
];

function oneOf<T extends string>(value: string | null, options: readonly T[], fallback: T): T {
  return options.includes(value as T) ? (value as T) : fallback;
}

class PrefsStore {
  accent = $state<Accent>("violet");
  defaultFit = $state<FitMode>("page");
  defaultLayout = $state<PageLayout>("single");
  defaultDirection = $state<ReadingDirection>("ltr");
  folders = $state<string[]>([]);
  rescanOnStart = $state(false);
  loaded = $state(false);

  async load(): Promise<void> {
    const get = (key: string) => api.getSetting(key).catch(() => null);
    const [accent, fit, layout, direction, folders, rescan] = await Promise.all([
      get("accent"),
      get("readerFit"),
      get("defaultLayout"),
      get("defaultDirection"),
      get("libraryFolders"),
      get("rescanOnStart"),
    ]);
    this.accent = oneOf(accent, ACCENTS.map((a) => a.value), "violet");
    this.defaultFit = oneOf(fit, ["page", "width", "original"] as const, "page");
    this.defaultLayout = oneOf(layout, ["single", "double", "webtoon"] as const, "single");
    this.defaultDirection = oneOf(direction, ["ltr", "rtl"] as const, "ltr");
    try {
      const parsed: unknown = folders ? JSON.parse(folders) : [];
      this.folders = Array.isArray(parsed) ? parsed.filter((f): f is string => typeof f === "string") : [];
    } catch {
      this.folders = [];
    }
    this.rescanOnStart = rescan === "true";
    this.applyAccent();
    this.loaded = true;
  }

  async set(key: string, value: string): Promise<void> {
    await api.setSetting(key, value);
  }

  async setAccent(accent: Accent): Promise<void> {
    this.accent = accent;
    this.applyAccent();
    await this.set("accent", accent);
  }

  async setFolders(folders: string[]): Promise<void> {
    this.folders = folders;
    await this.set("libraryFolders", JSON.stringify(folders));
  }

  /** Aplica el acento como variables CSS (los valores dependen del tema). */
  applyAccent(): void {
    const accent = ACCENTS.find((a) => a.value === this.accent) ?? ACCENTS[0];
    const root = document.documentElement.style;
    root.setProperty("--accent-light", accent.light);
    root.setProperty("--accent-dark", accent.dark);
  }
}

export const prefs = new PrefsStore();
