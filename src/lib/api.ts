// Envoltorios tipados de los comandos IPC de Rust.
import { invoke } from "@tauri-apps/api/core";
import type {
  Book,
  BookId,
  CommandError,
  ImportReport,
  LibraryQuery,
  LibraryStats,
  Locator,
  OpenedBook,
} from "./types";

export const api = {
  appVersion: () => invoke<string>("app_version"),
  listBooks: (query: LibraryQuery = {}) => invoke<Book[]>("library_list", { query }),
  libraryStats: () => invoke<LibraryStats>("library_stats"),
  importPaths: (paths: string[]) => invoke<ImportReport>("library_import", { paths }),
  removeBook: (id: BookId) => invoke<void>("library_remove", { id }),
  continueReading: (limit: number) => invoke<Book[]>("library_continue_reading", { limit }),
  openBook: (id: BookId) => invoke<OpenedBook>("book_open", { id }),
  closeBook: (id: BookId) => invoke<void>("book_close", { id }),
  saveProgress: (id: BookId, locator: Locator, percent: number) =>
    invoke<void>("progress_save", { id, locator, percent }),
  getSetting: (key: string) => invoke<string | null>("settings_get", { key }),
  setSetting: (key: string, value: string) => invoke<void>("settings_set", { key, value }),
};

function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "message" in value &&
    typeof (value as { message: unknown }).message === "string"
  );
}

/** Mensaje legible de cualquier error devuelto por `invoke`. */
export function errorMessage(error: unknown): string {
  if (isCommandError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

// Recursos servidos por el protocolo `nebula://` (en Windows, `http://nebula.localhost`).
const MEDIA_BASE = navigator.userAgent.includes("Windows")
  ? "http://nebula.localhost"
  : "nebula://localhost";

/** Portada en miniatura. `addedAt` invalida la caché si se reutiliza un id. */
export function coverUrl(book: Book): string {
  return `${MEDIA_BASE}/cover/${book.id}?v=${book.addedAt}`;
}

/** Página de un cómic. */
export function pageUrl(book: Book, index: number): string {
  return `${MEDIA_BASE}/page/${book.id}/${index}?v=${book.addedAt}`;
}

/** URL de un recurso de un EPUB (ruta dentro del archivo, sin codificar). */
export function epubUrl(book: Book, href: string): string {
  const encoded = href.split("/").map(encodeURIComponent).join("/");
  return `${MEDIA_BASE}/epub/${book.id}/${encoded}?v=${book.addedAt}`;
}

/** URL base de los recursos de un EPUB (para resolver rutas relativas). */
export function epubBase(book: Book): string {
  return `${MEDIA_BASE}/epub/${book.id}/`;
}

/** Si el formato ya tiene portada en miniatura. */
export function hasCover(book: Book): boolean {
  return (book.format === "cbz" || book.format === "epub") && book.cover !== "failed";
}
