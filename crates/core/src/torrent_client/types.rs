//! Types for torrent client operations.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during torrent client operations.
#[derive(Debug, Error)]
pub enum TorrentClientError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Torrent not found: {0}")]
    TorrentNotFound(String),

    #[error("Invalid torrent data: {0}")]
    InvalidTorrent(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// State of a torrent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TorrentState {
    /// Downloading from peers.
    Downloading,
    /// Seeding to peers.
    Seeding,
    /// Download or upload is paused.
    Paused,
    /// Checking file integrity.
    Checking,
    /// Queued for download.
    Queued,
    /// Stalled (no peers).
    Stalled,
    /// Error state.
    Error,
    /// Unknown state.
    Unknown,
}

impl TorrentState {
    /// Returns the string representation for API responses.
    pub fn as_str(&self) -> &'static str {
        match self {
            TorrentState::Downloading => "downloading",
            TorrentState::Seeding => "seeding",
            TorrentState::Paused => "paused",
            TorrentState::Checking => "checking",
            TorrentState::Queued => "queued",
            TorrentState::Stalled => "stalled",
            TorrentState::Error => "error",
            TorrentState::Unknown => "unknown",
        }
    }
}

/// Information about a torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    /// Info hash (lowercase hex).
    pub hash: String,
    /// Torrent name.
    pub name: String,
    /// Current state.
    pub state: TorrentState,
    /// Download progress (0.0 - 1.0).
    pub progress: f64,
    /// Total size in bytes.
    pub size_bytes: u64,
    /// Downloaded bytes.
    pub downloaded_bytes: u64,
    /// Uploaded bytes.
    pub uploaded_bytes: u64,
    /// Current download speed in bytes/second.
    pub download_speed: u64,
    /// Current upload speed in bytes/second.
    pub upload_speed: u64,
    /// Number of seeders.
    pub seeders: u32,
    /// Number of leechers.
    pub leechers: u32,
    /// Ratio (uploaded/downloaded).
    pub ratio: f64,
    /// ETA in seconds (None if unknown or complete).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_secs: Option<u64>,
    /// When the torrent was added.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_at: Option<DateTime<Utc>>,
    /// When the torrent completed downloading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Save path on disk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path: Option<String>,
    /// Category/label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Upload speed limit in bytes/second (0 = unlimited).
    pub upload_limit: u64,
    /// Download speed limit in bytes/second (0 = unlimited).
    pub download_limit: u64,
}

/// Request to add a new torrent.
#[derive(Debug, Clone)]
pub enum AddTorrentRequest {
    /// Add via magnet URI.
    Magnet {
        /// Magnet URI.
        uri: String,
        /// Optional download path override.
        download_path: Option<String>,
        /// Optional category/label.
        category: Option<String>,
        /// Start paused.
        paused: bool,
    },
    /// Add via .torrent file contents.
    TorrentFile {
        /// Raw .torrent file bytes.
        data: Vec<u8>,
        /// Original filename (for logging).
        filename: Option<String>,
        /// Optional download path override.
        download_path: Option<String>,
        /// Optional category/label.
        category: Option<String>,
        /// Start paused.
        paused: bool,
    },
}

impl AddTorrentRequest {
    /// Create a magnet request with default options.
    pub fn magnet(uri: impl Into<String>) -> Self {
        AddTorrentRequest::Magnet {
            uri: uri.into(),
            download_path: None,
            category: None,
            paused: false,
        }
    }

    /// Create a torrent file request with default options.
    pub fn torrent_file(data: Vec<u8>) -> Self {
        AddTorrentRequest::TorrentFile {
            data,
            filename: None,
            download_path: None,
            category: None,
            paused: false,
        }
    }

    /// Set the download path.
    pub fn with_download_path(mut self, path: impl Into<String>) -> Self {
        match &mut self {
            AddTorrentRequest::Magnet { download_path, .. } => {
                *download_path = Some(path.into());
            }
            AddTorrentRequest::TorrentFile { download_path, .. } => {
                *download_path = Some(path.into());
            }
        }
        self
    }

    /// Set the category.
    pub fn with_category(mut self, cat: impl Into<String>) -> Self {
        match &mut self {
            AddTorrentRequest::Magnet { category, .. } => {
                *category = Some(cat.into());
            }
            AddTorrentRequest::TorrentFile { category, .. } => {
                *category = Some(cat.into());
            }
        }
        self
    }

    /// Set whether to start paused.
    pub fn with_paused(mut self, p: bool) -> Self {
        match &mut self {
            AddTorrentRequest::Magnet { paused, .. } => {
                *paused = p;
            }
            AddTorrentRequest::TorrentFile { paused, .. } => {
                *paused = p;
            }
        }
        self
    }
}

/// Filters for listing torrents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TorrentFilters {
    /// Filter by state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<TorrentState>,
    /// Filter by category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Search by name (partial match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

impl TorrentFilters {
    /// Check if any filters are set.
    pub fn is_empty(&self) -> bool {
        self.state.is_none() && self.category.is_none() && self.search.is_none()
    }
}

/// Result of adding a torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTorrentResult {
    /// Info hash of the added torrent.
    pub hash: String,
    /// Name of the torrent (may be unknown for magnets initially).
    pub name: Option<String>,
}

/// Trait for torrent client backends.
#[async_trait]
pub trait TorrentClient: Send + Sync {
    /// Backend name for logging/audit.
    fn name(&self) -> &str;

    /// Add a new torrent.
    async fn add_torrent(
        &self,
        request: AddTorrentRequest,
    ) -> Result<AddTorrentResult, TorrentClientError>;

    /// List all torrents, optionally filtered.
    async fn list_torrents(
        &self,
        filters: &TorrentFilters,
    ) -> Result<Vec<TorrentInfo>, TorrentClientError>;

    /// Get a specific torrent by hash.
    async fn get_torrent(&self, hash: &str) -> Result<TorrentInfo, TorrentClientError>;

    /// Remove a torrent.
    /// If `delete_files` is true, also delete downloaded files.
    async fn remove_torrent(
        &self,
        hash: &str,
        delete_files: bool,
    ) -> Result<(), TorrentClientError>;

    /// Pause a torrent.
    async fn pause_torrent(&self, hash: &str) -> Result<(), TorrentClientError>;

    /// Resume a paused torrent.
    async fn resume_torrent(&self, hash: &str) -> Result<(), TorrentClientError>;

    /// Set upload speed limit for a torrent (bytes/second, 0 = unlimited).
    async fn set_upload_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError>;

    /// Set download speed limit for a torrent (bytes/second, 0 = unlimited).
    async fn set_download_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError>;

    /// Recheck/verify torrent files.
    async fn recheck_torrent(&self, hash: &str) -> Result<(), TorrentClientError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_state_as_str() {
        assert_eq!(TorrentState::Downloading.as_str(), "downloading");
        assert_eq!(TorrentState::Seeding.as_str(), "seeding");
        assert_eq!(TorrentState::Paused.as_str(), "paused");
        assert_eq!(TorrentState::Checking.as_str(), "checking");
        assert_eq!(TorrentState::Queued.as_str(), "queued");
        assert_eq!(TorrentState::Stalled.as_str(), "stalled");
        assert_eq!(TorrentState::Error.as_str(), "error");
        assert_eq!(TorrentState::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_torrent_state_serialization() {
        assert_eq!(
            serde_json::to_string(&TorrentState::Downloading).unwrap(),
            "\"downloading\""
        );
        assert_eq!(
            serde_json::to_string(&TorrentState::Seeding).unwrap(),
            "\"seeding\""
        );
    }

    #[test]
    fn test_add_torrent_request_magnet_builder() {
        let req = AddTorrentRequest::magnet("magnet:?xt=urn:btih:abc123")
            .with_download_path("/downloads")
            .with_category("movies")
            .with_paused(true);

        match req {
            AddTorrentRequest::Magnet {
                uri,
                download_path,
                category,
                paused,
            } => {
                assert_eq!(uri, "magnet:?xt=urn:btih:abc123");
                assert_eq!(download_path, Some("/downloads".to_string()));
                assert_eq!(category, Some("movies".to_string()));
                assert!(paused);
            }
            _ => panic!("Expected Magnet variant"),
        }
    }

    #[test]
    fn test_add_torrent_request_file_builder() {
        let data = vec![0u8; 100];
        let req = AddTorrentRequest::torrent_file(data.clone())
            .with_download_path("/downloads")
            .with_category("tv")
            .with_paused(false);

        match req {
            AddTorrentRequest::TorrentFile {
                data: d,
                download_path,
                category,
                paused,
                ..
            } => {
                assert_eq!(d.len(), 100);
                assert_eq!(download_path, Some("/downloads".to_string()));
                assert_eq!(category, Some("tv".to_string()));
                assert!(!paused);
            }
            _ => panic!("Expected TorrentFile variant"),
        }
    }

    #[test]
    fn test_torrent_filters_is_empty() {
        let empty = TorrentFilters::default();
        assert!(empty.is_empty());

        let with_state = TorrentFilters {
            state: Some(TorrentState::Downloading),
            ..Default::default()
        };
        assert!(!with_state.is_empty());

        let with_category = TorrentFilters {
            category: Some("movies".to_string()),
            ..Default::default()
        };
        assert!(!with_category.is_empty());
    }

    #[test]
    fn test_torrent_info_serialization() {
        let info = TorrentInfo {
            hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
            state: TorrentState::Downloading,
            progress: 0.5,
            size_bytes: 1024 * 1024 * 100,
            downloaded_bytes: 1024 * 1024 * 50,
            uploaded_bytes: 1024 * 1024 * 10,
            download_speed: 1024 * 100,
            upload_speed: 1024 * 10,
            seeders: 10,
            leechers: 5,
            ratio: 0.2,
            eta_secs: Some(3600),
            added_at: None,
            completed_at: None,
            save_path: Some("/downloads".to_string()),
            category: Some("movies".to_string()),
            upload_limit: 0,
            download_limit: 0,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: TorrentInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.hash, "abc123");
        assert_eq!(parsed.name, "Test Torrent");
        assert_eq!(parsed.state, TorrentState::Downloading);
        assert!((parsed.progress - 0.5).abs() < 0.001);
        assert_eq!(parsed.eta_secs, Some(3600));
    }

    #[test]
    fn test_add_torrent_result_serialization() {
        let result = AddTorrentResult {
            hash: "abc123def456".to_string(),
            name: Some("My Torrent".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: AddTorrentResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.hash, "abc123def456");
        assert_eq!(parsed.name, Some("My Torrent".to_string()));
    }
}
