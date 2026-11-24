// Preferencias tipográficas del lector EPUB, persistidas en la biblioteca.
import { api } from "#lib/api.ts";
import type { EpubFont, EpubStyle, EpubTheme } from "#lib/types.ts";

const SETTING_KEY = "epubStyle";

export const FONT_SIZE = { min: 80, max: 200, step: 10 } as const;
export const LINE_HEIGHT = { min: 1.2, max: 2.0, step: 0.1 } as const;
export const MARGINS = [
  { value: 24, label: "Estrecho" },
  { value: 56, label: "Normal" },
  { value: 112, label: "Amplio" },
] as const;

const DEFAULT_STYLE: EpubStyle = {
  theme: "paper",
  font: "original",
  fontSize: 100,
  lineHeight: 1.5,
  margin: 56,
};

const THEMES: readonly EpubTheme[] = ["paper", "sepia", "dark"];
const FONTS: readonly EpubFont[] = ["original", "serif", "sans"];

function clamp(value: unknown, min: number, max: number, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(Math.max(value, min), max)
    : fallback;
}

/** Valida un JSON guardado; cualquier campo incorrecto vuelve a su valor por defecto. */
function parse(raw: string | null): EpubStyle {
  if (!raw) return { ...DEFAULT_STYLE };
  try {
    const value = JSON.parse(raw) as Partial<Record<keyof EpubStyle, unknown>>;
    return {
      theme: THEMES.includes(value.theme as EpubTheme) ? (value.theme as EpubTheme) : DEFAULT_STYLE.theme,
      font: FONTS.includes(value.font as EpubFont) ? (value.font as EpubFont) : DEFAULT_STYLE.font,
      fontSize: clamp(value.fontSize, FONT_SIZE.min, FONT_SIZE.max, DEFAULT_STYLE.fontSize),
      lineHeight: clamp(value.lineHeight, LINE_HEIGHT.min, LINE_HEIGHT.max, DEFAULT_STYLE.lineHeight),
      margin: clamp(value.margin, 0, 200, DEFAULT_STYLE.margin),
    };
  } catch {
    return { ...DEFAULT_STYLE };
  }
}

class EpubStyleStore {
  value = $state<EpubStyle>({ ...DEFAULT_STYLE });
  loaded = $state(false);

  async load(): Promise<void> {
    if (this.loaded) return;
    try {
      this.value = parse(await api.getSetting(SETTING_KEY));
    } catch {
      // Se usan los valores por defecto.
    }
    this.loaded = true;
  }

  update(change: Partial<EpubStyle>): void {
    this.value = parse(JSON.stringify({ ...this.value, ...change }));
    void api.setSetting(SETTING_KEY, JSON.stringify(this.value)).catch(() => {});
  }
}

export const epubStyle = new EpubStyleStore();
