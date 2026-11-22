// Navegación entre secciones (la app es una única página).
import type { Book, BookKind } from "#lib/types.ts";

export type Section = "home" | "all" | "books" | "comics";

export const SECTION_KIND: Record<Exclude<Section, "home">, BookKind | null> = {
  all: null,
  books: "book",
  comics: "comic",
};

class NavStore {
  section = $state<Section>("home");
  /** Libro abierto en el lector; `null` = vista de biblioteca. */
  reading = $state<Book | null>(null);
  /** Pantalla completa (solo en el lector). */
  fullscreen = $state(false);

  go(section: Section): void {
    this.section = section;
  }

  openReader(book: Book): void {
    this.reading = book;
  }

  closeReader(): void {
    this.reading = null;
  }
}

export const nav = new NavStore();
