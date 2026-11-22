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
  pageCount: number | null;
  cover: CoverStatus;
  /** Fracción leída (0–1), o `null` si no se ha empezado. */
  progress: number | null;
}

export type CoverStatus = "pending" | "ready" | "failed";

export type Locator = { type: "page"; index: number };

export interface ReadingProgress {
  locator: Locator;
  percent: number;
  updatedAt: number;
}

export interface OpenedBook {
  book: Book;
  pageCount: number;
  progress: ReadingProgress | null;
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

export type ErrorCode =
  | "notFound"
  | "invalidInput"
  | "database"
  | "io"
  | "invalidFile"
  | "unsupported"
  | "internal";

export interface CommandError {
  code: ErrorCode;
  message: string;
}

export type ThemePreference = "system" | "light" | "dark";

export type FitMode = "page" | "width" | "original";
