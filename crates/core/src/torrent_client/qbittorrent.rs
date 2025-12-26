//! qBittorrent torrent client implementation.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use reqwest::{multipart, Client};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::QBittorrentConfig;

use super::{
    AddTorrentRequest, AddTorrentResult, TorrentClient, TorrentClientError, TorrentFilters,
    TorrentInfo, TorrentState,
};

/// qBittorrent client implementation.
pub struct QBittorrentClient {
    client: Client,
    config: QBittorrentConfig,
    /// Session ID cookie (refreshed on auth failure).
    session: Arc<RwLock<Option<String>>>,
}

impl QBittorrentClient {
    /// Create a new qBittorrent client.
    pub fn new(config: QBittorrentConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .cookie_store(true)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            session: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the base URL without trailing slash.
    fn base_url(&self) -> &str {
        self.config.url.trim_end_matches('/')
    }

    /// Login and store session cookie.
    async fn login(&self) -> Result<(), TorrentClientError> {
        let url = format!("{}/api/v2/auth/login", self.base_url());

        let params = [
            ("username", self.config.username.as_str()),
            ("password", self.config.password.as_str()),
        ];

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TorrentClientError::Timeout
                } else if e.is_connect() {
                    TorrentClientError::ConnectionFailed(e.to_string())
                } else {
                    TorrentClientError::ApiError(e.to_string())
                }
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if body.contains("Ok.") {
            debug!("qBittorrent login successful");
            // Session cookie is stored by the cookie jar
            let mut session = self.session.write().await;
            *session = Some("authenticated".to_string());
            Ok(())
        } else if body.contains("Fails.") || status.as_u16() == 403 {
            Err(TorrentClientError::AuthenticationFailed(
                "Invalid credentials".to_string(),
            ))
        } else {
            Err(TorrentClientError::AuthenticationFailed(format!(
                "Unexpected response: {}",
                body.chars().take(100).collect::<String>()
            )))
        }
    }

    /// Ensure we have a valid session, logging in if needed.
    async fn ensure_authenticated(&self) -> Result<(), TorrentClientError> {
        let session = self.session.read().await;
        if session.is_some() {
            return Ok(());
        }
        drop(session);
        self.login().await
    }

    /// Make an authenticated GET request.
    async fn get(&self, endpoint: &str) -> Result<String, TorrentClientError> {
        self.ensure_authenticated().await?;

        let url = format!("{}{}", self.base_url(), endpoint);
        let response = self.client.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                TorrentClientError::Timeout
            } else {
                TorrentClientError::ApiError(e.to_string())
            }
        })?;

        let status = response.status();
        if status.as_u16() == 403 {
            // Session expired, retry after login
            warn!("qBittorrent session expired, re-authenticating");
            {
                let mut session = self.session.write().await;
                *session = None;
            }
            self.login().await?;

            // Retry the request
            let response = self.client.get(&url).send().await.map_err(|e| {
                TorrentClientError::ApiError(e.to_string())
            })?;

            if !response.status().is_success() {
                return Err(TorrentClientError::ApiError(format!(
                    "HTTP {}",
                    response.status()
                )));
            }

            return response
                .text()
                .await
                .map_err(|e| TorrentClientError::ApiError(e.to_string()));
        }

        if !status.is_success() {
            return Err(TorrentClientError::ApiError(format!("HTTP {}", status)));
        }

        response
            .text()
            .await
            .map_err(|e| TorrentClientError::ApiError(e.to_string()))
    }

    /// Make an authenticated POST request with form data.
    async fn post_form(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<String, TorrentClientError> {
        self.ensure_authenticated().await?;

        let url = format!("{}{}", self.base_url(), endpoint);
        let response = self
            .client
            .post(&url)
            .form(params)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TorrentClientError::Timeout
                } else {
                    TorrentClientError::ApiError(e.to_string())
                }
            })?;

        let status = response.status();
        if status.as_u16() == 403 {
            // Session expired, retry after login
            warn!("qBittorrent session expired, re-authenticating");
            {
                let mut session = self.session.write().await;
                *session = None;
            }
            self.login().await?;

            // Retry the request
            let response = self
                .client
                .post(&url)
                .form(params)
                .send()
                .await
                .map_err(|e| TorrentClientError::ApiError(e.to_string()))?;

            if !response.status().is_success() {
                return Err(TorrentClientError::ApiError(format!(
                    "HTTP {}",
                    response.status()
                )));
            }

            return response
                .text()
                .await
                .map_err(|e| TorrentClientError::ApiError(e.to_string()));
        }

        if !status.is_success() {
            return Err(TorrentClientError::ApiError(format!("HTTP {}", status)));
        }

        response
            .text()
            .await
            .map_err(|e| TorrentClientError::ApiError(e.to_string()))
    }

    /// Make an authenticated POST request with multipart data.
    async fn post_multipart(
        &self,
        endpoint: &str,
        form: multipart::Form,
    ) -> Result<String, TorrentClientError> {
        self.ensure_authenticated().await?;

        let url = format!("{}{}", self.base_url(), endpoint);
        let response = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TorrentClientError::Timeout
                } else {
                    TorrentClientError::ApiError(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(TorrentClientError::ApiError(format!("HTTP {}", status)));
        }

        response
            .text()
            .await
            .map_err(|e| TorrentClientError::ApiError(e.to_string()))
    }
}

/// qBittorrent torrent info response.
#[derive(Debug, Deserialize)]
struct QBTorrentInfo {
    hash: String,
    name: String,
    state: String,
    progress: f64,
    size: i64,
    downloaded: i64,
    uploaded: i64,
    dlspeed: i64,
    upspeed: i64,
    num_seeds: i64,
    num_leechs: i64,
    ratio: f64,
    eta: i64,
    added_on: i64,
    completion_on: i64,
    save_path: String,
    category: String,
    up_limit: i64,
    dl_limit: i64,
}

impl QBTorrentInfo {
    fn into_torrent_info(self) -> TorrentInfo {
        TorrentInfo {
            hash: self.hash.to_lowercase(),
            name: self.name,
            state: parse_qb_state(&self.state),
            progress: self.progress,
            size_bytes: self.size.max(0) as u64,
            downloaded_bytes: self.downloaded.max(0) as u64,
            uploaded_bytes: self.uploaded.max(0) as u64,
            download_speed: self.dlspeed.max(0) as u64,
            upload_speed: self.upspeed.max(0) as u64,
            seeders: self.num_seeds.max(0) as u32,
            leechers: self.num_leechs.max(0) as u32,
            ratio: self.ratio,
            eta_secs: if self.eta > 0 && self.eta < 8640000 {
                Some(self.eta as u64)
            } else {
                None
            },
            added_at: timestamp_to_datetime(self.added_on),
            completed_at: if self.completion_on > 0 {
                timestamp_to_datetime(self.completion_on)
            } else {
                None
            },
            save_path: if self.save_path.is_empty() {
                None
            } else {
                Some(self.save_path)
            },
            category: if self.category.is_empty() {
                None
            } else {
                Some(self.category)
            },
            upload_limit: self.up_limit.max(0) as u64,
            download_limit: self.dl_limit.max(0) as u64,
        }
    }
}

/// Parse qBittorrent state string to TorrentState.
fn parse_qb_state(state: &str) -> TorrentState {
    match state {
        "downloading" | "forcedDL" | "metaDL" | "allocating" => TorrentState::Downloading,
        "uploading" | "forcedUP" => TorrentState::Seeding,
        "pausedDL" | "pausedUP" | "stoppedDL" | "stoppedUP" => TorrentState::Paused,
        "checkingDL" | "checkingUP" | "checkingResumeData" | "moving" => TorrentState::Checking,
        "queuedDL" | "queuedUP" => TorrentState::Queued,
        "stalledDL" | "stalledUP" => TorrentState::Stalled,
        "error" | "missingFiles" => TorrentState::Error,
        _ => TorrentState::Unknown,
    }
}

/// Convert Unix timestamp to DateTime<Utc>.
fn timestamp_to_datetime(ts: i64) -> Option<DateTime<Utc>> {
    if ts > 0 {
        Utc.timestamp_opt(ts, 0).single()
    } else {
        None
    }
}

#[async_trait]
impl TorrentClient for QBittorrentClient {
    fn name(&self) -> &str {
        "qbittorrent"
    }

    async fn add_torrent(
        &self,
        request: AddTorrentRequest,
    ) -> Result<AddTorrentResult, TorrentClientError> {
        match request {
            AddTorrentRequest::Magnet {
                uri,
                download_path,
                category,
                paused,
            } => {
                let mut form = multipart::Form::new().text("urls", uri.clone());

                if let Some(path) = download_path.as_ref().or(self.config.download_path.as_ref()) {
                    form = form.text("savepath", path.clone());
                }
                if let Some(cat) = category {
                    form = form.text("category", cat);
                }
                if paused {
                    form = form.text("paused", "true");
                }

                self.post_multipart("/api/v2/torrents/add", form).await?;

                // Extract hash from magnet URI
                let hash = extract_hash_from_magnet(&uri).unwrap_or_default();

                Ok(AddTorrentResult {
                    hash,
                    name: None, // Name not known until metadata is downloaded
                })
            }
            AddTorrentRequest::TorrentFile {
                data,
                filename,
                download_path,
                category,
                paused,
            } => {
                let file_part = multipart::Part::bytes(data)
                    .file_name(filename.unwrap_or_else(|| "torrent.torrent".to_string()))
                    .mime_str("application/x-bittorrent")
                    .map_err(|e| TorrentClientError::InvalidTorrent(e.to_string()))?;

                let mut form = multipart::Form::new().part("torrents", file_part);

                if let Some(path) = download_path.as_ref().or(self.config.download_path.as_ref()) {
                    form = form.text("savepath", path.clone());
                }
                if let Some(cat) = category {
                    form = form.text("category", cat);
                }
                if paused {
                    form = form.text("paused", "true");
                }

                self.post_multipart("/api/v2/torrents/add", form).await?;

                // Hash will need to be looked up from the torrent list
                // For now, return empty - caller can list torrents to find it
                Ok(AddTorrentResult {
                    hash: String::new(),
                    name: None,
                })
            }
        }
    }

    async fn list_torrents(
        &self,
        filters: &TorrentFilters,
    ) -> Result<Vec<TorrentInfo>, TorrentClientError> {
        let mut endpoint = "/api/v2/torrents/info".to_string();
        let mut query_parts = Vec::new();

        if let Some(state) = &filters.state {
            let filter = match state {
                TorrentState::Downloading => "downloading",
                TorrentState::Seeding => "seeding",
                TorrentState::Paused => "paused",
                TorrentState::Stalled => "stalled",
                TorrentState::Checking => "checking",
                TorrentState::Error => "errored",
                _ => "all",
            };
            if filter != "all" {
                query_parts.push(format!("filter={}", filter));
            }
        }

        if let Some(category) = &filters.category {
            query_parts.push(format!(
                "category={}",
                urlencoding::encode(category)
            ));
        }

        if !query_parts.is_empty() {
            endpoint.push('?');
            endpoint.push_str(&query_parts.join("&"));
        }

        let response = self.get(&endpoint).await?;
        let torrents: Vec<QBTorrentInfo> = serde_json::from_str(&response)
            .map_err(|e| TorrentClientError::ApiError(format!("Failed to parse response: {}", e)))?;

        let mut results: Vec<TorrentInfo> =
            torrents.into_iter().map(|t| t.into_torrent_info()).collect();

        // Apply client-side search filter if specified
        if let Some(search) = &filters.search {
            let search_lower = search.to_lowercase();
            results.retain(|t| t.name.to_lowercase().contains(&search_lower));
        }

        Ok(results)
    }

    async fn get_torrent(&self, hash: &str) -> Result<TorrentInfo, TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        let endpoint = format!("/api/v2/torrents/info?hashes={}", hash_lower);
        let response = self.get(&endpoint).await?;

        let torrents: Vec<QBTorrentInfo> = serde_json::from_str(&response)
            .map_err(|e| TorrentClientError::ApiError(format!("Failed to parse response: {}", e)))?;

        torrents
            .into_iter()
            .next()
            .map(|t| t.into_torrent_info())
            .ok_or_else(|| TorrentClientError::TorrentNotFound(hash.to_string()))
    }

    async fn remove_torrent(&self, hash: &str, delete_files: bool) -> Result<(), TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        let delete_str = if delete_files { "true" } else { "false" };

        self.post_form(
            "/api/v2/torrents/delete",
            &[("hashes", &hash_lower), ("deleteFiles", delete_str)],
        )
        .await?;

        Ok(())
    }

    async fn pause_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        self.post_form("/api/v2/torrents/pause", &[("hashes", &hash_lower)])
            .await?;
        Ok(())
    }

    async fn resume_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        self.post_form("/api/v2/torrents/resume", &[("hashes", &hash_lower)])
            .await?;
        Ok(())
    }

    async fn set_upload_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        let limit_str = limit.to_string();
        self.post_form(
            "/api/v2/torrents/setUploadLimit",
            &[("hashes", &hash_lower), ("limit", &limit_str)],
        )
        .await?;
        Ok(())
    }

    async fn set_download_limit(&self, hash: &str, limit: u64) -> Result<(), TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        let limit_str = limit.to_string();
        self.post_form(
            "/api/v2/torrents/setDownloadLimit",
            &[("hashes", &hash_lower), ("limit", &limit_str)],
        )
        .await?;
        Ok(())
    }

    async fn recheck_torrent(&self, hash: &str) -> Result<(), TorrentClientError> {
        let hash_lower = hash.to_lowercase();
        self.post_form("/api/v2/torrents/recheck", &[("hashes", &hash_lower)])
            .await?;
        Ok(())
    }
}

/// Extract info hash from a magnet URI.
fn extract_hash_from_magnet(magnet: &str) -> Option<String> {
    // Look for xt=urn:btih:HASH
    let parts: Vec<&str> = magnet.split('?').collect();
    if parts.len() < 2 {
        return None;
    }

    for param in parts[1].split('&') {
        if let Some(value) = param.strip_prefix("xt=urn:btih:") {
            // Handle both hex and base32 hashes
            let hash = value.split('&').next().unwrap_or(value);
            return Some(hash.to_lowercase());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_qb_state_downloading() {
        assert_eq!(parse_qb_state("downloading"), TorrentState::Downloading);
        assert_eq!(parse_qb_state("forcedDL"), TorrentState::Downloading);
        assert_eq!(parse_qb_state("metaDL"), TorrentState::Downloading);
    }

    #[test]
    fn test_parse_qb_state_seeding() {
        assert_eq!(parse_qb_state("uploading"), TorrentState::Seeding);
        assert_eq!(parse_qb_state("forcedUP"), TorrentState::Seeding);
    }

    #[test]
    fn test_parse_qb_state_paused() {
        assert_eq!(parse_qb_state("pausedDL"), TorrentState::Paused);
        assert_eq!(parse_qb_state("pausedUP"), TorrentState::Paused);
        assert_eq!(parse_qb_state("stoppedDL"), TorrentState::Paused);
        assert_eq!(parse_qb_state("stoppedUP"), TorrentState::Paused);
    }

    #[test]
    fn test_parse_qb_state_checking() {
        assert_eq!(parse_qb_state("checkingDL"), TorrentState::Checking);
        assert_eq!(parse_qb_state("checkingUP"), TorrentState::Checking);
        assert_eq!(parse_qb_state("checkingResumeData"), TorrentState::Checking);
    }

    #[test]
    fn test_parse_qb_state_queued() {
        assert_eq!(parse_qb_state("queuedDL"), TorrentState::Queued);
        assert_eq!(parse_qb_state("queuedUP"), TorrentState::Queued);
    }

    #[test]
    fn test_parse_qb_state_stalled() {
        assert_eq!(parse_qb_state("stalledDL"), TorrentState::Stalled);
        assert_eq!(parse_qb_state("stalledUP"), TorrentState::Stalled);
    }

    #[test]
    fn test_parse_qb_state_error() {
        assert_eq!(parse_qb_state("error"), TorrentState::Error);
        assert_eq!(parse_qb_state("missingFiles"), TorrentState::Error);
    }

    #[test]
    fn test_parse_qb_state_unknown() {
        assert_eq!(parse_qb_state("something_else"), TorrentState::Unknown);
    }

    #[test]
    fn test_extract_hash_from_magnet() {
        let magnet = "magnet:?xt=urn:btih:abc123def456&dn=Test";
        assert_eq!(extract_hash_from_magnet(magnet), Some("abc123def456".to_string()));

        let magnet_upper = "magnet:?xt=urn:btih:ABC123DEF456&dn=Test";
        assert_eq!(extract_hash_from_magnet(magnet_upper), Some("abc123def456".to_string()));

        let invalid = "not a magnet";
        assert_eq!(extract_hash_from_magnet(invalid), None);

        let no_hash = "magnet:?dn=Test";
        assert_eq!(extract_hash_from_magnet(no_hash), None);
    }

    #[test]
    fn test_timestamp_to_datetime() {
        let dt = timestamp_to_datetime(1703980800);
        assert!(dt.is_some());
        let dt = dt.unwrap();
        assert_eq!(dt.year(), 2023);

        let invalid = timestamp_to_datetime(-1);
        assert!(invalid.is_none());

        let zero = timestamp_to_datetime(0);
        assert!(zero.is_none());
    }

    #[test]
    fn test_qb_torrent_info_conversion() {
        let qb_info = QBTorrentInfo {
            hash: "ABC123".to_string(),
            name: "Test Torrent".to_string(),
            state: "downloading".to_string(),
            progress: 0.5,
            size: 1000000,
            downloaded: 500000,
            uploaded: 100000,
            dlspeed: 10000,
            upspeed: 1000,
            num_seeds: 10,
            num_leechs: 5,
            ratio: 0.2,
            eta: 3600,
            added_on: 1703980800,
            completion_on: 0,
            save_path: "/downloads".to_string(),
            category: "movies".to_string(),
            up_limit: 0,
            dl_limit: 0,
        };

        let info = qb_info.into_torrent_info();
        assert_eq!(info.hash, "abc123"); // lowercase
        assert_eq!(info.name, "Test Torrent");
        assert_eq!(info.state, TorrentState::Downloading);
        assert!((info.progress - 0.5).abs() < 0.001);
        assert_eq!(info.size_bytes, 1000000);
        assert_eq!(info.eta_secs, Some(3600));
        assert_eq!(info.category, Some("movies".to_string()));
    }
}
