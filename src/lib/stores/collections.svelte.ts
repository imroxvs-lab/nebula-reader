// Colecciones (estanterías) del usuario.
import { api, errorMessage } from "#lib/api.ts";
import type { Book, Collection, CollectionId } from "#lib/types.ts";
import { library } from "./library.svelte.ts";
import { nav } from "./nav.svelte.ts";
import { toasts } from "./toasts.svelte.ts";

class CollectionsStore {
  items = $state<Collection[]>([]);

  async refresh(): Promise<void> {
    try {
      this.items = await api.collections();
      // Mantiene actualizada la colección abierta (nombre y recuento).
      if (nav.collection) {
        const current = this.items.find((c) => c.id === nav.collection?.id);
        if (current) nav.collection = current;
        else nav.go("all");
      }
    } catch (error) {
      toasts.show("error", "No se pudieron cargar las colecciones", errorMessage(error));
    }
  }

  async create(name: string, books: Book[] = []): Promise<Collection | null> {
    try {
      const created = await api.createCollection(name);
      if (books.length > 0) await api.addToCollection(created.id, books.map((b) => b.id));
      await this.refresh();
      library.refresh();
      toasts.show("success", `Colección «${created.name}» creada`);
      return created;
    } catch (error) {
      toasts.show("error", "No se pudo crear la colección", errorMessage(error));
      return null;
    }
  }

  async rename(id: CollectionId, name: string): Promise<boolean> {
    try {
      await api.renameCollection(id, name);
      await this.refresh();
      return true;
    } catch (error) {
      toasts.show("error", "No se pudo cambiar el nombre", errorMessage(error));
      return false;
    }
  }

  async remove(collection: Collection): Promise<void> {
    try {
      await api.deleteCollection(collection.id);
      if (nav.collection?.id === collection.id) nav.go("all");
      await this.refresh();
      toasts.show("success", `Se borró la colección «${collection.name}»`, "Los libros siguen en la biblioteca.");
    } catch (error) {
      toasts.show("error", "No se pudo borrar la colección", errorMessage(error));
    }
  }

  async toggle(collection: Collection, book: Book, member: boolean): Promise<void> {
    try {
      if (member) await api.removeFromCollection(collection.id, [book.id]);
      else await api.addToCollection(collection.id, [book.id]);
      await this.refresh();
      library.refresh();
    } catch (error) {
      toasts.show("error", "No se pudo actualizar la colección", errorMessage(error));
    }
  }
}

export const collections = new CollectionsStore();
