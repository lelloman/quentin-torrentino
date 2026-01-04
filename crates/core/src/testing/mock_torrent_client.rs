//! Mock torrent client for testing.

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::torrent_client::{
    AddTorrentRequest, AddTorrentResult, TorrentClient, TorrentClientError, TorrentFilters,
    TorrentInfo, TorrentState,
};

/// A recorded torrent addition for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedAddTorrent {
    /// The request that was made.
    pub request: AddTorrentRequest,
    /// When the request was made.
    pub timestamp: chrono::DateTime<Utc>,
}

/// Internal state for a mock torrent.
#[derive(Debug, Clone)]
struct MockTorrentState {
    info: TorrentInfo,
    paused: bool,
}

/// Mock implementation of the TorrentClient trait.
///
/// Provides controllable behavior for testing:
/// - Track added torrents for assertions
/// - Control torrent progress/state
/// - Simulate failures
///
/// # Example
///
/// ```rust,ignore
/// let client = MockTorrentClient::new();
///
/// // Add a torrent
/// client.add_torrent(AddTorrentRequest::magnet("magnet:?...")).await?;
///
/// // Check what was added
/// let added = client.added_torrents().await;
/// assert_eq!(added.len(), 1);
///
/// // Simulate download progress
/// client.set_progress("abc123", 0.5).await;
/// client.set_progress("abc123", 1.0).await; // Complete
///
/// // Get torrent info
/// let info = client.get_torrent("abc123").await?;
/// assert_eq!(info.state, TorrentState::Seeding);
/// ```
#[derive(Debug)]
pub struct MockTorrentClient {
    /// Recorded add_torrent calls.
    added: Arc<RwLock<Vec<RecordedAddTorrent>>>,
    /// Current torrent states by hash.
    torrents: Arc<RwLock<HashMap<String, MockTorrentState>>>,
    /// If set, the next operation will fail with this error.
    next_error: Arc<RwLock<Option<TorrentClientError>>>,
    /// Counter for generating unique hashes.
    hash_counter: Arc<RwLock<u32>>,
    /// Default save path for new torrents.
    default_save_path: String,
}

impl Default for MockTorrentClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTorrentClient {
    /// Create a new mock torrent client.
    pub fn new() -> Self {
        Self {
            added: Arc::new(RwLock::new(Vec::new())),
            torrents: Arc::new(RwLock::new(HashMap::new())),
            next_error: Arc::new(RwLock::new(None)),
            hash_counter: Arc::new(RwLock::new(0)),
            default_save_path: "/mock/downloads".to_string(),
        }
    }

    /// Create a mock client with a custom save path.
    pub fn with_save_path(save_path: impl Into<String>) -> Self {
        Self {
            default_save_path: save_path.into(),
            ..Self::new()
        }
    }

    /// Get all recorded add_torrent calls.
    pub async fn added_torrents(&self) -> Vec<RecordedAddTorrent> {
        self.added.read().await.clone()
    }

    /// Clear recorded add_torrent calls.
    pub async fn clear_recorded(&self) {
        self.added.write().await.clear();
    }

    /// Set the progress for a torrent (0.0 to 1.0).
    ///
    /// When progress reaches 1.0, the torrent state changes to Seeding.
    pub async fn set_progress(&self, hash: &str, progress: f64) {
        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            let progress = progress.clamp(0.0, 1.0);
            torrent.info.progress = progress;
            torrent.info.downloaded_bytes = (torrent.info.size_bytes as f64 * progress) as u64;

            if progress >= 1.0 {
                torrent.info.state = TorrentState::Seeding;
                torrent.info.completed_at = Some(Utc::now());
                torrent.info.eta_secs = None;
            } else {
                torrent.info.state = if torrent.paused {
                    TorrentState::Paused
                } else {
                    TorrentState::Downloading
                };
                // Estimate ETA based on simulated speed
                let remaining_bytes = torrent.info.size_bytes - torrent.info.downloaded_bytes;
                if torrent.info.download_speed > 0 {
                    torrent.info.eta_secs = Some(remaining_bytes / torrent.info.download_speed);
                }
            }
        }
    }

    /// Set the state for a torrent directly.
    pub async fn set_state(&self, hash: &str, state: TorrentState) {
        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.info.state = state;
        }
    }

    /// Set the download/upload speeds for a torrent.
    pub async fn set_speeds(&self, hash: &str, download: u64, upload: u64) {
        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.info.download_speed = download;
            torrent.info.upload_speed = upload;
        }
    }

    /// Configure the next operation to fail with the given error.
    pub async fn set_next_error(&self, error: TorrentClientError) {
        *self.next_error.write().await = Some(error);
    }

    /// Clear any pending error.
    pub async fn clear_next_error(&self) {
        *self.next_error.write().await = None;
    }

    /// Check if a torrent exists.
    pub async fn has_torrent(&self, hash: &str) -> bool {
        self.torrents.read().await.contains_key(hash)
    }

    /// Get the number of torrents.
    pub async fn torrent_count(&self) -> usize {
        self.torrents.read().await.len()
    }

    /// Pre-populate a torrent (for testing get/list operations).
    pub async fn add_mock_torrent(&self, info: TorrentInfo) {
        let hash = info.hash.clone();
        self.torrents.write().await.insert(
            hash,
            MockTorrentState {
                paused: info.state == TorrentState::Paused,
                info,
            },
        );
    }

    /// Take the next error if set.
    async fn take_error(&self) -> Option<TorrentClientError> {
        self.next_error.write().await.take()
    }

    /// Generate a unique mock hash.
    async fn generate_hash(&self) -> String {
        let mut counter = self.hash_counter.write().await;
        *counter += 1;
        format!("mockhash{:08x}", *counter)
    }

    /// Extract info hash from magnet URI if present.
    fn extract_hash_from_magnet(uri: &str) -> Option<String> {
        // Look for xt=urn:btih: in the magnet link
        uri.split('&')
            .find(|part| part.starts_with("xt=urn:btih:") || part.contains("xt=urn:btih:"))
            .and_then(|part| {
                part.split("xt=urn:btih:")
                    .nth(1)
                    .map(|h| h.split('&').next().unwrap_or(h).to_lowercase())
            })
    }
}

#[async_trait]
impl TorrentClient for MockTorrentClient {
    fn name(&self) -> &str {
        "mock"
    }

    async fn add_torrent(
        &self,
        request: AddTorrentRequest,
    ) -> Result<AddTorrentResult, TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        // Record the request
        self.added.write().await.push(RecordedAddTorrent {
            request: request.clone(),
            timestamp: Utc::now(),
        });

        // Generate or extract hash
        let hash = match &request {
            AddTorrentRequest::Magnet { uri, .. } => {
                Self::extract_hash_from_magnet(uri).unwrap_or_else(|| {
                    // If no hash in magnet, generate one
                    futures::executor::block_on(self.generate_hash())
                })
            }
            AddTorrentRequest::TorrentFile { .. } => {
                futures::executor::block_on(self.generate_hash())
            }
        };

        // Extract name and other info from request
        let hash_prefix = if hash.len() >= 8 { &hash[..8] } else { &hash };
        let (name, save_path, category) = match &request {
            AddTorrentRequest::Magnet {
                download_path,
                category,
                ..
            } => (
                format!("Mock Torrent {}", hash_prefix),
                download_path.clone(),
                category.clone(),
            ),
            AddTorrentRequest::TorrentFile {
                filename,
                download_path,
                category,
                ..
            } => (
                filename
                    .clone()
                    .unwrap_or_else(|| format!("Mock Torrent {}", hash_prefix)),
                download_path.clone(),
                category.clone(),
            ),
        };

        // Create torrent state
        let info = TorrentInfo {
            hash: hash.clone(),
            name: name.clone(),
            state: TorrentState::Downloading,
            progress: 0.0,
            size_bytes: 100 * 1024 * 1024, // 100 MB default
            downloaded_bytes: 0,
            uploaded_bytes: 0,
            download_speed: 1024 * 1024, // 1 MB/s
            upload_speed: 256 * 1024,    // 256 KB/s
            seeders: 10,
            leechers: 5,
            ratio: 0.0,
            eta_secs: Some(100),
            added_at: Some(Utc::now()),
            completed_at: None,
            save_path: Some(save_path.unwrap_or_else(|| self.default_save_path.clone())),
            category,
            upload_limit: 0,
            download_limit: 0,
        };

        self.torrents.write().await.insert(
            hash.clone(),
            MockTorrentState {
                info,
                paused: false,
            },
        );

        Ok(AddTorrentResult {
            hash,
            name: Some(name),
        })
    }

    async fn list_torrents(
        &self,
        filters: &TorrentFilters,
    ) -> Result<Vec<TorrentInfo>, TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        let torrents = self.torrents.read().await;
        let mut result: Vec<TorrentInfo> = torrents
            .values()
            .filter(|t| {
                // Apply state filter
                if let Some(state) = &filters.state {
                    if &t.info.state != state {
                        return false;
                    }
                }
                // Apply category filter
                if let Some(category) = &filters.category {
                    if t.info.category.as_ref() != Some(category) {
                        return false;
                    }
                }
                // Apply search filter
                if let Some(search) = &filters.search {
                    if !t.info.name.to_lowercase().contains(&search.to_lowercase()) {
                        return false;
                    }
                }
                true
            })
            .map(|t| t.info.clone())
            .collect();

        // Sort by added_at descending
        result.sort_by(|a, b| b.added_at.cmp(&a.added_at));

        Ok(result)
    }

    async fn get_torrent(&self, hash: &str) -> Result<TorrentInfo, TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.torrents
            .read()
            .await
            .get(hash)
            .map(|t| t.info.clone())
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))
    }

    async fn remove_torrent(
        &self,
        hash: &str,
        _delete_files: bool,
    ) -> Result<(), TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        if self.torrents.write().await.remove(hash).is_some() {
            Ok(())
        } else {
            Err(TorrentClientError::TorrentNotFound(hash.to_string()))
        }
    }

    async fn pause_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.paused = true;
            if torrent.info.progress < 1.0 {
                torrent.info.state = TorrentState::Paused;
            }
            torrent.info.download_speed = 0;
            torrent.info.upload_speed = 0;
            Ok(())
        } else {
            Err(TorrentClientError::TorrentNotFound(hash.to_string()))
        }
    }

    async fn resume_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.paused = false;
            if torrent.info.progress < 1.0 {
                torrent.info.state = TorrentState::Downloading;
                torrent.info.download_speed = 1024 * 1024;
                torrent.info.upload_speed = 256 * 1024;
            }
            Ok(())
        } else {
            Err(TorrentClientError::TorrentNotFound(hash.to_string()))
        }
    }

    async fn set_upload_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.info.upload_limit = limit;
            Ok(())
        } else {
            Err(TorrentClientError::TorrentNotFound(hash.to_string()))
        }
    }

    async fn set_download_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.info.download_limit = limit;
            Ok(())
        } else {
            Err(TorrentClientError::TorrentNotFound(hash.to_string()))
        }
    }

    async fn recheck_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        let mut torrents = self.torrents.write().await;
        if let Some(torrent) = torrents.get_mut(hash) {
            torrent.info.state = TorrentState::Checking;
            Ok(())
        } else {
            Err(TorrentClientError::TorrentNotFound(hash.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_get_torrent() {
        let client = MockTorrentClient::new();

        let result = client
            .add_torrent(AddTorrentRequest::magnet(
                "magnet:?xt=urn:btih:abc123def456",
            ))
            .await
            .unwrap();

        assert_eq!(result.hash, "abc123def456");

        let info = client.get_torrent("abc123def456").await.unwrap();
        assert_eq!(info.hash, "abc123def456");
        assert_eq!(info.state, TorrentState::Downloading);
        assert_eq!(info.progress, 0.0);
    }

    #[tokio::test]
    async fn test_progress_tracking() {
        let client = MockTorrentClient::new();

        let result = client
            .add_torrent(AddTorrentRequest::magnet(
                "magnet:?xt=urn:btih:testprogress",
            ))
            .await
            .unwrap();

        // Progress to 50%
        client.set_progress(&result.hash, 0.5).await;
        let info = client.get_torrent(&result.hash).await.unwrap();
        assert!((info.progress - 0.5).abs() < 0.01);
        assert_eq!(info.state, TorrentState::Downloading);

        // Complete to 100%
        client.set_progress(&result.hash, 1.0).await;
        let info = client.get_torrent(&result.hash).await.unwrap();
        assert!((info.progress - 1.0).abs() < 0.01);
        assert_eq!(info.state, TorrentState::Seeding);
        assert!(info.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_recorded_requests() {
        let client = MockTorrentClient::new();

        client
            .add_torrent(AddTorrentRequest::magnet("magnet:?xt=urn:btih:one"))
            .await
            .unwrap();
        client
            .add_torrent(AddTorrentRequest::magnet("magnet:?xt=urn:btih:two"))
            .await
            .unwrap();

        let added = client.added_torrents().await;
        assert_eq!(added.len(), 2);
    }

    #[tokio::test]
    async fn test_error_injection() {
        let client = MockTorrentClient::new();

        client
            .set_next_error(TorrentClientError::ConnectionFailed("test".into()))
            .await;

        let result = client
            .add_torrent(AddTorrentRequest::magnet("magnet:?xt=urn:btih:err"))
            .await;

        assert!(result.is_err());

        // Error should be consumed
        let result = client
            .add_torrent(AddTorrentRequest::magnet("magnet:?xt=urn:btih:ok"))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_with_filters() {
        let client = MockTorrentClient::new();

        client
            .add_torrent(
                AddTorrentRequest::magnet("magnet:?xt=urn:btih:movie1").with_category("movies"),
            )
            .await
            .unwrap();
        client
            .add_torrent(
                AddTorrentRequest::magnet("magnet:?xt=urn:btih:music1").with_category("music"),
            )
            .await
            .unwrap();

        let all = client
            .list_torrents(&TorrentFilters::default())
            .await
            .unwrap();
        assert_eq!(all.len(), 2);

        let movies = client
            .list_torrents(&TorrentFilters {
                category: Some("movies".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(movies.len(), 1);
    }

    #[tokio::test]
    async fn test_pause_resume() {
        let client = MockTorrentClient::new();

        let result = client
            .add_torrent(AddTorrentRequest::magnet("magnet:?xt=urn:btih:pausetest"))
            .await
            .unwrap();

        client.pause_torrent(&result.hash).await.unwrap();
        let info = client.get_torrent(&result.hash).await.unwrap();
        assert_eq!(info.state, TorrentState::Paused);
        assert_eq!(info.download_speed, 0);

        client.resume_torrent(&result.hash).await.unwrap();
        let info = client.get_torrent(&result.hash).await.unwrap();
        assert_eq!(info.state, TorrentState::Downloading);
        assert!(info.download_speed > 0);
    }
}
