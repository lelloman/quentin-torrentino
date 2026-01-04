//! Jackett search backend implementation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::debug;

use crate::config::JackettConfig;
use crate::metrics;

use super::dedup::deduplicate_results;
use super::{
    IndexerStatus, RawTorrentResult, SearchCategory, SearchError, SearchQuery, SearchResult,
    Searcher,
};

/// Jackett search backend implementation.
pub struct JackettSearcher {
    client: Client,
    config: JackettConfig,
}

impl JackettSearcher {
    /// Create a new JackettSearcher with the given configuration.
    pub fn new(config: JackettConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Build the Jackett API URL for a search.
    /// Uses "all" to search all configured indexers at once.
    fn build_search_url(&self, query: &SearchQuery) -> String {
        let mut url = format!(
            "{}/api/v2.0/indexers/all/results?apikey={}&Query={}",
            self.config.url.trim_end_matches('/'),
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

    /// Build URL to list configured indexers.
    fn build_indexers_url(&self) -> String {
        format!(
            "{}/api/v2.0/indexers?apikey={}",
            self.config.url.trim_end_matches('/'),
            urlencoding::encode(&self.config.api_key)
        )
    }
}

#[async_trait]
impl Searcher for JackettSearcher {
    fn name(&self) -> &str {
        "jackett"
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchResult, SearchError> {
        let start = Instant::now();
        let url = self.build_search_url(query);

        debug!(query = %query.query, "Searching Jackett (all indexers)");

        let response = self.client.get(&url).send().await.map_err(|e| {
            metrics::EXTERNAL_SERVICE_REQUESTS
                .with_label_values(&["jackett", "search", "error"])
                .inc();
            if e.is_timeout() {
                SearchError::Timeout
            } else if e.is_connect() {
                SearchError::ConnectionFailed(e.to_string())
            } else {
                SearchError::ApiError(e.to_string())
            }
        })?;

        if !response.status().is_success() {
            metrics::EXTERNAL_SERVICE_REQUESTS
                .with_label_values(&["jackett", "search", "error"])
                .inc();
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SearchError::ApiError(format!(
                "HTTP {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )));
        }

        let jackett_response: JackettSearchResponse = response
            .json()
            .await
            .map_err(|e| SearchError::ApiError(format!("Failed to parse response: {}", e)))?;

        // Convert to raw results
        let raw_results: Vec<RawTorrentResult> = jackett_response
            .Results
            .into_iter()
            .map(|r| RawTorrentResult {
                title: r.Title,
                indexer: r.Tracker.unwrap_or_else(|| "unknown".to_string()),
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
                files: None,
            })
            .collect();

        // Collect indexer errors from response
        let mut indexer_errors: HashMap<String, String> = HashMap::new();
        let total_indexers = jackett_response
            .Indexers
            .as_ref()
            .map(|i| i.len())
            .unwrap_or(0);
        if let Some(indexers) = jackett_response.Indexers {
            for indexer in indexers {
                if let Some(error) = indexer.Error {
                    if !error.is_empty() {
                        indexer_errors
                            .insert(indexer.Name.unwrap_or_else(|| "unknown".to_string()), error);
                    }
                }
            }
        }

        debug!(raw_results = raw_results.len(), "Jackett search complete");

        // Deduplicate results
        let mut candidates = deduplicate_results(raw_results);

        // Apply limit
        if let Some(limit) = query.limit {
            candidates.truncate(limit as usize);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // Only error if ALL indexers failed (no successful searches at all).
        // If some indexers succeeded but returned no results, that's a valid empty result.
        let failed_indexers = indexer_errors.len();
        if candidates.is_empty() && failed_indexers > 0 && failed_indexers == total_indexers {
            return Err(SearchError::AllIndexersFailed(indexer_errors));
        }

        // Record metrics
        metrics::EXTERNAL_SERVICE_DURATION
            .with_label_values(&["jackett", "search"])
            .observe(duration_ms as f64 / 1000.0);
        metrics::EXTERNAL_SERVICE_REQUESTS
            .with_label_values(&["jackett", "search", "success"])
            .inc();
        metrics::SEARCH_RESULTS
            .with_label_values(&[])
            .observe(candidates.len() as f64);

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
        let url = self.build_indexers_url();

        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch indexers from Jackett");
                return vec![];
            }
        };

        if !response.status().is_success() {
            tracing::warn!(status = %response.status(), "Failed to fetch indexers from Jackett");
            return vec![];
        }

        let indexers: Vec<JackettIndexer> = match response.json().await {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse indexers response");
                return vec![];
            }
        };

        indexers
            .into_iter()
            .filter(|i| i.configured)
            .map(|i| IndexerStatus {
                name: i.id,
                enabled: i.configured,
            })
            .collect()
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
    DateTime::parse_from_rfc3339(date_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc())
        })
}

// Jackett API response types
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JackettSearchResponse {
    Results: Vec<JackettResult>,
    Indexers: Option<Vec<JackettIndexerStatus>>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JackettResult {
    Title: String,
    Tracker: Option<String>,
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

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JackettIndexerStatus {
    Name: Option<String>,
    Error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JackettIndexer {
    id: String,
    configured: bool,
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
        let config = JackettConfig {
            url: "http://localhost:9117".to_string(),
            api_key: "test-key".to_string(),
            timeout_secs: 30,
        };

        let searcher = JackettSearcher::new(config);

        let query = SearchQuery {
            query: "test query".to_string(),
            indexers: None,
            categories: None,
            limit: None,
        };

        let url = searcher.build_search_url(&query);
        assert!(url.contains("http://localhost:9117/api/v2.0/indexers/all/results"));
        assert!(url.contains("apikey=test-key"));
        assert!(url.contains("Query=test%20query"));
    }

    #[test]
    fn test_build_search_url_with_categories() {
        let config = JackettConfig {
            url: "http://localhost:9117/".to_string(),
            api_key: "key".to_string(),
            timeout_secs: 30,
        };

        let searcher = JackettSearcher::new(config);

        let query = SearchQuery {
            query: "test".to_string(),
            indexers: None,
            categories: Some(vec![SearchCategory::Music, SearchCategory::Movies]),
            limit: None,
        };

        let url = searcher.build_search_url(&query);
        assert!(url.contains("Category[]=3000")); // Music
        assert!(url.contains("Category[]=2000")); // Movies
    }
}
