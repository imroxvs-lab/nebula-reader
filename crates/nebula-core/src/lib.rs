//! Núcleo de Nebula Reader.
//!
//! Contiene el dominio, el acceso a formatos de libros y cómics y la biblioteca
//! persistente. No depende de Tauri, de modo que se puede probar de forma aislada.

#![forbid(unsafe_code)]

pub mod archive;
pub mod cover;
pub mod document;
pub mod domain;
pub mod epub;
pub mod error;
pub mod import;
pub mod library;
pub mod metadata;
pub mod natural;
#[cfg(feature = "pdf")]
pub mod pdf;
mod xml;

pub use archive::{ArchiveLimits, ComicArchive, ImageType, PageData};
pub use document::OpenDocument;
pub use domain::{
    Book, BookFormat, BookId, BookKind, Bookmark, BookmarkId, Collection, CollectionId,
    CoverStatus, FacetValue, ImportFailure, ImportReport, LibraryFacets, LibraryQuery,
    LibraryStats, Locator, NewBook, ReadStatus, ReadingProgress, RescanReport, SortBy, Timestamp,
    title_from_path,
};
pub use epub::{EpubBook, EpubMetadata, SpineItem, TocEntry};
pub use error::{CoreError, CoreResult};
pub use import::{ScanOutcome, scan_paths};
pub use library::{InsertOutcome, KnownFile, Library, RescanPlan, plan_rescan};
pub use metadata::BookMetadata;

/// Versión del núcleo, expuesta para diagnósticos en la interfaz.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
