// Navegación entre secciones (la app es una única página).
import type { BookKind } from "#lib/types.ts";

export type Section = "home" | "all" | "books" | "comics";

export const SECTION_KIND: Record<Exclude<Section, "home">, BookKind | null> = {
  all: null,
  books: "book",
  comics: "comic",
};

class NavStore {
  section = $state<Section>("home");

  go(section: Section): void {
    this.section = section;
  }
}

export const nav = new NavStore();
