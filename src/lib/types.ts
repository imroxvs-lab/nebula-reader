// Tipos espejo de `nebula-core` (serializados en camelCase).

export type BookId = number;
export type BookFormat = "cbz" | "cbr" | "cb7" | "epub" | "pdf";
export type BookKind = "book" | "comic";
export type SortBy = "title" | "addedAt" | "lastOpened";

export interface Book {
  id: BookId;
  path: string;
  title: string;
  format: BookFormat;
  fileSize: number;
  /** Segundos Unix. */
  addedAt: number;
  lastOpenedAt: number | null;
}

export interface LibraryQuery {
  search?: string | null;
  kind?: BookKind | null;
  sort?: SortBy;
  limit?: number | null;
}

export interface LibraryStats {
  books: number;
  comics: number;
}

export interface ImportFailure {
  path: string;
  reason: string;
}

export interface ImportReport {
  added: number;
  skippedDuplicates: number;
  unsupported: number;
  failures: ImportFailure[];
}

export type ErrorCode = "notFound" | "invalidInput" | "database" | "io" | "internal";

export interface CommandError {
  code: ErrorCode;
  message: string;
}

export type ThemePreference = "system" | "light" | "dark";
