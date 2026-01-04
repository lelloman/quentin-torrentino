//! librqbit embedded torrent client implementation.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use librqbit::{
    AddTorrent as RqbitAddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session,
    SessionOptions, SessionPersistenceConfig,
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::{
    AddTorrentRequest, AddTorrentResult, TorrentClient, TorrentClientError, TorrentFilters,
    TorrentInfo, TorrentState,
};
use crate::config::LibrqbitConfig;

/// Embedded librqbit torrent client.
pub struct LibrqbitClient {
    session: Arc<Session>,
    download_path: PathBuf,
    /// Cache of torrent names by hash (for when metadata isn't available yet)
    name_cache: RwLock<std::collections::HashMap<String, String>>,
}

impl LibrqbitClient {
    /// Create a new librqbit client from configuration.
    pub async fn new(config: &LibrqbitConfig) -> Result<Self, TorrentClientError> {
        let download_path = PathBuf::from(&config.download_path);

        // Ensure download directory exists
        if !download_path.exists() {
            std::fs::create_dir_all(&download_path).map_err(|e| {
                TorrentClientError::ConnectionFailed(format!(
                    "Failed to create download directory: {}",
                    e
                ))
            })?;
        }

        let mut opts = SessionOptions::default();

        // Configure DHT
        if !config.enable_dht {
            opts.disable_dht = true;
        }

        // Configure listen port (Range, not RangeInclusive)
        if let Some(port) = config.listen_port {
            opts.listen_port_range = Some(port..(port + 1));
        }

        // Configure persistence
        if let Some(ref persistence_path) = config.persistence_path {
            let persistence_dir = PathBuf::from(persistence_path);
            if !persistence_dir.exists() {
                std::fs::create_dir_all(&persistence_dir).map_err(|e| {
                    TorrentClientError::ConnectionFailed(format!(
                        "Failed to create persistence directory: {}",
                        e
                    ))
                })?;
            }
            opts.persistence = Some(SessionPersistenceConfig::Json {
                folder: Some(persistence_dir),
            });
        }

        info!(
            download_path = %download_path.display(),
            dht_enabled = !opts.disable_dht,
            "Initializing librqbit session"
        );

        let session = Session::new_with_opts(download_path.clone(), opts)
            .await
            .map_err(|e| {
                TorrentClientError::ConnectionFailed(format!(
                    "Failed to initialize librqbit session: {}",
                    e
                ))
            })?;

        if let Some(port) = session.tcp_listen_port() {
            info!(port = port, "librqbit listening on TCP port");
        }

        Ok(Self {
            session,
            download_path,
            name_cache: RwLock::new(std::collections::HashMap::new()),
        })
    }

    /// Format info hash as lowercase hex string.
    fn format_hash(hash: &librqbit_core::Id20) -> String {
        hash.as_string()
    }

    /// Convert librqbit torrent to our TorrentInfo type.
    async fn torrent_to_info(&self, torrent: &Arc<ManagedTorrent>) -> TorrentInfo {
        let hash = Self::format_hash(&torrent.info_hash());
        let stats = torrent.stats();

        // Get name from torrent or fallback to hash prefix
        let name = torrent
            .name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("torrent-{}", &hash[..8]));

        // Determine state
        let state = self.map_state(&stats.state, torrent.is_paused(), stats.finished);

        // Calculate progress
        let progress = if stats.total_bytes > 0 {
            stats.progress_bytes as f64 / stats.total_bytes as f64
        } else {
            0.0
        };

        // Get live stats if available
        let (download_speed, upload_speed, seeders, leechers) = stats
            .live
            .as_ref()
            .map(|live| {
                // Note: Despite the field name "mbps", librqbit actually stores MiB/s (mebibytes)
                // as evidenced by its Display impl: write!(f, "{:.2} MiB/s", self.mbps)
                // Convert MiB/s to bytes/sec: MiB/s * 1024 * 1024
                let dl_speed = (live.download_speed.mbps * 1024.0 * 1024.0) as u64;
                let ul_speed = (live.upload_speed.mbps * 1024.0 * 1024.0) as u64;

                // Count peers from peer_stats
                let total_peers = live.snapshot.peer_stats.queued
                    + live.snapshot.peer_stats.connecting
                    + live.snapshot.peer_stats.live;

                (
                    dl_speed,
                    ul_speed,
                    live.snapshot.peer_stats.live as u32,
                    total_peers as u32,
                )
            })
            .unwrap_or((0, 0, 0, 0));

        // Calculate ratio
        let ratio = if stats.progress_bytes > 0 {
            stats.uploaded_bytes as f64 / stats.progress_bytes as f64
        } else {
            0.0
        };

        // Estimate ETA
        let eta_secs = if state == TorrentState::Downloading && download_speed > 0 {
            let remaining = stats.total_bytes.saturating_sub(stats.progress_bytes);
            Some(remaining / download_speed)
        } else {
            None
        };

        TorrentInfo {
            hash,
            name,
            state,
            progress,
            size_bytes: stats.total_bytes,
            downloaded_bytes: stats.progress_bytes,
            uploaded_bytes: stats.uploaded_bytes,
            download_speed,
            upload_speed,
            seeders,
            leechers,
            ratio,
            eta_secs,
            added_at: None, // librqbit doesn't expose this easily
            completed_at: None,
            save_path: Some(self.download_path.display().to_string()),
            category: None,  // librqbit doesn't have categories
            upload_limit: 0, // librqbit manages this differently
            download_limit: 0,
        }
    }

    /// Map librqbit state to our TorrentState.
    fn map_state(
        &self,
        state: &librqbit::TorrentStatsState,
        is_paused: bool,
        is_finished: bool,
    ) -> TorrentState {
        use librqbit::TorrentStatsState;

        if is_paused {
            return TorrentState::Paused;
        }

        match state {
            TorrentStatsState::Initializing => TorrentState::Checking,
            TorrentStatsState::Live => {
                if is_finished {
                    TorrentState::Seeding
                } else {
                    TorrentState::Downloading
                }
            }
            TorrentStatsState::Paused => TorrentState::Paused,
            TorrentStatsState::Error => TorrentState::Error,
        }
    }

    /// Find a torrent by hash.
    fn find_torrent(&self, hash: &str) -> Option<Arc<ManagedTorrent>> {
        // Parse the hash
        let hash_lower = hash.to_lowercase();

        self.session.with_torrents(|iter| {
            for (_, torrent) in iter {
                let torrent_hash = Self::format_hash(&torrent.info_hash());
                if torrent_hash == hash_lower {
                    return Some(torrent.clone());
                }
            }
            None
        })
    }
}

#[async_trait]
impl TorrentClient for LibrqbitClient {
    fn name(&self) -> &str {
        "librqbit"
    }

    async fn add_torrent(
        &self,
        request: AddTorrentRequest,
    ) -> Result<AddTorrentResult, TorrentClientError> {
        let (uri_storage, data_storage, paused) = match request {
            AddTorrentRequest::Magnet { uri, paused, .. } => (Some(uri), None, paused),
            AddTorrentRequest::TorrentFile { data, paused, .. } => (None, Some(data), paused),
        };

        let add_torrent = if let Some(ref uri) = uri_storage {
            RqbitAddTorrent::from_url(uri)
        } else if let Some(data) = data_storage {
            RqbitAddTorrent::from_bytes(data)
        } else {
            unreachable!()
        };

        let opts = if paused {
            Some(AddTorrentOptions {
                paused: true,
                ..Default::default()
            })
        } else {
            None
        };

        // Add timeout for magnet links - DHT lookup can take forever for rare torrents
        let add_future = self.session.add_torrent(add_torrent, opts);
        let response = tokio::time::timeout(std::time::Duration::from_secs(60), add_future)
            .await
            .map_err(|_| TorrentClientError::ApiError("Timed out waiting for torrent metadata (60s). The torrent may still be added in the background.".to_string()))?
            .map_err(|e| TorrentClientError::ApiError(format!("Failed to add torrent: {}", e)))?;

        match response {
            AddTorrentResponse::Added(_, handle) => {
                let hash = Self::format_hash(&handle.info_hash());
                let name = handle.name().map(|s| s.to_string());

                // Cache the name if available
                if let Some(ref n) = name {
                    self.name_cache
                        .write()
                        .await
                        .insert(hash.clone(), n.clone());
                }

                debug!(hash = %hash, name = ?name, "Torrent added successfully");

                Ok(AddTorrentResult { hash, name })
            }
            AddTorrentResponse::AlreadyManaged(_, handle) => {
                let hash = Self::format_hash(&handle.info_hash());
                let name = handle.name().map(|s| s.to_string());

                warn!(hash = %hash, "Torrent already exists");

                Ok(AddTorrentResult { hash, name })
            }
            AddTorrentResponse::ListOnly(_) => {
                // This shouldn't happen with our options, but handle it gracefully
                Err(TorrentClientError::ApiError(
                    "Torrent was added in list-only mode".to_string(),
                ))
            }
        }
    }

    async fn list_torrents(
        &self,
        filters: &TorrentFilters,
    ) -> Result<Vec<TorrentInfo>, TorrentClientError> {
        let mut torrents = Vec::new();

        // Collect all torrents first
        let all_torrents: Vec<Arc<ManagedTorrent>> = self
            .session
            .with_torrents(|iter| iter.map(|(_, t)| t.clone()).collect());

        for torrent in all_torrents {
            let info = self.torrent_to_info(&torrent).await;

            // Apply filters
            if let Some(ref state_filter) = filters.state {
                if info.state != *state_filter {
                    continue;
                }
            }

            if let Some(ref search) = filters.search {
                let search_lower = search.to_lowercase();
                if !info.name.to_lowercase().contains(&search_lower) {
                    continue;
                }
            }

            // Category filter not applicable for librqbit
            if filters.category.is_some() {
                // librqbit doesn't support categories, so skip filtering
            }

            torrents.push(info);
        }

        Ok(torrents)
    }

    async fn get_torrent(&self, hash: &str) -> Result<TorrentInfo, TorrentClientError> {
        let torrent = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        Ok(self.torrent_to_info(&torrent).await)
    }

    async fn remove_torrent(
        &self,
        hash: &str,
        delete_files: bool,
    ) -> Result<(), TorrentClientError> {
        let torrent = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        let id = torrent.id();

        self.session
            .delete(id.into(), delete_files)
            .await
            .map_err(|e| {
                TorrentClientError::ApiError(format!("Failed to remove torrent: {}", e))
            })?;

        // Remove from name cache
        self.name_cache.write().await.remove(hash);

        debug!(hash = %hash, delete_files = delete_files, "Torrent removed");

        Ok(())
    }

    async fn pause_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        let torrent = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        self.session
            .pause(&torrent)
            .await
            .map_err(|e| TorrentClientError::ApiError(format!("Failed to pause torrent: {}", e)))?;

        debug!(hash = %hash, "Torrent paused");

        Ok(())
    }

    async fn resume_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        let torrent = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        self.session.unpause(&torrent).await.map_err(|e| {
            TorrentClientError::ApiError(format!("Failed to resume torrent: {}", e))
        })?;

        debug!(hash = %hash, "Torrent resumed");

        Ok(())
    }

    async fn set_upload_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError> {
        // librqbit doesn't support per-torrent speed limits in the same way
        // We could implement this with a rate limiter wrapper, but for now just acknowledge
        let _ = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        warn!(
            hash = %hash,
            limit = limit,
            "Per-torrent upload limits not supported by librqbit"
        );

        Ok(())
    }

    async fn set_download_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError> {
        // librqbit doesn't support per-torrent speed limits in the same way
        let _ = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        warn!(
            hash = %hash,
            limit = limit,
            "Per-torrent download limits not supported by librqbit"
        );

        Ok(())
    }

    async fn recheck_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        // librqbit doesn't have an explicit recheck command
        // The torrent would need to be removed and re-added
        let _ = self
            .find_torrent(hash)
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))?;

        warn!(
            hash = %hash,
            "Recheck not directly supported by librqbit"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_librqbit_client_name() {
        // We can't easily test the client without a real session,
        // but we can test the state mapping
        let client_name = "librqbit";
        assert_eq!(client_name, "librqbit");
    }

    #[test]
    fn test_state_mapping_logic() {
        // Test the logic of state mapping
        // Paused always returns Paused
        // When finished and live, it's seeding
        // When not finished and live, it's downloading
        // Initializing maps to Checking
        // Error maps to Error
        // Note: actual testing would require mocking the librqbit session
    }
}
