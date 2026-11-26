// Marcadores del libro abierto (una instancia por lector).
import { api, errorMessage } from "#lib/api.ts";
import type { BookId, Bookmark, Locator } from "#lib/types.ts";
import { toasts } from "./toasts.svelte.ts";

/** Si dos localizadores apuntan al mismo sitio (en EPUB, con cierta tolerancia). */
export function sameLocation(a: Locator, b: Locator): boolean {
  if (a.type === "page" && b.type === "page") return a.index === b.index;
  if (a.type === "epub" && b.type === "epub") return a.spine === b.spine && Math.abs(a.position - b.position) < 0.01;
  return false;
}

export class BookmarkList {
  items = $state<Bookmark[]>([]);
  readonly #book: BookId;

  constructor(book: BookId) {
    this.#book = book;
  }

  async load(): Promise<void> {
    try {
      this.items = await api.bookmarks(this.#book);
    } catch (error) {
      toasts.show("error", "No se pudieron cargar los marcadores", errorMessage(error));
    }
  }

  find(locator: Locator): Bookmark | undefined {
    return this.items.find((b) => sameLocation(b.locator, locator));
  }

  /** Añade un marcador o, si ya hay uno en ese sitio, lo quita. */
  async toggle(locator: Locator, percent: number, label: string): Promise<void> {
    const existing = this.find(locator);
    if (existing) {
      await this.remove(existing);
      toasts.show("info", "Marcador quitado", existing.label);
      return;
    }
    try {
      const created = await api.addBookmark(this.#book, locator, percent, label);
      this.items = [...this.items, created].toSorted((a, b) => a.percent - b.percent);
      toasts.show("success", "Marcador añadido", created.label);
    } catch (error) {
      toasts.show("error", "No se pudo añadir el marcador", errorMessage(error));
    }
  }

  async remove(bookmark: Bookmark): Promise<void> {
    try {
      await api.removeBookmark(bookmark.id);
      this.items = this.items.filter((b) => b.id !== bookmark.id);
    } catch (error) {
      toasts.show("error", "No se pudo quitar el marcador", errorMessage(error));
    }
  }
}
