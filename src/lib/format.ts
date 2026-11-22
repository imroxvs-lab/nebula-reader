// Utilidades de presentación.
import type { BookFormat, BookKind } from "./types";

const COMIC_FORMATS: readonly BookFormat[] = ["cbz", "cbr", "cb7"];

export const SUPPORTED_EXTENSIONS: readonly BookFormat[] = ["cbz", "cbr", "cb7", "epub", "pdf"];

export function kindOf(format: BookFormat): BookKind {
  return COMIC_FORMATS.includes(format) ? "comic" : "book";
}

const sizeFormatter = new Intl.NumberFormat("es-ES", { maximumFractionDigits: 1 });

export function formatSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${sizeFormatter.format(value)} ${units[unit]}`;
}

export function greeting(date = new Date()): string {
  const hour = date.getHours();
  if (hour < 6) return "Buenas noches";
  if (hour < 13) return "Buenos días";
  if (hour < 21) return "Buenas tardes";
  return "Buenas noches";
}

export function plural(count: number, singular: string, pluralForm = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : pluralForm}`;
}

/** Tono estable (0-359) derivado de un texto, para portadas provisionales. */
export function hueFromText(text: string): number {
  let hash = 2166136261;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return Math.abs(hash) % 360;
}

const percentFormatter = new Intl.NumberFormat("es-ES", {
  style: "percent",
  maximumFractionDigits: 0,
});

/** Fracción (0–1) como porcentaje: `0.42` → «42 %». */
export function formatPercent(fraction: number): string {
  return percentFormatter.format(fraction);
}
