//! Mock searcher for testing.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::searcher::{
    IndexerStatus, SearchError, SearchQuery, SearchResult, Searcher, TorrentCandidate,
};

/// A recorded search for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedSearch {
    /// The query that was searched.
    pub query: SearchQuery,
    /// When the search was made.
    pub timestamp: std::time::Instant,
}

/// A query handler that produces results dynamically based on the query.
type QueryHandler = Box<dyn Fn(&str) -> Option<Vec<TorrentCandidate>> + Send + Sync>;

/// Mock implementation of the Searcher trait.
///
/// Provides controllable behavior for testing:
/// - Return configurable search results
/// - Track search queries for assertions
/// - Simulate failures and delays
///
/// # Example
///
/// ```rust,ignore
/// use torrentino_core::testing::{MockSearcher, fixtures};
///
/// let searcher = MockSearcher::new();
///
/// // Configure results
/// searcher.set_results(vec![
///     fixtures::audio_candidate("The Beatles", "Abbey Road", "abc123"),
///     fixtures::audio_candidate("The Beatles", "Let It Be", "def456"),
/// ]).await;
///
/// // Search
/// let result = searcher.search(&SearchQuery { query: "beatles".into(), .. }).await?;
/// assert_eq!(result.candidates.len(), 2);
///
/// // Check what was searched
/// let searches = searcher.recorded_searches().await;
/// assert_eq!(searches.len(), 1);
/// assert!(searches[0].query.query.contains("beatles"));
/// ```
pub struct MockSearcher {
    /// Configured results to return.
    results: Arc<RwLock<Vec<TorrentCandidate>>>,
    /// Recorded search queries.
    searches: Arc<RwLock<Vec<RecordedSearch>>>,
    /// If set, the next search will fail with this error.
    next_error: Arc<RwLock<Option<SearchError>>>,
    /// Simulated indexer errors.
    indexer_errors: Arc<RwLock<HashMap<String, String>>>,
    /// Configured indexer statuses.
    indexers: Arc<RwLock<Vec<IndexerStatus>>>,
    /// Filter function for results (optional).
    result_filter: Arc<RwLock<Option<Box<dyn Fn(&SearchQuery, &TorrentCandidate) -> bool + Send + Sync>>>>,
    /// Query handler for dynamic result generation based on query string.
    query_handler: Arc<RwLock<Option<QueryHandler>>>,
}

impl std::fmt::Debug for MockSearcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockSearcher")
            .field("results", &"<results>")
            .field("searches", &"<searches>")
            .field("next_error", &"<next_error>")
            .field("indexer_errors", &"<indexer_errors>")
            .field("indexers", &"<indexers>")
            .field("result_filter", &"<filter>")
            .field("query_handler", &"<handler>")
            .finish()
    }
}

impl Default for MockSearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSearcher {
    /// Create a new mock searcher with empty results.
    pub fn new() -> Self {
        Self {
            results: Arc::new(RwLock::new(Vec::new())),
            searches: Arc::new(RwLock::new(Vec::new())),
            next_error: Arc::new(RwLock::new(None)),
            indexer_errors: Arc::new(RwLock::new(HashMap::new())),
            indexers: Arc::new(RwLock::new(vec![
                IndexerStatus {
                    name: "mock-indexer-1".to_string(),
                    enabled: true,
                },
                IndexerStatus {
                    name: "mock-indexer-2".to_string(),
                    enabled: true,
                },
            ])),
            result_filter: Arc::new(RwLock::new(None)),
            query_handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a mock searcher with predefined results.
    pub fn with_results(results: Vec<TorrentCandidate>) -> Self {
        let searcher = Self::new();
        // Use blocking approach for constructor
        *searcher.results.blocking_write() = results;
        searcher
    }

    /// Set the results to return for subsequent searches.
    pub async fn set_results(&self, results: Vec<TorrentCandidate>) {
        *self.results.write().await = results;
    }

    /// Add a single result.
    pub async fn add_result(&self, result: TorrentCandidate) {
        self.results.write().await.push(result);
    }

    /// Clear all results.
    pub async fn clear_results(&self) {
        self.results.write().await.clear();
    }

    /// Get recorded search queries.
    pub async fn recorded_searches(&self) -> Vec<RecordedSearch> {
        self.searches.read().await.clone()
    }

    /// Clear recorded searches.
    pub async fn clear_recorded(&self) {
        self.searches.write().await.clear();
    }

    /// Get the number of searches performed.
    pub async fn search_count(&self) -> usize {
        self.searches.read().await.len()
    }

    /// Configure the next search to fail with the given error.
    pub async fn set_next_error(&self, error: SearchError) {
        *self.next_error.write().await = Some(error);
    }

    /// Clear any pending error.
    pub async fn clear_next_error(&self) {
        *self.next_error.write().await = None;
    }

    /// Set simulated indexer errors (name -> error message).
    pub async fn set_indexer_errors(&self, errors: HashMap<String, String>) {
        *self.indexer_errors.write().await = errors;
    }

    /// Add a simulated indexer error.
    pub async fn add_indexer_error(&self, indexer: &str, error: &str) {
        self.indexer_errors
            .write()
            .await
            .insert(indexer.to_string(), error.to_string());
    }

    /// Clear indexer errors.
    pub async fn clear_indexer_errors(&self) {
        self.indexer_errors.write().await.clear();
    }

    /// Set the indexer statuses.
    pub async fn set_indexers(&self, indexers: Vec<IndexerStatus>) {
        *self.indexers.write().await = indexers;
    }

    /// Set a filter function that determines which results to return based on the query.
    ///
    /// This allows dynamic filtering based on search queries.
    pub async fn set_result_filter<F>(&self, filter: F)
    where
        F: Fn(&SearchQuery, &TorrentCandidate) -> bool + Send + Sync + 'static,
    {
        *self.result_filter.write().await = Some(Box::new(filter));
    }

    /// Clear the result filter.
    pub async fn clear_result_filter(&self) {
        *self.result_filter.write().await = None;
    }

    /// Set a query handler that dynamically generates results based on the query string.
    ///
    /// This is useful for testing fallback scenarios where different queries should
    /// return different results. The handler receives the query string and should return
    /// `Some(results)` to override the default results, or `None` to use the default behavior.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// searcher.set_query_handler(|query| {
    ///     if query.contains("discography") {
    ///         Some(vec![discography_candidate])
    ///     } else {
    ///         Some(vec![]) // Return empty for other queries
    ///     }
    /// }).await;
    /// ```
    pub async fn set_query_handler<F>(&self, handler: F)
    where
        F: Fn(&str) -> Option<Vec<TorrentCandidate>> + Send + Sync + 'static,
    {
        *self.query_handler.write().await = Some(Box::new(handler));
    }

    /// Clear the query handler.
    pub async fn clear_query_handler(&self) {
        *self.query_handler.write().await = None;
    }

    /// Take the next error if set.
    async fn take_error(&self) -> Option<SearchError> {
        self.next_error.write().await.take()
    }
}

#[async_trait]
impl Searcher for MockSearcher {
    fn name(&self) -> &str {
        "mock"
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchResult, SearchError> {
        // Check for injected error
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        // Record the search
        self.searches.write().await.push(RecordedSearch {
            query: query.clone(),
            timestamp: Instant::now(),
        });

        // Check if query handler provides results
        let handler = self.query_handler.read().await;
        if let Some(ref h) = *handler {
            if let Some(handler_results) = h(&query.query) {
                // Apply limit if specified
                let candidates = if let Some(limit) = query.limit {
                    handler_results.into_iter().take(limit as usize).collect()
                } else {
                    handler_results
                };

                return Ok(SearchResult {
                    query: query.clone(),
                    candidates,
                    duration_ms: 50,
                    indexer_errors: self.indexer_errors.read().await.clone(),
                });
            }
        }
        drop(handler);

        // Get results, optionally filtered
        let all_results = self.results.read().await;
        let filter = self.result_filter.read().await;

        let candidates: Vec<TorrentCandidate> = if let Some(ref f) = *filter {
            all_results
                .iter()
                .filter(|c| f(query, c))
                .cloned()
                .collect()
        } else {
            // Default: filter by query string in title (case-insensitive)
            let query_lower = query.query.to_lowercase();
            all_results
                .iter()
                .filter(|c| {
                    query_lower.is_empty()
                        || c.title.to_lowercase().contains(&query_lower)
                        || query_lower
                            .split_whitespace()
                            .all(|word| c.title.to_lowercase().contains(word))
                })
                .cloned()
                .collect()
        };

        // Apply limit if specified
        let candidates = if let Some(limit) = query.limit {
            candidates.into_iter().take(limit as usize).collect()
        } else {
            candidates
        };

        Ok(SearchResult {
            query: query.clone(),
            candidates,
            duration_ms: 50, // Simulated fast search
            indexer_errors: self.indexer_errors.read().await.clone(),
        })
    }

    async fn indexer_status(&self) -> Vec<IndexerStatus> {
        self.indexers.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures;

    #[tokio::test]
    async fn test_basic_search() {
        let searcher = MockSearcher::new();
        searcher
            .set_results(vec![
                fixtures::audio_candidate("The Beatles", "Abbey Road", "abc123"),
                fixtures::audio_candidate("The Beatles", "Let It Be", "def456"),
            ])
            .await;

        let result = searcher
            .search(&SearchQuery {
                query: "beatles".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(result.candidates.len(), 2);
    }

    #[tokio::test]
    async fn test_query_filtering() {
        let searcher = MockSearcher::new();
        searcher
            .set_results(vec![
                fixtures::audio_candidate("The Beatles", "Abbey Road", "abc123"),
                fixtures::video_candidate("The Matrix", 1999, "matrix1"),
            ])
            .await;

        let result = searcher
            .search(&SearchQuery {
                query: "matrix".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert!(result.candidates[0].title.contains("Matrix"));
    }

    #[tokio::test]
    async fn test_recorded_searches() {
        let searcher = MockSearcher::new();

        searcher
            .search(&SearchQuery {
                query: "first".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await
            .unwrap();

        searcher
            .search(&SearchQuery {
                query: "second".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await
            .unwrap();

        let searches = searcher.recorded_searches().await;
        assert_eq!(searches.len(), 2);
        assert_eq!(searches[0].query.query, "first");
        assert_eq!(searches[1].query.query, "second");
    }

    #[tokio::test]
    async fn test_error_injection() {
        let searcher = MockSearcher::new();
        searcher
            .set_next_error(SearchError::ConnectionFailed("test error".into()))
            .await;

        let result = searcher
            .search(&SearchQuery {
                query: "test".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await;

        assert!(result.is_err());

        // Error should be consumed
        let result = searcher
            .search(&SearchQuery {
                query: "test".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_indexer_errors() {
        let searcher = MockSearcher::new();
        searcher
            .add_indexer_error("broken-indexer", "Connection timeout")
            .await;

        let result = searcher
            .search(&SearchQuery {
                query: "".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await
            .unwrap();

        assert!(result.indexer_errors.contains_key("broken-indexer"));
    }

    #[tokio::test]
    async fn test_limit() {
        let searcher = MockSearcher::new();
        searcher
            .set_results(vec![
                fixtures::audio_candidate("Artist 1", "Album 1", "hash1"),
                fixtures::audio_candidate("Artist 2", "Album 2", "hash2"),
                fixtures::audio_candidate("Artist 3", "Album 3", "hash3"),
            ])
            .await;

        let result = searcher
            .search(&SearchQuery {
                query: "".into(),
                indexers: None,
                categories: None,
                limit: Some(2),
            })
            .await
            .unwrap();

        assert_eq!(result.candidates.len(), 2);
    }

    #[tokio::test]
    async fn test_custom_filter() {
        let searcher = MockSearcher::new();
        searcher
            .set_results(vec![
                fixtures::audio_candidate("Small", "Album", "small"),
                fixtures::video_candidate("Large", 2024, "large"),
            ])
            .await;

        // Filter by size
        searcher
            .set_result_filter(|_query, candidate| candidate.size_bytes < 1024 * 1024 * 500)
            .await;

        let result = searcher
            .search(&SearchQuery {
                query: "".into(),
                indexers: None,
                categories: None,
                limit: None,
            })
            .await
            .unwrap();

        // Only the small audio file should match
        assert_eq!(result.candidates.len(), 1);
        assert!(result.candidates[0].title.contains("Small"));
    }
}
