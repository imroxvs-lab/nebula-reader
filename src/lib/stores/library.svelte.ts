// Estado compartido de la biblioteca y acciones que la modifican.
import { ask, open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage } from "#lib/api.ts";
import { SUPPORTED_EXTENSIONS, plural } from "#lib/format.ts";
import type { Book, ImportReport, LibraryStats } from "#lib/types.ts";
import { toasts } from "./toasts.svelte.ts";

class LibraryStore {
  /** Se incrementa tras cada cambio para que las vistas recarguen. */
  revision = $state(0);
  stats = $state<LibraryStats>({ books: 0, comics: 0 });
  statsLoaded = $state(false);
  importing = $state(false);

  get total(): number {
    return this.stats.books + this.stats.comics;
  }

  async refreshStats(): Promise<void> {
    try {
      this.stats = await api.libraryStats();
      this.statsLoaded = true;
    } catch (error) {
      toasts.show("error", "No se pudo leer la biblioteca", errorMessage(error));
    }
  }

  #changed(): void {
    this.revision += 1;
    void this.refreshStats();
  }

  async importPaths(paths: string[]): Promise<void> {
    if (paths.length === 0 || this.importing) return;
    this.importing = true;
    const progress = toasts.show("progress", "Añadiendo a la biblioteca…");
    try {
      const report = await api.importPaths(paths);
      const { kind, message, detail } = describeReport(report);
      toasts.replace(progress, kind, message, detail);
      if (report.added > 0) this.#changed();
    } catch (error) {
      toasts.replace(progress, "error", "La importación falló", errorMessage(error));
    } finally {
      this.importing = false;
    }
  }

  async pickFiles(): Promise<void> {
    const selected = await open({
      title: "Añadir libros y cómics",
      multiple: true,
      filters: [
        { name: "Libros y cómics", extensions: [...SUPPORTED_EXTENSIONS] },
        { name: "Cómics", extensions: ["cbz", "cbr", "cb7"] },
        { name: "Libros", extensions: ["epub", "pdf"] },
      ],
    });
    if (selected) await this.importPaths(selected);
  }

  async pickFolder(): Promise<void> {
    const selected = await open({
      title: "Añadir una carpeta (se incluyen las subcarpetas)",
      directory: true,
      multiple: true,
    });
    if (selected) await this.importPaths(selected);
  }

  async remove(book: Book): Promise<void> {
    const confirmed = await ask(
      `«${book.title}» se quitará de la biblioteca. El archivo no se borrará del disco.`,
      { title: "Quitar de la biblioteca", kind: "warning", okLabel: "Quitar", cancelLabel: "Cancelar" },
    );
    if (!confirmed) return;
    try {
      await api.removeBook(book.id);
      toasts.show("success", `Se quitó «${book.title}»`);
      this.#changed();
    } catch (error) {
      toasts.show("error", "No se pudo quitar el libro", errorMessage(error));
    }
  }
}

function describeReport(report: ImportReport): {
  kind: "success" | "info" | "error";
  message: string;
  detail?: string;
} {
  const parts: string[] = [];
  if (report.skippedDuplicates > 0) parts.push(`${plural(report.skippedDuplicates, "ya estaba", "ya estaban")} en la biblioteca`);
  if (report.unsupported > 0) parts.push(`${plural(report.unsupported, "archivo no soportado", "archivos no soportados")}`);
  if (report.failures.length > 0) parts.push(`${plural(report.failures.length, "error", "errores")} de lectura`);
  const detail = parts.length > 0 ? parts.join(" · ") : undefined;

  if (report.added > 0) {
    return { kind: "success", message: `${plural(report.added, "elemento añadido", "elementos añadidos")}`, detail };
  }
  if (report.failures.length > 0) {
    const first = report.failures[0];
    return { kind: "error", message: "No se añadió nada", detail: detail ?? `${first.path}: ${first.reason}` };
  }
  return { kind: "info", message: "No se añadió nada nuevo", detail: detail ?? "No se encontraron libros ni cómics" };
}

export const library = new LibraryStore();
