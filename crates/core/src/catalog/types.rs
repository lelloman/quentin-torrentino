//! Types for the torrent catalog (search result cache).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A cached torrent entry from previous searches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTorrent {
    /// Info hash (lowercase hex).
    pub info_hash: String,
    /// Torrent title.
    pub title: String,
    /// Total size in bytes.
    pub size_bytes: u64,
    /// Category (e.g., "Audio", "Music/Lossless").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// When first cached.
    pub first_seen_at: DateTime<Utc>,
    /// When last seen in a search result.
    pub last_seen_at: DateTime<Utc>,
    /// Number of times this torrent appeared in search results.
    pub seen_count: u32,
    /// All known sources for this torrent.
    pub sources: Vec<CachedTorrentSource>,
    /// File list (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<CachedTorrentFile>>,
}

/// A source/indexer for a cached torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTorrentSource {
    /// Indexer name.
    pub indexer: String,
    /// Magnet URI (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnet_uri: Option<String>,
    /// .torrent download URL (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torrent_url: Option<String>,
    /// Seeders (last known).
    pub seeders: u32,
    /// Leechers (last known).
    pub leechers: u32,
    /// Details page URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    /// When this source was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A file within a cached torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTorrentFile {
    /// File path within torrent.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Search mode for combined catalog + external search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// Search only the local cache.
    CacheOnly,
    /// Search only external indexers (Jackett).
    ExternalOnly,
    /// Search both, combine results.
    #[default]
    Both,
}

/// Query for searching the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSearchQuery {
    /// Search text (matched against title and file paths).
    pub query: String,
    /// Maximum results.
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    100
}

/// Catalog statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogStats {
    /// Total cached torrents.
    pub total_torrents: u64,
    /// Total cached files.
    pub total_files: u64,
    /// Total size of all cached torrents (bytes).
    pub total_size_bytes: u64,
    /// Number of unique indexers.
    pub unique_indexers: u32,
    /// Oldest entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_entry: Option<DateTime<Utc>>,
    /// Most recent entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_entry: Option<DateTime<Utc>>,
}

/// Errors for catalog operations.
#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_mode_serialization() {
        assert_eq!(
            serde_json::to_string(&SearchMode::CacheOnly).unwrap(),
            "\"cache_only\""
        );
        assert_eq!(
            serde_json::to_string(&SearchMode::ExternalOnly).unwrap(),
            "\"external_only\""
        );
        assert_eq!(
            serde_json::to_string(&SearchMode::Both).unwrap(),
            "\"both\""
        );
    }

    #[test]
    fn test_search_mode_default() {
        let mode = SearchMode::default();
        assert_eq!(mode, SearchMode::Both);
    }

    #[test]
    fn test_catalog_search_query_default_limit() {
        let json = r#"{"query": "test"}"#;
        let query: CatalogSearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.query, "test");
        assert_eq!(query.limit, 100);
    }

    #[test]
    fn test_cached_torrent_serialization() {
        let torrent = CachedTorrent {
            info_hash: "abc123".to_string(),
            title: "Test Torrent".to_string(),
            size_bytes: 1024 * 1024 * 100,
            category: Some("Music".to_string()),
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
            seen_count: 5,
            sources: vec![CachedTorrentSource {
                indexer: "rutracker".to_string(),
                magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
                torrent_url: None,
                seeders: 10,
                leechers: 2,
                details_url: None,
                updated_at: Utc::now(),
            }],
            files: Some(vec![CachedTorrentFile {
                path: "album/01 - Track.flac".to_string(),
                size_bytes: 50 * 1024 * 1024,
            }]),
        };

        let json = serde_json::to_string(&torrent).unwrap();
        let parsed: CachedTorrent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.info_hash, "abc123");
        assert_eq!(parsed.title, "Test Torrent");
        assert_eq!(parsed.sources.len(), 1);
        assert_eq!(parsed.files.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_catalog_stats_serialization() {
        let stats = CatalogStats {
            total_torrents: 100,
            total_files: 5000,
            total_size_bytes: 1024 * 1024 * 1024 * 50,
            unique_indexers: 3,
            oldest_entry: None,
            newest_entry: Some(Utc::now()),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(!json.contains("oldest_entry")); // None should be skipped
        assert!(json.contains("newest_entry"));
    }
}
