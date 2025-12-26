//! Jackett search backend implementation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::JackettConfig;

use super::dedup::deduplicate_results;
use super::rate_limiter::{IndexerRateLimitConfig, RateLimiterPool};
use super::{
    IndexerStatus, RawTorrentResult, SearchCategory, SearchError, SearchQuery, SearchResult,
    Searcher,
};

/// Internal state for tracking an indexer.
#[derive(Debug)]
struct IndexerState {
    enabled: bool,
    last_used: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

/// Jackett search backend implementation.
pub struct JackettSearcher {
    client: Client,
    config: JackettConfig,
    rate_limiters: RateLimiterPool,
    indexer_state: RwLock<HashMap<String, IndexerState>>,
}

impl JackettSearcher {
    /// Create a new JackettSearcher with the given configuration.
    pub fn new(config: JackettConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .expect("Failed to create HTTP client");

        let rate_limit_configs: Vec<_> = config
            .indexers
            .iter()
            .map(|i| IndexerRateLimitConfig {
                name: i.name.clone(),
                rate_limit_rpm: i.rate_limit_rpm,
            })
            .collect();
        let rate_limiters = RateLimiterPool::new(&rate_limit_configs);

        let indexer_state = config
            .indexers
            .iter()
            .map(|i| {
                (
                    i.name.clone(),
                    IndexerState {
                        enabled: i.enabled,
                        last_used: None,
                        last_error: None,
                    },
                )
            })
            .collect();

        Self {
            client,
            config,
            rate_limiters,
            indexer_state: RwLock::new(indexer_state),
        }
    }

    /// Build the Jackett API URL for a search.
    fn build_search_url(&self, query: &SearchQuery, indexer: &str) -> String {
        let mut url = format!(
            "{}/api/v2.0/indexers/{}/results?apikey={}&Query={}",
            self.config.url.trim_end_matches('/'),
            urlencoding::encode(indexer),
            urlencoding::encode(&self.config.api_key),
            urlencoding::encode(&query.query)
        );

        if let Some(categories) = &query.categories {
            for cat in categories {
                for cat_id in category_to_jackett_ids(cat) {
                    url.push_str(&format!("&Category[]={}", cat_id));
                }
            }
        }

        url
    }

    /// Search a single indexer.
    async fn search_indexer(
        &self,
        query: &SearchQuery,
        indexer: &str,
    ) -> Result<Vec<RawTorrentResult>, SearchError> {
        // Check rate limit first
        self.rate_limiters.try_acquire(indexer).await?;

        let url = self.build_search_url(query, indexer);
        debug!(indexer = indexer, "Searching Jackett");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SearchError::Timeout
                } else if e.is_connect() {
                    SearchError::ConnectionFailed(e.to_string())
                } else {
                    SearchError::ApiError(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SearchError::ApiError(format!(
                "HTTP {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )));
        }

        let jackett_response: JackettResponse = response
            .json()
            .await
            .map_err(|e| SearchError::ApiError(format!("Failed to parse response: {}", e)))?;

        // Update indexer state
        {
            let mut state = self.indexer_state.write().await;
            if let Some(s) = state.get_mut(indexer) {
                s.last_used = Some(Utc::now());
                s.last_error = None;
            }
        }

        debug!(
            indexer = indexer,
            results = jackett_response.Results.len(),
            "Jackett search complete"
        );

        Ok(jackett_response
            .Results
            .into_iter()
            .map(|r| RawTorrentResult {
                title: r.Title,
                indexer: indexer.to_string(),
                magnet_uri: r.MagnetUri,
                torrent_url: r.Link,
                info_hash: r.InfoHash.map(|h| h.to_lowercase()),
                size_bytes: r.Size.unwrap_or(0) as u64,
                seeders: r.Seeders.unwrap_or(0).max(0) as u32,
                leechers: r
                    .Peers
                    .unwrap_or(0)
                    .saturating_sub(r.Seeders.unwrap_or(0))
                    .max(0) as u32,
                category: r.CategoryDesc,
                publish_date: r.PublishDate.and_then(|d| parse_jackett_date(&d)),
                details_url: r.Details,
                files: None, // Jackett doesn't return file lists in search results
            })
            .collect())
    }
}

#[async_trait]
impl Searcher for JackettSearcher {
    fn name(&self) -> &str {
        "jackett"
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchResult, SearchError> {
        let start = Instant::now();

        // Determine which indexers to search
        let indexers_to_search: Vec<String> = {
            let state = self.indexer_state.read().await;
            match &query.indexers {
                Some(requested) => requested
                    .iter()
                    .filter(|i| state.get(*i).map(|s| s.enabled).unwrap_or(false))
                    .cloned()
                    .collect(),
                None => state
                    .iter()
                    .filter(|(_, s)| s.enabled)
                    .map(|(name, _)| name.clone())
                    .collect(),
            }
        };

        if indexers_to_search.is_empty() {
            return Err(SearchError::AllIndexersFailed(
                [("*".to_string(), "No enabled indexers".to_string())].into(),
            ));
        }

        debug!(
            indexers = ?indexers_to_search,
            query = %query.query,
            "Starting parallel search"
        );

        // Search all indexers concurrently
        let search_futures: Vec<_> = indexers_to_search
            .iter()
            .map(|indexer| {
                let indexer = indexer.clone();
                let query = query.clone();
                async move {
                    let result = self.search_indexer(&query, &indexer).await;
                    (indexer, result)
                }
            })
            .collect();

        let results = futures::future::join_all(search_futures).await;

        let mut all_raw: Vec<RawTorrentResult> = Vec::new();
        let mut indexer_errors: HashMap<String, String> = HashMap::new();

        for (indexer, result) in results {
            match result {
                Ok(mut torrents) => {
                    all_raw.append(&mut torrents);
                }
                Err(e) => {
                    warn!(indexer = %indexer, error = %e, "Indexer search failed");
                    // Update indexer state with error
                    {
                        let mut state = self.indexer_state.write().await;
                        if let Some(s) = state.get_mut(&indexer) {
                            s.last_error = Some(e.to_string());
                        }
                    }
                    indexer_errors.insert(indexer, e.to_string());
                }
            }
        }

        // Deduplicate results
        let mut candidates = deduplicate_results(all_raw);

        // Apply limit
        if let Some(limit) = query.limit {
            candidates.truncate(limit as usize);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // If all indexers failed, return error
        if candidates.is_empty() && !indexer_errors.is_empty() {
            return Err(SearchError::AllIndexersFailed(indexer_errors));
        }

        debug!(
            results = candidates.len(),
            duration_ms = duration_ms,
            "Search complete"
        );

        Ok(SearchResult {
            query: query.clone(),
            candidates,
            duration_ms,
            indexer_errors,
        })
    }

    async fn indexer_status(&self) -> Vec<IndexerStatus> {
        let state = self.indexer_state.read().await;
        let rate_status = self.rate_limiters.all_status().await;
        let rate_map: HashMap<_, _> = rate_status.into_iter().collect();

        self.config
            .indexers
            .iter()
            .map(|cfg| {
                let s = state.get(&cfg.name);
                let default_rate = super::RateLimitStatus {
                    requests_per_minute: cfg.rate_limit_rpm,
                    tokens_available: cfg.rate_limit_rpm as f32,
                    next_available_in_ms: None,
                };
                IndexerStatus {
                    name: cfg.name.clone(),
                    enabled: s.map(|s| s.enabled).unwrap_or(cfg.enabled),
                    rate_limit: rate_map.get(&cfg.name).cloned().unwrap_or(default_rate),
                    last_used: s.and_then(|s| s.last_used),
                    last_error: s.and_then(|s| s.last_error.clone()),
                }
            })
            .collect()
    }

    async fn update_indexer_rate_limit(
        &self,
        indexer: &str,
        requests_per_minute: u32,
    ) -> Result<(), SearchError> {
        self.rate_limiters
            .set_rate_limit(indexer, requests_per_minute)
            .await
    }

    async fn set_indexer_enabled(&self, indexer: &str, enabled: bool) -> Result<(), SearchError> {
        let mut state = self.indexer_state.write().await;
        match state.get_mut(indexer) {
            Some(s) => {
                s.enabled = enabled;
                Ok(())
            }
            None => Err(SearchError::IndexerNotFound(indexer.to_string())),
        }
    }
}

/// Map our categories to Jackett category IDs.
fn category_to_jackett_ids(cat: &SearchCategory) -> Vec<i32> {
    match cat {
        SearchCategory::Audio | SearchCategory::Music => vec![3000], // Audio
        SearchCategory::Movies => vec![2000],                        // Movies
        SearchCategory::Tv => vec![5000],                            // TV
        SearchCategory::Books => vec![7000],                         // Books
        SearchCategory::Software => vec![4000],                      // PC
        SearchCategory::Other => vec![8000],                         // Other
    }
}

/// Parse Jackett's date format.
fn parse_jackett_date(date_str: &str) -> Option<DateTime<Utc>> {
    // Jackett returns dates in ISO 8601 format
    DateTime::parse_from_rfc3339(date_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            // Try parsing without timezone
            chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc())
        })
}

// Jackett API response types
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JackettResponse {
    Results: Vec<JackettResult>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JackettResult {
    Title: String,
    MagnetUri: Option<String>,
    Link: Option<String>,
    InfoHash: Option<String>,
    Size: Option<i64>,
    Seeders: Option<i32>,
    Peers: Option<i32>,
    CategoryDesc: Option<String>,
    PublishDate: Option<String>,
    Details: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_category_to_jackett_ids() {
        assert_eq!(category_to_jackett_ids(&SearchCategory::Music), vec![3000]);
        assert_eq!(category_to_jackett_ids(&SearchCategory::Audio), vec![3000]);
        assert_eq!(category_to_jackett_ids(&SearchCategory::Movies), vec![2000]);
        assert_eq!(category_to_jackett_ids(&SearchCategory::Tv), vec![5000]);
    }

    #[test]
    fn test_parse_jackett_date_rfc3339() {
        let date = parse_jackett_date("2024-06-15T10:30:00Z");
        assert!(date.is_some());
        let date = date.unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 6);
        assert_eq!(date.day(), 15);
    }

    #[test]
    fn test_parse_jackett_date_with_offset() {
        let date = parse_jackett_date("2024-06-15T10:30:00+02:00");
        assert!(date.is_some());
    }

    #[test]
    fn test_parse_jackett_date_no_timezone() {
        let date = parse_jackett_date("2024-06-15T10:30:00");
        assert!(date.is_some());
    }

    #[test]
    fn test_parse_jackett_date_invalid() {
        let date = parse_jackett_date("invalid");
        assert!(date.is_none());
    }

    #[test]
    fn test_build_search_url() {
        use crate::config::IndexerConfig;

        let config = JackettConfig {
            url: "http://localhost:9117".to_string(),
            api_key: "test-key".to_string(),
            timeout_secs: 30,
            indexers: vec![IndexerConfig {
                name: "test".to_string(),
                enabled: true,
                rate_limit_rpm: 10,
            }],
        };

        let searcher = JackettSearcher::new(config);

        let query = SearchQuery {
            query: "test query".to_string(),
            indexers: None,
            categories: None,
            limit: None,
        };

        let url = searcher.build_search_url(&query, "test");
        assert!(url.contains("http://localhost:9117/api/v2.0/indexers/test/results"));
        assert!(url.contains("apikey=test-key"));
        assert!(url.contains("Query=test%20query"));
    }

    #[test]
    fn test_build_search_url_with_categories() {
        use crate::config::IndexerConfig;

        let config = JackettConfig {
            url: "http://localhost:9117/".to_string(), // trailing slash
            api_key: "key".to_string(),
            timeout_secs: 30,
            indexers: vec![IndexerConfig {
                name: "test".to_string(),
                enabled: true,
                rate_limit_rpm: 10,
            }],
        };

        let searcher = JackettSearcher::new(config);

        let query = SearchQuery {
            query: "test".to_string(),
            indexers: None,
            categories: Some(vec![SearchCategory::Music, SearchCategory::Movies]),
            limit: None,
        };

        let url = searcher.build_search_url(&query, "test");
        assert!(url.contains("Category[]=3000")); // Music
        assert!(url.contains("Category[]=2000")); // Movies
    }
}
