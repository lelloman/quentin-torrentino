//! Types for the torrent search system.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// Re-export DateTime for use in other modules
pub use chrono;

/// Query parameters for a torrent search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Free-text search query.
    pub query: String,
    /// Optional: limit to specific indexers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexers: Option<Vec<String>>,
    /// Optional: limit to specific categories (music, movies, tv, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<SearchCategory>>,
    /// Maximum results to return (default: 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Content category for filtering search results.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SearchCategory {
    Audio,
    Music,
    Movies,
    Tv,
    Books,
    Software,
    Other,
}

/// A torrent search result (deduplicated by info_hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentCandidate {
    /// Torrent title (from first source).
    pub title: String,
    /// Info hash (lowercase hex) - used for deduplication.
    /// Empty string if unknown.
    pub info_hash: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Total seeders across all sources.
    pub seeders: u32,
    /// Total leechers across all sources.
    pub leechers: u32,
    /// Category as reported by indexer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// When the torrent was published (earliest across sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_date: Option<DateTime<Utc>>,
    /// File list (if provided by any indexer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<TorrentFile>>,
    /// All indexers that have this torrent.
    pub sources: Vec<TorrentSource>,
    /// Whether this result came from the local cache.
    #[serde(default)]
    pub from_cache: bool,
}

/// A single indexer's listing for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentSource {
    /// Which indexer returned this result.
    pub indexer: String,
    /// Magnet URI from this indexer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magnet_uri: Option<String>,
    /// .torrent download URL from this indexer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torrent_url: Option<String>,
    /// Seeders reported by this indexer.
    pub seeders: u32,
    /// Leechers reported by this indexer.
    pub leechers: u32,
    /// Direct link to torrent page on this indexer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
}

/// A file within a torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    /// Path within the torrent.
    pub path: String,
    /// Size in bytes.
    pub size_bytes: u64,
}

/// Raw result from a single indexer (before deduplication).
#[derive(Debug, Clone)]
pub struct RawTorrentResult {
    pub title: String,
    pub indexer: String,
    pub magnet_uri: Option<String>,
    pub torrent_url: Option<String>,
    pub info_hash: Option<String>,
    pub size_bytes: u64,
    pub seeders: u32,
    pub leechers: u32,
    pub category: Option<String>,
    pub publish_date: Option<DateTime<Utc>>,
    pub details_url: Option<String>,
    pub files: Option<Vec<TorrentFile>>,
}

/// Search result with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The search query that was executed.
    pub query: SearchQuery,
    /// Deduplicated results (grouped by info_hash).
    pub candidates: Vec<TorrentCandidate>,
    /// How long the search took in milliseconds.
    pub duration_ms: u64,
    /// Any indexers that failed (name -> error message).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub indexer_errors: HashMap<String, String>,
}

/// Status of a single indexer (from Jackett).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerStatus {
    /// Indexer name/ID.
    pub name: String,
    /// Whether this indexer is configured/enabled.
    pub enabled: bool,
}

/// Errors that can occur during search operations.
#[derive(Debug, Error)]
pub enum SearchError {
    #[error("Search backend connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Search backend API error: {0}")]
    ApiError(String),

    #[error("Indexer not found: {0}")]
    IndexerNotFound(String),

    #[error("Rate limited for indexer {indexer}, retry in {retry_after_ms}ms")]
    RateLimited {
        indexer: String,
        retry_after_ms: u64,
    },

    #[error("All indexers failed")]
    AllIndexersFailed(HashMap<String, String>),

    #[error("Request timeout")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Trait for torrent search backends.
#[async_trait]
pub trait Searcher: Send + Sync {
    /// Provider name for logging/audit.
    fn name(&self) -> &str;

    /// Execute a search across configured indexers.
    async fn search(&self, query: &SearchQuery) -> Result<SearchResult, SearchError>;

    /// Get status of all configured indexers.
    async fn indexer_status(&self) -> Vec<IndexerStatus>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_serialization() {
        let query = SearchQuery {
            query: "test query".to_string(),
            indexers: Some(vec!["indexer1".to_string()]),
            categories: Some(vec![SearchCategory::Music]),
            limit: Some(50),
        };

        let json = serde_json::to_string(&query).unwrap();
        let parsed: SearchQuery = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.query, "test query");
        assert_eq!(parsed.indexers, Some(vec!["indexer1".to_string()]));
        assert_eq!(parsed.categories, Some(vec![SearchCategory::Music]));
        assert_eq!(parsed.limit, Some(50));
    }

    #[test]
    fn test_search_query_minimal() {
        let json = r#"{"query": "minimal"}"#;
        let parsed: SearchQuery = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.query, "minimal");
        assert!(parsed.indexers.is_none());
        assert!(parsed.categories.is_none());
        assert!(parsed.limit.is_none());
    }

    #[test]
    fn test_search_category_serialization() {
        assert_eq!(
            serde_json::to_string(&SearchCategory::Music).unwrap(),
            "\"music\""
        );
        assert_eq!(
            serde_json::to_string(&SearchCategory::Movies).unwrap(),
            "\"movies\""
        );
        assert_eq!(
            serde_json::to_string(&SearchCategory::Tv).unwrap(),
            "\"tv\""
        );
    }

    #[test]
    fn test_torrent_candidate_serialization() {
        let candidate = TorrentCandidate {
            title: "Test Torrent".to_string(),
            info_hash: "abc123".to_string(),
            size_bytes: 1024,
            seeders: 10,
            leechers: 5,
            category: Some("Music".to_string()),
            publish_date: None,
            files: None,
            sources: vec![TorrentSource {
                indexer: "test_indexer".to_string(),
                magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
                torrent_url: None,
                seeders: 10,
                leechers: 5,
                details_url: None,
            }],
            from_cache: false,
        };

        let json = serde_json::to_string(&candidate).unwrap();
        let parsed: TorrentCandidate = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.title, "Test Torrent");
        assert_eq!(parsed.info_hash, "abc123");
        assert_eq!(parsed.sources.len(), 1);
        assert_eq!(parsed.sources[0].indexer, "test_indexer");
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            query: SearchQuery {
                query: "test".to_string(),
                indexers: None,
                categories: None,
                limit: None,
            },
            candidates: vec![],
            duration_ms: 100,
            indexer_errors: HashMap::new(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("indexer_errors")); // Empty map should be skipped

        let parsed: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.duration_ms, 100);
    }
}
