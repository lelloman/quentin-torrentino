//! File enricher - fetches and caches torrent file listings.
//!
//! This module provides functionality to enrich torrent candidates with
//! file listings by fetching .torrent files from their URLs, parsing them,
//! and caching the results.

use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, warn};

use crate::catalog::TorrentCatalog;

use super::torrent_parser::{parse_torrent_files, parse_torrent_info_hash};
use super::{TorrentCandidate, TorrentFile};

/// Configuration for file enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEnricherConfig {
    /// Whether file enrichment is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Maximum number of candidates to enrich with file listings.
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,

    /// Minimum score threshold for enrichment (0.0 - 1.0).
    /// Only candidates scoring above this will be enriched.
    #[serde(default = "default_min_score_threshold")]
    pub min_score_threshold: f32,

    /// Timeout for fetching each .torrent file (seconds).
    #[serde(default = "default_fetch_timeout_secs")]
    pub fetch_timeout_secs: u64,

    /// Maximum concurrent .torrent file downloads.
    #[serde(default = "default_max_parallel_fetches")]
    pub max_parallel_fetches: usize,
}

fn default_enabled() -> bool {
    true
}

fn default_max_candidates() -> usize {
    15
}

fn default_min_score_threshold() -> f32 {
    0.4
}

fn default_fetch_timeout_secs() -> u64 {
    10
}

fn default_max_parallel_fetches() -> usize {
    5
}

impl Default for FileEnricherConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_candidates: default_max_candidates(),
            min_score_threshold: default_min_score_threshold(),
            fetch_timeout_secs: default_fetch_timeout_secs(),
            max_parallel_fetches: default_max_parallel_fetches(),
        }
    }
}

/// Errors that can occur during file enrichment.
#[derive(Debug, Error)]
pub enum FileEnrichError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Catalog error: {0}")]
    Catalog(String),

    #[error("No torrent URL available")]
    NoTorrentUrl,

    #[error("Timeout fetching torrent")]
    Timeout,
}

/// Statistics from an enrichment operation.
#[derive(Debug, Clone, Default)]
pub struct EnrichmentStats {
    /// Number of candidates with files from cache.
    pub cache_hits: u32,
    /// Number of .torrent files successfully fetched and parsed.
    pub fetched: u32,
    /// Number of fetch/parse failures.
    pub failed: u32,
    /// Number of candidates skipped (no torrent_url).
    pub skipped_no_url: u32,
    /// Number of candidates that already had files.
    pub already_had_files: u32,
}

/// File enricher - fetches and caches torrent file listings.
pub struct FileEnricher {
    http_client: Client,
    catalog: Arc<dyn TorrentCatalog>,
    config: FileEnricherConfig,
}

impl FileEnricher {
    /// Create a new file enricher.
    pub fn new(catalog: Arc<dyn TorrentCatalog>, config: FileEnricherConfig) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(config.fetch_timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client,
            catalog,
            config,
        }
    }

    /// Check if enrichment is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configuration.
    pub fn config(&self) -> &FileEnricherConfig {
        &self.config
    }

    /// Enrich candidates with file listings.
    ///
    /// This method:
    /// 1. Checks the catalog cache for existing file listings
    /// 2. For cache misses, fetches .torrent files in parallel
    /// 3. Parses the files and stores them in the catalog
    /// 4. Updates the candidates with the file listings
    ///
    /// Candidates are modified in place.
    pub async fn enrich(&self, candidates: &mut [TorrentCandidate]) -> EnrichmentStats {
        let mut stats = EnrichmentStats::default();

        if !self.config.enabled {
            return stats;
        }

        // Collect candidates that need enrichment
        let mut to_fetch: Vec<(usize, String, String)> = Vec::new(); // (index, info_hash, torrent_url)

        for (idx, candidate) in candidates.iter_mut().enumerate() {
            // Skip if already has files
            if candidate.files.is_some() {
                stats.already_had_files += 1;
                continue;
            }

            // Skip if no info_hash
            if candidate.info_hash.is_empty() {
                continue;
            }

            // Check cache first
            match self.catalog.get_files(&candidate.info_hash) {
                Ok(Some(cached_files)) => {
                    // Cache hit - convert to TorrentFile
                    candidate.files = Some(
                        cached_files
                            .into_iter()
                            .map(|f| TorrentFile {
                                path: f.path,
                                size_bytes: f.size_bytes,
                            })
                            .collect(),
                    );
                    stats.cache_hits += 1;
                    debug!(
                        info_hash = %candidate.info_hash,
                        files = candidate.files.as_ref().map(|f| f.len()).unwrap_or(0),
                        "File listing from cache"
                    );
                    continue;
                }
                Ok(None) => {
                    // Cache miss - need to fetch
                }
                Err(e) => {
                    warn!(
                        info_hash = %candidate.info_hash,
                        error = %e,
                        "Error checking catalog cache"
                    );
                }
            }

            // Find a torrent_url to fetch
            let torrent_url = candidate
                .sources
                .iter()
                .find_map(|s| s.torrent_url.clone());

            match torrent_url {
                Some(url) => {
                    to_fetch.push((idx, candidate.info_hash.clone(), url));
                }
                None => {
                    stats.skipped_no_url += 1;
                }
            }
        }

        if to_fetch.is_empty() {
            return stats;
        }

        debug!(
            count = to_fetch.len(),
            "Fetching .torrent files for enrichment"
        );

        // Fetch in parallel with concurrency limit
        let results: Vec<(usize, Result<Vec<TorrentFile>, FileEnrichError>)> = stream::iter(to_fetch)
            .map(|(idx, info_hash, url)| {
                let client = self.http_client.clone();
                let catalog = self.catalog.clone();
                let title = candidates[idx].title.clone();

                async move {
                    let result = Self::fetch_and_parse(&client, &url).await;

                    // If successful, store in catalog
                    if let Ok(ref files) = result {
                        if let Err(e) = catalog.store_files(&info_hash, &title, files) {
                            warn!(
                                info_hash = %info_hash,
                                error = %e,
                                "Failed to store files in catalog"
                            );
                        }
                    }

                    (idx, result)
                }
            })
            .buffer_unordered(self.config.max_parallel_fetches)
            .collect()
            .await;

        // Apply results to candidates
        for (idx, result) in results {
            match result {
                Ok(files) => {
                    debug!(
                        info_hash = %candidates[idx].info_hash,
                        files = files.len(),
                        "Enriched with file listing"
                    );
                    candidates[idx].files = Some(files);
                    stats.fetched += 1;
                }
                Err(e) => {
                    debug!(
                        info_hash = %candidates[idx].info_hash,
                        error = %e,
                        "Failed to fetch file listing"
                    );
                    stats.failed += 1;
                }
            }
        }

        stats
    }

    /// Fetch a .torrent file and parse it.
    async fn fetch_and_parse(
        client: &Client,
        url: &str,
    ) -> Result<Vec<TorrentFile>, FileEnrichError> {
        debug!(url = %url, "Fetching .torrent file");

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    FileEnrichError::Timeout
                } else {
                    FileEnrichError::Http(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(FileEnrichError::Http(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| FileEnrichError::Http(e.to_string()))?;

        let files =
            parse_torrent_files(&bytes).map_err(|e| FileEnrichError::Parse(e.to_string()))?;

        Ok(files)
    }

    /// Fetch a single .torrent file and extract info_hash and files.
    ///
    /// Useful for verifying/enriching a specific torrent.
    pub async fn fetch_torrent_metadata(
        &self,
        url: &str,
    ) -> Result<(String, Vec<TorrentFile>), FileEnrichError> {
        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    FileEnrichError::Timeout
                } else {
                    FileEnrichError::Http(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(FileEnrichError::Http(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| FileEnrichError::Http(e.to_string()))?;

        let info_hash =
            parse_torrent_info_hash(&bytes).map_err(|e| FileEnrichError::Parse(e.to_string()))?;
        let files =
            parse_torrent_files(&bytes).map_err(|e| FileEnrichError::Parse(e.to_string()))?;

        Ok((info_hash, files))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = FileEnricherConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_candidates, 15);
        assert_eq!(config.min_score_threshold, 0.4);
        assert_eq!(config.fetch_timeout_secs, 10);
        assert_eq!(config.max_parallel_fetches, 5);
    }

    #[test]
    fn test_config_serialization() {
        let config = FileEnricherConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: FileEnricherConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_candidates, config.max_candidates);
    }

    #[test]
    fn test_enrichment_stats_default() {
        let stats = EnrichmentStats::default();
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.fetched, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.skipped_no_url, 0);
    }
}
