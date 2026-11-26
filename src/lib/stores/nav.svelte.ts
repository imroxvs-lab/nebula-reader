// Navegación entre secciones (la app es una única página).
import type { Book, BookKind, Collection } from "#lib/types.ts";

export type Section = "home" | "all" | "books" | "comics" | "collection" | "settings";

export const SECTION_KIND: Record<"all" | "books" | "comics", BookKind | null> = {
  all: null,
  books: "book",
  comics: "comic",
};

class NavStore {
  section = $state<Section>("home");
  /** Colección mostrada cuando `section === "collection"`. */
  collection = $state<Collection | null>(null);
  /** Libro abierto en el lector; `null` = vista de biblioteca. */
  reading = $state<Book | null>(null);
  /** Pantalla completa (solo en el lector). */
  fullscreen = $state(false);

  go(section: Exclude<Section, "collection">): void {
    this.section = section;
    this.collection = null;
  }

  goToCollection(collection: Collection): void {
    this.collection = collection;
    this.section = "collection";
  }

  openReader(book: Book): void {
    this.reading = book;
  }

  closeReader(): void {
    this.reading = null;
  }
}

export const nav = new NavStore();
