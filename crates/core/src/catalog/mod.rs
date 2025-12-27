//! Torrent catalog - a cache of torrent metadata from previous searches.
//!
//! The catalog stores search results so future searches can check locally
//! before hitting external indexers (Jackett).

mod sqlite;
mod types;

pub use sqlite::SqliteCatalog;
pub use types::*;

use crate::searcher::TorrentCandidate;

/// Trait for torrent catalog storage.
pub trait TorrentCatalog: Send + Sync {
    /// Store search results in the catalog.
    ///
    /// Deduplicates by info_hash - if a torrent already exists, its sources
    /// are merged and seen_count/last_seen_at are updated.
    ///
    /// Returns the number of new torrents added (not updates).
    fn store(&self, candidates: &[TorrentCandidate]) -> Result<u32, CatalogError>;

    /// Search the catalog by query string.
    ///
    /// Matches against torrent title and file paths using LIKE.
    fn search(&self, query: &CatalogSearchQuery) -> Result<Vec<CachedTorrent>, CatalogError>;

    /// Get a specific torrent by info_hash.
    fn get(&self, info_hash: &str) -> Result<CachedTorrent, CatalogError>;

    /// Get catalog statistics.
    fn stats(&self) -> Result<CatalogStats, CatalogError>;

    /// Check if a torrent exists in the catalog.
    fn exists(&self, info_hash: &str) -> Result<bool, CatalogError>;

    /// Remove a torrent from the catalog.
    fn remove(&self, info_hash: &str) -> Result<(), CatalogError>;

    /// Clear all cached data.
    fn clear(&self) -> Result<(), CatalogError>;
}
