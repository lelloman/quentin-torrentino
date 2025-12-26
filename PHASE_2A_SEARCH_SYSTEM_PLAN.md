# Phase 2a: Search System

## Overview

This phase implements the torrent search infrastructure: a `Searcher` trait, Jackett client implementation with per-indexer rate limiting, HTTP API endpoints, and dashboard UI for testing searches and configuring indexers.

By the end, you'll be able to:
- Configure Jackett connection and indexers via dashboard
- Run test searches from the dashboard and see results
- Adjust per-indexer rate limits on the fly

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Search backend | Jackett (first) | Most common, aggregates multiple indexers |
| Rate limiting | Token bucket per indexer | Respects private tracker limits |
| Response format | Normalized `TorrentCandidate` | Clean types, defer smart filtering to matcher |
| Deduplication | Merge by info_hash | Same hash = identical content, show all sources |
| Title parsing | None (Phase 4) | Complex, music-specific, belongs in matcher |
| Config storage | Runtime only (for now) | Config file is source of truth, dashboard shows/edits runtime state |
| Indexer selection | Per-search optional filter | Flexibility for testing specific indexers |

## Scope

### In Scope
- `Searcher` trait definition in `torrentino-core`
- `JackettSearcher` implementation
- Per-indexer rate limiting with token bucket algorithm
- Result deduplication by info_hash (merge identical torrents from multiple indexers)
- Info hash normalization (lowercase)
- Configuration types for searcher settings
- `POST /api/v1/search` - execute a search
- `GET /api/v1/searcher/status` - get searcher status and indexer info
- `GET /api/v1/searcher/indexers` - list configured indexers with rate limit status
- `PATCH /api/v1/searcher/indexers/{name}` - update indexer settings (rate limits)
- Dashboard search testing page
- Dashboard indexer settings page
- Audit events for searches

### Out of Scope
- Prowlarr support (future)
- Direct tracker API support (future)
- Title parsing / metadata extraction (Phase 4 matcher)
- Smart filtering/ranking of results (Phase 4 matcher)
- Persistent config changes (config file remains source of truth)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Dashboard                                │
│  ┌──────────────────┐  ┌──────────────────────────────────────┐ │
│  │  Search Testing  │  │  Indexer Settings                    │ │
│  │  - Query input   │  │  - List indexers                     │ │
│  │  - Indexer filter│  │  - Rate limit status                 │ │
│  │  - Results table │  │  - Edit rate limits                  │ │
│  └────────┬─────────┘  └─────────────────┬────────────────────┘ │
└───────────┼──────────────────────────────┼──────────────────────┘
            │                              │
            ▼                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         HTTP API                                 │
│  POST /api/v1/search                                            │
│  GET  /api/v1/searcher/status                                   │
│  GET  /api/v1/searcher/indexers                                 │
│  PATCH /api/v1/searcher/indexers/{name}                         │
└─────────────────────────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────────┐
│                     JackettSearcher                              │
│  ┌─────────────────┐  ┌─────────────────────────────────────┐   │
│  │  Jackett Client │  │  RateLimiterPool                    │   │
│  │  - HTTP client  │  │  - Per-indexer token buckets        │   │
│  │  - API calls    │  │  - Acquire before search            │   │
│  └────────┬────────┘  └──────────────────┬──────────────────┘   │
│           │                              │                       │
│           └──────────────┬───────────────┘                       │
│                          ▼                                       │
│                   Jackett Server                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Rust Types

### Searcher Trait

```rust
// crates/core/src/searcher/mod.rs

use async_trait::async_trait;

/// Query parameters for a torrent search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Free-text search query
    pub query: String,
    /// Optional: limit to specific indexers
    pub indexers: Option<Vec<String>>,
    /// Optional: limit to specific categories (music, movies, tv, etc.)
    pub categories: Option<Vec<SearchCategory>>,
    /// Maximum results to return (default: 100)
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

/// A torrent search result (deduplicated by info_hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentCandidate {
    /// Torrent title (from first source)
    pub title: String,
    /// Info hash (lowercase hex) - used for deduplication
    pub info_hash: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Total seeders across all sources
    pub seeders: u32,
    /// Total leechers across all sources
    pub leechers: u32,
    /// Category as reported by indexer
    pub category: Option<String>,
    /// When the torrent was published (earliest across sources)
    pub publish_date: Option<DateTime<Utc>>,
    /// File list (if provided by any indexer)
    pub files: Option<Vec<TorrentFile>>,
    /// All indexers that have this torrent
    pub sources: Vec<TorrentSource>,
}

/// A single indexer's listing for a torrent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentSource {
    /// Which indexer returned this result
    pub indexer: String,
    /// Magnet URI from this indexer
    pub magnet_uri: Option<String>,
    /// .torrent download URL from this indexer
    pub torrent_url: Option<String>,
    /// Seeders reported by this indexer
    pub seeders: u32,
    /// Leechers reported by this indexer
    pub leechers: u32,
    /// Direct link to torrent page on this indexer
    pub details_url: Option<String>,
}

/// Raw result from a single indexer (before deduplication)
#[derive(Debug, Clone)]
pub(crate) struct RawTorrentResult {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    pub path: String,
    pub size_bytes: u64,
}

/// Search result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The search query that was executed
    pub query: SearchQuery,
    /// Deduplicated results (grouped by info_hash)
    pub candidates: Vec<TorrentCandidate>,
    /// How long the search took
    pub duration_ms: u64,
    /// Any indexers that failed (name -> error message)
    pub indexer_errors: HashMap<String, String>,
}

/// Deduplicate raw results by info_hash
fn deduplicate_results(raw: Vec<RawTorrentResult>) -> Vec<TorrentCandidate> {
    let mut by_hash: HashMap<String, TorrentCandidate> = HashMap::new();
    let mut no_hash: Vec<TorrentCandidate> = Vec::new();

    for r in raw {
        match r.info_hash {
            Some(hash) => {
                let hash = hash.to_lowercase();
                if let Some(existing) = by_hash.get_mut(&hash) {
                    // Add as additional source
                    existing.seeders += r.seeders;
                    existing.leechers += r.leechers;
                    existing.sources.push(TorrentSource {
                        indexer: r.indexer,
                        magnet_uri: r.magnet_uri,
                        torrent_url: r.torrent_url,
                        seeders: r.seeders,
                        leechers: r.leechers,
                        details_url: r.details_url,
                    });
                    // Keep earliest publish date
                    if let Some(date) = r.publish_date {
                        existing.publish_date = Some(match existing.publish_date {
                            Some(existing_date) => existing_date.min(date),
                            None => date,
                        });
                    }
                    // Keep files if we didn't have them
                    if existing.files.is_none() && r.files.is_some() {
                        existing.files = r.files;
                    }
                } else {
                    // First occurrence of this hash
                    by_hash.insert(hash.clone(), TorrentCandidate {
                        title: r.title,
                        info_hash: hash,
                        size_bytes: r.size_bytes,
                        seeders: r.seeders,
                        leechers: r.leechers,
                        category: r.category,
                        publish_date: r.publish_date,
                        files: r.files,
                        sources: vec![TorrentSource {
                            indexer: r.indexer,
                            magnet_uri: r.magnet_uri,
                            torrent_url: r.torrent_url,
                            seeders: r.seeders,
                            leechers: r.leechers,
                            details_url: r.details_url,
                        }],
                    });
                }
            }
            None => {
                // No info_hash - can't deduplicate, include as single-source result
                no_hash.push(TorrentCandidate {
                    title: r.title,
                    info_hash: String::new(), // Empty = unknown
                    size_bytes: r.size_bytes,
                    seeders: r.seeders,
                    leechers: r.leechers,
                    category: r.category,
                    publish_date: r.publish_date,
                    files: r.files,
                    sources: vec![TorrentSource {
                        indexer: r.indexer,
                        magnet_uri: r.magnet_uri,
                        torrent_url: r.torrent_url,
                        seeders: r.seeders,
                        leechers: r.leechers,
                        details_url: r.details_url,
                    }],
                });
            }
        }
    }

    let mut results: Vec<_> = by_hash.into_values().chain(no_hash).collect();
    // Sort by total seeders descending
    results.sort_by(|a, b| b.seeders.cmp(&a.seeders));
    results
}

#[async_trait]
pub trait Searcher: Send + Sync {
    /// Provider name for logging/audit
    fn name(&self) -> &str;

    /// Execute a search across configured indexers
    async fn search(&self, query: &SearchQuery) -> Result<SearchResult, SearchError>;

    /// Get status of all configured indexers
    async fn indexer_status(&self) -> Vec<IndexerStatus>;

    /// Update rate limit for an indexer (returns error if indexer not found)
    async fn update_indexer_rate_limit(
        &self,
        indexer: &str,
        requests_per_minute: u32,
    ) -> Result<(), SearchError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerStatus {
    pub name: String,
    pub enabled: bool,
    pub rate_limit: RateLimitStatus,
    pub last_used: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub requests_per_minute: u32,
    pub tokens_available: f32,
    pub next_available_in_ms: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Jackett connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Jackett API error: {0}")]
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
```

### Configuration Types

```rust
// crates/core/src/config.rs (additions)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearcherConfig {
    pub backend: SearcherBackend,
    #[serde(default)]
    pub jackett: Option<JackettConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearcherBackend {
    Jackett,
    // Future: Prowlarr, DirectApi
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JackettConfig {
    /// Jackett server URL (e.g., "http://localhost:9117")
    pub url: String,
    /// Jackett API key
    pub api_key: String,
    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    /// Configured indexers with rate limits
    #[serde(default)]
    pub indexers: Vec<IndexerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Indexer name (as shown in Jackett)
    pub name: String,
    /// Whether this indexer is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Rate limit: max requests per minute (default: 10)
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rpm: u32,
}

fn default_timeout() -> u32 { 30 }
fn default_true() -> bool { true }
fn default_rate_limit() -> u32 { 10 }
```

### Rate Limiter

```rust
// crates/core/src/searcher/rate_limiter.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

/// Token bucket rate limiter for a single indexer
pub struct TokenBucket {
    /// Max tokens (= requests per minute)
    capacity: f32,
    /// Current available tokens
    tokens: f32,
    /// Tokens added per second
    refill_rate: f32,
    /// Last refill time
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(requests_per_minute: u32) -> Self {
        let capacity = requests_per_minute as f32;
        Self {
            capacity,
            tokens: capacity, // Start full
            refill_rate: capacity / 60.0,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire a token. Returns Ok(()) if successful,
    /// Err(wait_duration) if rate limited.
    pub fn try_acquire(&mut self) -> Result<(), Duration> {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            // Calculate wait time until 1 token available
            let tokens_needed = 1.0 - self.tokens;
            let wait_secs = tokens_needed / self.refill_rate;
            Err(Duration::from_secs_f32(wait_secs))
        }
    }

    /// Update the rate limit
    pub fn set_rate_limit(&mut self, requests_per_minute: u32) {
        self.capacity = requests_per_minute as f32;
        self.refill_rate = self.capacity / 60.0;
        // Don't reset tokens - keep current state
        self.tokens = self.tokens.min(self.capacity);
    }

    /// Get current status
    pub fn status(&mut self) -> RateLimitStatus {
        self.refill();
        RateLimitStatus {
            requests_per_minute: self.capacity as u32,
            tokens_available: self.tokens,
            next_available_in_ms: if self.tokens >= 1.0 {
                None
            } else {
                let tokens_needed = 1.0 - self.tokens;
                Some((tokens_needed / self.refill_rate * 1000.0) as u64)
            },
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f32();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }
}

/// Pool of rate limiters, one per indexer
pub struct RateLimiterPool {
    limiters: RwLock<HashMap<String, TokenBucket>>,
}

impl RateLimiterPool {
    pub fn new(indexers: &[IndexerConfig]) -> Self {
        let mut limiters = HashMap::new();
        for indexer in indexers {
            limiters.insert(
                indexer.name.clone(),
                TokenBucket::new(indexer.rate_limit_rpm),
            );
        }
        Self {
            limiters: RwLock::new(limiters),
        }
    }

    /// Try to acquire a token for the given indexer
    pub async fn try_acquire(&self, indexer: &str) -> Result<(), SearchError> {
        let mut limiters = self.limiters.write().await;
        match limiters.get_mut(indexer) {
            Some(bucket) => match bucket.try_acquire() {
                Ok(()) => Ok(()),
                Err(wait) => Err(SearchError::RateLimited {
                    indexer: indexer.to_string(),
                    retry_after_ms: wait.as_millis() as u64,
                }),
            },
            None => Err(SearchError::IndexerNotFound(indexer.to_string())),
        }
    }

    /// Update rate limit for an indexer
    pub async fn set_rate_limit(&self, indexer: &str, rpm: u32) -> Result<(), SearchError> {
        let mut limiters = self.limiters.write().await;
        match limiters.get_mut(indexer) {
            Some(bucket) => {
                bucket.set_rate_limit(rpm);
                Ok(())
            }
            None => Err(SearchError::IndexerNotFound(indexer.to_string())),
        }
    }

    /// Get status of all indexers
    pub async fn status(&self) -> Vec<(String, RateLimitStatus)> {
        let mut limiters = self.limiters.write().await;
        limiters
            .iter_mut()
            .map(|(name, bucket)| (name.clone(), bucket.status()))
            .collect()
    }
}
```

## Jackett Client

```rust
// crates/core/src/searcher/jackett.rs

use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct JackettSearcher {
    client: Client,
    config: JackettConfig,
    rate_limiters: RateLimiterPool,
    /// Runtime state for indexers (last used, last error, etc.)
    indexer_state: RwLock<HashMap<String, IndexerState>>,
}

struct IndexerState {
    enabled: bool,
    last_used: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

impl JackettSearcher {
    pub fn new(config: JackettConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .expect("Failed to create HTTP client");

        let rate_limiters = RateLimiterPool::new(&config.indexers);

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

    /// Build Jackett API URL for search
    fn build_search_url(&self, query: &SearchQuery, indexer: &str) -> String {
        let mut url = format!(
            "{}/api/v2.0/indexers/{}/results?apikey={}&Query={}",
            self.config.url,
            indexer,
            self.config.api_key,
            urlencoding::encode(&query.query)
        );

        if let Some(categories) = &query.categories {
            let cat_ids: Vec<String> = categories
                .iter()
                .flat_map(|c| category_to_jackett_ids(c))
                .map(|id| id.to_string())
                .collect();
            if !cat_ids.is_empty() {
                url.push_str(&format!("&Category[]={}", cat_ids.join("&Category[]=")));
            }
        }

        url
    }

    /// Search a single indexer
    async fn search_indexer(
        &self,
        query: &SearchQuery,
        indexer: &str,
    ) -> Result<Vec<TorrentCandidate>, SearchError> {
        // Check rate limit first
        self.rate_limiters.try_acquire(indexer).await?;

        let url = self.build_search_url(query, indexer);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SearchError::ConnectionFailed(e.to_string()))?;

        if !response.status().is_success() {
            let error = format!("HTTP {}: {}", response.status(), response.text().await.unwrap_or_default());
            return Err(SearchError::ApiError(error));
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

        Ok(jackett_response
            .Results
            .into_iter()
            .map(|r| TorrentCandidate {
                title: r.Title,
                indexer: indexer.to_string(),
                magnet_uri: r.MagnetUri,
                torrent_url: r.Link,
                info_hash: r.InfoHash.map(|h| h.to_lowercase()),
                size_bytes: r.Size.unwrap_or(0) as u64,
                seeders: r.Seeders.unwrap_or(0) as u32,
                leechers: r.Peers.unwrap_or(0).saturating_sub(r.Seeders.unwrap_or(0)) as u32,
                category: r.CategoryDesc,
                publish_date: r.PublishDate.and_then(|d| d.parse().ok()),
                details_url: r.Details,
                files: None, // Jackett doesn't return file lists in search
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

        // Search all indexers concurrently
        let futures: Vec<_> = indexers_to_search
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

        let results = futures::future::join_all(futures).await;

        let mut candidates = Vec::new();
        let mut indexer_errors = HashMap::new();

        for (indexer, result) in results {
            match result {
                Ok(mut torrents) => {
                    candidates.append(&mut torrents);
                }
                Err(e) => {
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

        // Apply limit
        if let Some(limit) = query.limit {
            candidates.truncate(limit as usize);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // If all indexers failed, return error
        if candidates.is_empty() && !indexer_errors.is_empty() {
            return Err(SearchError::AllIndexersFailed(indexer_errors));
        }

        Ok(SearchResult {
            query: query.clone(),
            candidates,
            duration_ms,
            indexer_errors,
        })
    }

    async fn indexer_status(&self) -> Vec<IndexerStatus> {
        let state = self.indexer_state.read().await;
        let rate_status = self.rate_limiters.status().await;
        let rate_map: HashMap<_, _> = rate_status.into_iter().collect();

        self.config
            .indexers
            .iter()
            .map(|cfg| {
                let s = state.get(&cfg.name);
                IndexerStatus {
                    name: cfg.name.clone(),
                    enabled: s.map(|s| s.enabled).unwrap_or(cfg.enabled),
                    rate_limit: rate_map
                        .get(&cfg.name)
                        .cloned()
                        .unwrap_or(RateLimitStatus {
                            requests_per_minute: cfg.rate_limit_rpm,
                            tokens_available: cfg.rate_limit_rpm as f32,
                            next_available_in_ms: None,
                        }),
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

/// Map our categories to Jackett category IDs
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
```

## API Endpoints

### Search Endpoint

```
POST /api/v1/search
Content-Type: application/json

{
  "query": "Radiohead OK Computer FLAC",
  "indexers": ["rutracker", "redacted"],  // optional
  "categories": ["music"],                 // optional
  "limit": 50                              // optional, default 100
}

Response 200:
{
  "query": { ... },
  "candidates": [
    {
      "title": "Radiohead - OK Computer (1997) [FLAC]",
      "info_hash": "abc123def456...",
      "size_bytes": 524288000,
      "seeders": 87,
      "leechers": 5,
      "category": "Music/Lossless",
      "publish_date": "2020-05-15T10:30:00Z",
      "files": null,
      "sources": [
        {
          "indexer": "rutracker",
          "magnet_uri": "magnet:?xt=urn:btih:abc123...",
          "torrent_url": "https://rutracker.org/download/...",
          "seeders": 42,
          "leechers": 3,
          "details_url": "https://rutracker.org/topic/..."
        },
        {
          "indexer": "redacted",
          "magnet_uri": "magnet:?xt=urn:btih:abc123...",
          "torrent_url": null,
          "seeders": 30,
          "leechers": 1,
          "details_url": "https://redacted.ch/torrents/..."
        },
        {
          "indexer": "torrentleech",
          "magnet_uri": null,
          "torrent_url": "https://torrentleech.org/download/...",
          "seeders": 15,
          "leechers": 1,
          "details_url": "https://torrentleech.org/torrent/..."
        }
      ]
    },
    ...
  ],
  "duration_ms": 1234,
  "indexer_errors": {}
}

Response 400: Invalid request
Response 503: All indexers failed
```

### Searcher Status Endpoint

```
GET /api/v1/searcher/status

Response 200:
{
  "backend": "jackett",
  "jackett_url": "http://localhost:9117",
  "connected": true,
  "indexers_count": 3,
  "indexers_enabled": 2
}
```

### Indexers List Endpoint

```
GET /api/v1/searcher/indexers

Response 200:
{
  "indexers": [
    {
      "name": "rutracker",
      "enabled": true,
      "rate_limit": {
        "requests_per_minute": 10,
        "tokens_available": 8.5,
        "next_available_in_ms": null
      },
      "last_used": "2024-12-26T10:30:00Z",
      "last_error": null
    },
    {
      "name": "redacted",
      "enabled": true,
      "rate_limit": {
        "requests_per_minute": 5,
        "tokens_available": 0.2,
        "next_available_in_ms": 9600
      },
      "last_used": "2024-12-26T10:29:50Z",
      "last_error": null
    }
  ]
}
```

### Update Indexer Endpoint

```
PATCH /api/v1/searcher/indexers/{name}
Content-Type: application/json

{
  "rate_limit_rpm": 15,
  "enabled": true
}

Response 200:
{
  "name": "rutracker",
  "enabled": true,
  "rate_limit": {
    "requests_per_minute": 15,
    "tokens_available": 8.5,
    "next_available_in_ms": null
  },
  "last_used": "2024-12-26T10:30:00Z",
  "last_error": null
}

Response 404: Indexer not found
```

## Audit Events

```rust
// Add to crates/core/src/audit/events.rs

pub enum AuditEvent {
    // ... existing events ...

    /// A search was executed
    SearchExecuted {
        /// Who initiated the search (from auth)
        user_id: String,
        /// Search backend used
        searcher: String,
        /// The query that was searched
        query: String,
        /// Which indexers were queried
        indexers_queried: Vec<String>,
        /// Number of results returned
        results_count: u32,
        /// How long the search took
        duration_ms: u64,
        /// Any indexers that failed
        indexer_errors: HashMap<String, String>,
    },

    /// Indexer rate limit was updated
    IndexerRateLimitUpdated {
        user_id: String,
        indexer: String,
        old_rpm: u32,
        new_rpm: u32,
    },
}
```

## Configuration File Format

```toml
# config.toml additions

[searcher]
backend = "jackett"

[searcher.jackett]
url = "http://localhost:9117"
api_key = "your-jackett-api-key"
timeout_secs = 30

[[searcher.jackett.indexers]]
name = "rutracker"
enabled = true
rate_limit_rpm = 10

[[searcher.jackett.indexers]]
name = "redacted"
enabled = true
rate_limit_rpm = 5

[[searcher.jackett.indexers]]
name = "torrentleech"
enabled = false
rate_limit_rpm = 10
```

## Dashboard

### Project Structure Additions

```
crates/dashboard/src/
├── api/
│   └── searcher.ts          # Search API client
├── composables/
│   └── useSearcher.ts       # Search composable
├── components/
│   └── search/
│       ├── SearchForm.vue       # Query input + filters
│       ├── SearchResults.vue    # Results table
│       ├── IndexerList.vue      # List of indexers with status
│       └── IndexerSettings.vue  # Edit indexer modal
├── views/
│   ├── SearchView.vue       # Search testing page
│   └── SettingsView.vue     # Settings page (add indexers tab)
```

### TypeScript Types

```typescript
// src/api/types.ts additions

export interface SearchQuery {
  query: string;
  indexers?: string[];
  categories?: SearchCategory[];
  limit?: number;
}

export type SearchCategory = 'audio' | 'music' | 'movies' | 'tv' | 'books' | 'software' | 'other';

export interface TorrentCandidate {
  title: string;
  info_hash: string;
  size_bytes: number;
  seeders: number;       // Total across all sources
  leechers: number;      // Total across all sources
  category?: string;
  publish_date?: string;
  files?: TorrentFile[];
  sources: TorrentSource[];
}

export interface TorrentSource {
  indexer: string;
  magnet_uri?: string;
  torrent_url?: string;
  seeders: number;
  leechers: number;
  details_url?: string;
}

export interface TorrentFile {
  path: string;
  size_bytes: number;
}

export interface SearchResult {
  query: SearchQuery;
  candidates: TorrentCandidate[];
  duration_ms: number;
  indexer_errors: Record<string, string>;
}

export interface IndexerStatus {
  name: string;
  enabled: boolean;
  rate_limit: RateLimitStatus;
  last_used?: string;
  last_error?: string;
}

export interface RateLimitStatus {
  requests_per_minute: number;
  tokens_available: number;
  next_available_in_ms?: number;
}

export interface SearcherStatus {
  backend: string;
  jackett_url: string;
  connected: boolean;
  indexers_count: number;
  indexers_enabled: number;
}
```

### API Client

```typescript
// src/api/searcher.ts

import { apiClient } from './client';
import type { SearchQuery, SearchResult, SearcherStatus, IndexerStatus } from './types';

export async function search(query: SearchQuery): Promise<SearchResult> {
  return apiClient.post('/api/v1/search', query);
}

export async function getSearcherStatus(): Promise<SearcherStatus> {
  return apiClient.get('/api/v1/searcher/status');
}

export async function getIndexers(): Promise<{ indexers: IndexerStatus[] }> {
  return apiClient.get('/api/v1/searcher/indexers');
}

export async function updateIndexer(
  name: string,
  update: { rate_limit_rpm?: number; enabled?: boolean }
): Promise<IndexerStatus> {
  return apiClient.patch(`/api/v1/searcher/indexers/${name}`, update);
}
```

### Composable

```typescript
// src/composables/useSearcher.ts

import { ref, computed } from 'vue';
import * as searcherApi from '@/api/searcher';
import type { SearchQuery, SearchResult, IndexerStatus, SearcherStatus } from '@/api/types';

export function useSearcher() {
  const searchResult = ref<SearchResult | null>(null);
  const indexers = ref<IndexerStatus[]>([]);
  const status = ref<SearcherStatus | null>(null);
  const isSearching = ref(false);
  const isLoading = ref(false);
  const error = ref<string | null>(null);

  async function search(query: SearchQuery) {
    isSearching.value = true;
    error.value = null;
    try {
      searchResult.value = await searcherApi.search(query);
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Search failed';
      throw e;
    } finally {
      isSearching.value = false;
    }
  }

  async function fetchStatus() {
    isLoading.value = true;
    try {
      status.value = await searcherApi.getSearcherStatus();
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch status';
    } finally {
      isLoading.value = false;
    }
  }

  async function fetchIndexers() {
    isLoading.value = true;
    try {
      const response = await searcherApi.getIndexers();
      indexers.value = response.indexers;
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch indexers';
    } finally {
      isLoading.value = false;
    }
  }

  async function updateIndexer(name: string, update: { rate_limit_rpm?: number; enabled?: boolean }) {
    try {
      const updated = await searcherApi.updateIndexer(name, update);
      const idx = indexers.value.findIndex(i => i.name === name);
      if (idx !== -1) {
        indexers.value[idx] = updated;
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to update indexer';
      throw e;
    }
  }

  const enabledIndexers = computed(() => indexers.value.filter(i => i.enabled));

  return {
    searchResult,
    indexers,
    status,
    isSearching,
    isLoading,
    error,
    enabledIndexers,
    search,
    fetchStatus,
    fetchIndexers,
    updateIndexer,
  };
}
```

### Views

#### Search View

```vue
<!-- src/views/SearchView.vue -->
<template>
  <AppLayout>
    <div class="p-6">
      <h1 class="text-2xl font-bold mb-6">Search Testing</h1>

      <!-- Search Form -->
      <SearchForm
        :indexers="enabledIndexers"
        :is-searching="isSearching"
        @search="handleSearch"
      />

      <!-- Error -->
      <ErrorAlert v-if="error" :message="error" class="mt-4" />

      <!-- Results -->
      <SearchResults
        v-if="searchResult"
        :result="searchResult"
        class="mt-6"
        @copy-magnet="copyMagnet"
      />
    </div>
  </AppLayout>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useSearcher } from '@/composables/useSearcher';
import AppLayout from '@/components/layout/AppLayout.vue';
import SearchForm from '@/components/search/SearchForm.vue';
import SearchResults from '@/components/search/SearchResults.vue';
import ErrorAlert from '@/components/common/ErrorAlert.vue';
import type { SearchQuery } from '@/api/types';

const {
  searchResult,
  enabledIndexers,
  isSearching,
  error,
  search,
  fetchIndexers,
} = useSearcher();

onMounted(() => {
  fetchIndexers();
});

async function handleSearch(query: SearchQuery) {
  await search(query);
}

function copyMagnet(magnet: string) {
  navigator.clipboard.writeText(magnet);
  // TODO: Show toast notification
}
</script>
```

### Routes

```typescript
// src/router/index.ts additions

{
  path: '/search',
  name: 'search',
  component: () => import('@/views/SearchView.vue'),
},
{
  path: '/settings',
  name: 'settings',
  component: () => import('@/views/SettingsView.vue'),
},
```

### Sidebar Navigation

Add to AppSidebar.vue:
- Search (icon: magnifying glass) -> `/search`
- Settings (icon: gear) -> `/settings`

## Implementation Tasks

### Task 1: Core Types & Trait
**Files**: `crates/core/src/searcher/mod.rs`, `crates/core/src/searcher/types.rs`

- [x] Create searcher module structure
- [x] Define `SearchQuery`, `TorrentCandidate`, `SearchResult` types
- [x] Define `SearchCategory` enum
- [x] Define `Searcher` trait
- [x] Define `SearchError` type
- [x] Define `IndexerStatus`, `RateLimitStatus` types
- [x] Export from `crates/core/src/lib.rs`

### Task 2: Rate Limiter
**Files**: `crates/core/src/searcher/rate_limiter.rs`

- [ ] Implement `TokenBucket` struct
- [ ] Implement `try_acquire`, `set_rate_limit`, `status` methods
- [ ] Implement `RateLimiterPool` for multiple indexers
- [ ] Write unit tests for rate limiting logic

### Task 3: Configuration
**Files**: `crates/core/src/config.rs`

- [ ] Add `SearcherConfig` struct
- [ ] Add `JackettConfig` struct
- [ ] Add `IndexerConfig` struct
- [ ] Add to main `Config` struct
- [ ] Add to `SanitizedConfig` (hide API key)
- [ ] Write config parsing tests

### Task 4: Jackett Client
**Files**: `crates/core/src/searcher/jackett.rs`, `crates/core/src/searcher/dedup.rs`

- [ ] Implement `JackettSearcher` struct
- [ ] Implement Jackett API response parsing
- [ ] Implement `Searcher` trait for `JackettSearcher`
- [ ] Implement concurrent multi-indexer search
- [ ] Implement result deduplication by info_hash
- [ ] Handle rate limiting integration
- [ ] Track indexer state (last used, errors)
- [ ] Write unit tests for deduplication logic
- [ ] Write integration tests (with mock server)

### Task 5: Audit Events
**Files**: `crates/core/src/audit/events.rs`

- [ ] Add `SearchExecuted` event
- [ ] Add `IndexerRateLimitUpdated` event
- [ ] Update event serialization

### Task 6: API Endpoints
**Files**: `crates/server/src/api/searcher.rs`, `crates/server/src/api/routes.rs`

- [ ] Create searcher handlers module
- [ ] Implement `POST /api/v1/search` handler
- [ ] Implement `GET /api/v1/searcher/status` handler
- [ ] Implement `GET /api/v1/searcher/indexers` handler
- [ ] Implement `PATCH /api/v1/searcher/indexers/{name}` handler
- [ ] Add searcher to `AppState`
- [ ] Register routes
- [ ] Emit audit events

### Task 7: Dashboard API Client
**Files**: `crates/dashboard/src/api/searcher.ts`, `crates/dashboard/src/api/types.ts`

- [ ] Add TypeScript types
- [ ] Implement `search()` function
- [ ] Implement `getSearcherStatus()` function
- [ ] Implement `getIndexers()` function
- [ ] Implement `updateIndexer()` function

### Task 8: Dashboard Composable
**Files**: `crates/dashboard/src/composables/useSearcher.ts`

- [ ] Create `useSearcher` composable
- [ ] Implement search functionality
- [ ] Implement indexer management
- [ ] Handle loading/error states

### Task 9: Dashboard Components
**Files**: `crates/dashboard/src/components/search/*`

- [ ] Create `SearchForm.vue` (query input, indexer checkboxes, category filter)
- [ ] Create `SearchResults.vue` (results table with sorting)
- [ ] Create `IndexerList.vue` (indexer cards with status)
- [ ] Create `IndexerSettings.vue` (edit modal)

### Task 10: Dashboard Views
**Files**: `crates/dashboard/src/views/SearchView.vue`, `crates/dashboard/src/views/SettingsView.vue`

- [ ] Create `SearchView.vue`
- [ ] Create `SettingsView.vue` with indexers tab
- [ ] Add routes
- [ ] Add sidebar navigation

### Task 11: Testing & Polish

- [ ] Unit tests for rate limiter
- [ ] Unit tests for Jackett response parsing
- [ ] Integration test with mock Jackett server
- [ ] Manual testing with real Jackett
- [ ] Error handling edge cases
- [ ] Loading states in dashboard

## Success Criteria

- [ ] `cargo build` succeeds with new searcher module
- [ ] `cargo test` passes all new tests
- [ ] `npm run build` succeeds in dashboard
- [ ] `npm run type-check` passes
- [ ] Can configure Jackett in `config.toml`
- [ ] Search from dashboard returns results
- [ ] Rate limiting prevents excessive requests
- [ ] Can view indexer status in dashboard
- [ ] Can update indexer rate limits from dashboard
- [ ] Audit events logged for searches

## Manual Testing Guide

### 1. Start Jackett
```bash
# Ensure Jackett is running at http://localhost:9117
# Configure at least one indexer in Jackett UI
```

### 2. Configure Quentin
```toml
# config.toml
[searcher]
backend = "jackett"

[searcher.jackett]
url = "http://localhost:9117"
api_key = "your-api-key"

[[searcher.jackett.indexers]]
name = "your-indexer"
rate_limit_rpm = 10
```

### 3. Test Search Flow
1. Start backend: `cargo run -p torrentino-server`
2. Start dashboard: `cd crates/dashboard && npm run dev`
3. Navigate to Search page
4. Enter a query and click Search
5. Verify results appear
6. Copy a magnet URI

### 4. Test Rate Limiting
1. Set an indexer to 1 request per minute
2. Run multiple searches quickly
3. Verify rate limit error appears
4. Wait and verify search works again

### 5. Test Indexer Settings
1. Navigate to Settings > Indexers
2. View indexer status (tokens, last used)
3. Update rate limit
4. Verify change persists

## Dependencies

### Rust (additions to Cargo.toml)

```toml
# crates/core/Cargo.toml
[dependencies]
reqwest = { version = "0.11", features = ["json"] }
urlencoding = "2.1"
futures = "0.3"
```

### Dashboard (no new dependencies needed)

Existing `fetch` wrapper and Vue/TypeScript setup are sufficient.
