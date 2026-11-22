// Envoltorios tipados de los comandos IPC de Rust.
import { invoke } from "@tauri-apps/api/core";
import type {
  Book,
  BookId,
  CommandError,
  ImportReport,
  LibraryQuery,
  LibraryStats,
} from "./types";

export const api = {
  appVersion: () => invoke<string>("app_version"),
  listBooks: (query: LibraryQuery = {}) => invoke<Book[]>("library_list", { query }),
  libraryStats: () => invoke<LibraryStats>("library_stats"),
  importPaths: (paths: string[]) => invoke<ImportReport>("library_import", { paths }),
  removeBook: (id: BookId) => invoke<void>("library_remove", { id }),
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
