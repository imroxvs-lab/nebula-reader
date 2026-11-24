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
  author: string | null;
  series: string | null;
  seriesIndex: number | null;
}

export type CoverStatus = "pending" | "ready" | "failed";

export type Locator =
  | { type: "page"; index: number }
  | { type: "epub"; spine: number; position: number };

export interface SpineItem {
  href: string;
  size: number;
  linear: boolean;
}

export interface TocEntry {
  title: string;
  href: string;
  fragment: string | null;
  level: number;
}

export type ReaderContent =
  | { type: "comic" }
  | { type: "epub"; spine: SpineItem[]; toc: TocEntry[] }
  | { type: "pdf"; aspectRatio: number };

export interface ReadingProgress {
  locator: Locator;
  percent: number;
  updatedAt: number;
}

export interface OpenedBook {
  book: Book;
  pageCount: number;
  progress: ReadingProgress | null;
  content: ReaderContent;
  /** Opciones de lectura guardadas (JSON), si las hay. */
  readerOptions: string | null;
}

export type PageLayout = "single" | "double" | "webtoon";
export type ReadingDirection = "ltr" | "rtl";

export interface ReaderOptions {
  layout: PageLayout;
  direction: ReadingDirection;
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

export type EpubTheme = "paper" | "sepia" | "dark";
export type EpubFont = "original" | "serif" | "sans";

export interface EpubStyle {
  theme: EpubTheme;
  font: EpubFont;
  /** Porcentaje del tamaño base (80–200). */
  fontSize: number;
  /** Interlineado (1.2–2.0). */
  lineHeight: number;
  /** Margen lateral en píxeles. */
  margin: number;
}
