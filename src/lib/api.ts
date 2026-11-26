// Envoltorios tipados de los comandos IPC de Rust.
import { invoke } from "@tauri-apps/api/core";
import type {
  Book,
  BookId,
  Bookmark,
  BookmarkId,
  Collection,
  CollectionId,
  CommandError,
  ImportReport,
  LibraryQuery,
  LibraryFacets,
  LibraryStats,
  Locator,
  ReadStatus,
  RescanReport,
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
  setBookOptions: (id: BookId, options: string) => invoke<void>("book_options_set", { id, options }),
  setStatus: (id: BookId, status: ReadStatus) => invoke<void>("book_set_status", { id, status }),
  relocateBook: (id: BookId, path: string) => invoke<void>("book_relocate", { id, path }),
  facets: () => invoke<LibraryFacets>("library_facets"),
  rescan: () => invoke<RescanReport>("library_rescan"),
  bookmarks: (id: BookId) => invoke<Bookmark[]>("bookmarks_list", { id }),
  addBookmark: (id: BookId, locator: Locator, percent: number, label: string) =>
    invoke<Bookmark>("bookmark_add", { id, locator, percent, label }),
  removeBookmark: (id: BookmarkId) => invoke<void>("bookmark_remove", { id }),
  collections: () => invoke<Collection[]>("collections_list"),
  createCollection: (name: string) => invoke<Collection>("collection_create", { name }),
  renameCollection: (id: CollectionId, name: string) => invoke<void>("collection_rename", { id, name }),
  deleteCollection: (id: CollectionId) => invoke<void>("collection_delete", { id }),
  addToCollection: (id: CollectionId, books: BookId[]) => invoke<void>("collection_add_books", { id, books }),
  removeFromCollection: (id: CollectionId, books: BookId[]) =>
    invoke<void>("collection_remove_books", { id, books }),
  bookCollections: (id: BookId) => invoke<CollectionId[]>("book_collections", { id }),
  startupFiles: () => invoke<string[]>("startup_files"),
  bookForPath: (path: string) => invoke<Book>("book_for_path", { path }),
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

/** Página de un PDF renderizada a `width` píxeles físicos. */
export function pdfUrl(book: Book, index: number, width: number): string {
  return `${MEDIA_BASE}/pdf/${book.id}/${index}?w=${width}&v=${book.addedAt}`;
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

/** Si se puede pedir la portada en miniatura (todos los formatos la tienen). */
export function hasCover(book: Book): boolean {
  return book.cover !== "failed";
}
