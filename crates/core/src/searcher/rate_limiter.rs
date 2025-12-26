//! Token bucket rate limiter for per-indexer rate limiting.
//!
//! Kept for potential future use but currently not used since Jackett
//! handles rate limiting internally.

use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use super::SearchError;

/// Rate limit status for an indexer.
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub requests_per_minute: u32,
    pub tokens_available: f32,
    pub next_available_in_ms: Option<u64>,
}

/// Token bucket rate limiter for a single indexer.
///
/// Uses the token bucket algorithm where tokens are added at a constant rate
/// and consumed when requests are made. If no tokens are available, the request
/// must wait.
pub struct TokenBucket {
    /// Max tokens (= requests per minute).
    capacity: f32,
    /// Current available tokens.
    tokens: f32,
    /// Tokens added per second.
    refill_rate: f32,
    /// Last refill time.
    last_refill: Instant,
}

impl TokenBucket {
    /// Create a new token bucket with the given rate limit.
    ///
    /// The bucket starts full, allowing immediate requests up to the capacity.
    pub fn new(requests_per_minute: u32) -> Self {
        let capacity = requests_per_minute as f32;
        Self {
            capacity,
            tokens: capacity, // Start full
            refill_rate: capacity / 60.0,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire a token.
    ///
    /// Returns `Ok(())` if a token was acquired successfully.
    /// Returns `Err(wait_duration)` if rate limited, with the duration to wait.
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

    /// Update the rate limit.
    ///
    /// The current token count is preserved (clamped to new capacity).
    pub fn set_rate_limit(&mut self, requests_per_minute: u32) {
        self.capacity = requests_per_minute as f32;
        self.refill_rate = self.capacity / 60.0;
        // Don't reset tokens - keep current state, but clamp to new capacity
        self.tokens = self.tokens.min(self.capacity);
    }

    /// Get the current rate limit status.
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

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f32();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }
}

/// Configuration for an indexer in the rate limiter pool.
#[derive(Debug, Clone)]
pub struct IndexerRateLimitConfig {
    /// Indexer name.
    pub name: String,
    /// Rate limit: max requests per minute.
    pub rate_limit_rpm: u32,
}

/// Pool of rate limiters, one per indexer.
///
/// Thread-safe and async-compatible.
pub struct RateLimiterPool {
    limiters: RwLock<HashMap<String, TokenBucket>>,
}

impl RateLimiterPool {
    /// Create a new rate limiter pool with the given indexer configurations.
    pub fn new(indexers: &[IndexerRateLimitConfig]) -> Self {
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

    /// Create an empty rate limiter pool.
    pub fn empty() -> Self {
        Self {
            limiters: RwLock::new(HashMap::new()),
        }
    }

    /// Add or update an indexer in the pool.
    pub async fn add_indexer(&self, name: &str, rate_limit_rpm: u32) {
        let mut limiters = self.limiters.write().await;
        if let Some(bucket) = limiters.get_mut(name) {
            bucket.set_rate_limit(rate_limit_rpm);
        } else {
            limiters.insert(name.to_string(), TokenBucket::new(rate_limit_rpm));
        }
    }

    /// Remove an indexer from the pool.
    pub async fn remove_indexer(&self, name: &str) -> bool {
        let mut limiters = self.limiters.write().await;
        limiters.remove(name).is_some()
    }

    /// Try to acquire a token for the given indexer.
    ///
    /// Returns `Ok(())` if a token was acquired.
    /// Returns `Err(SearchError::RateLimited)` if rate limited.
    /// Returns `Err(SearchError::IndexerNotFound)` if the indexer doesn't exist.
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

    /// Update rate limit for an indexer.
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

    /// Get rate limit status for a specific indexer.
    pub async fn get_status(&self, indexer: &str) -> Option<RateLimitStatus> {
        let mut limiters = self.limiters.write().await;
        limiters.get_mut(indexer).map(|bucket| bucket.status())
    }

    /// Get status of all indexers.
    pub async fn all_status(&self) -> Vec<(String, RateLimitStatus)> {
        let mut limiters = self.limiters.write().await;
        limiters
            .iter_mut()
            .map(|(name, bucket)| (name.clone(), bucket.status()))
            .collect()
    }

    /// Check if an indexer exists in the pool.
    pub async fn has_indexer(&self, indexer: &str) -> bool {
        let limiters = self.limiters.read().await;
        limiters.contains_key(indexer)
    }

    /// Get the list of indexer names.
    pub async fn indexer_names(&self) -> Vec<String> {
        let limiters = self.limiters.read().await;
        limiters.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_token_bucket_new() {
        let bucket = TokenBucket::new(10);
        assert_eq!(bucket.capacity, 10.0);
        assert_eq!(bucket.tokens, 10.0);
        assert!((bucket.refill_rate - 10.0 / 60.0).abs() < 0.001);
    }

    #[test]
    fn test_token_bucket_acquire_success() {
        let mut bucket = TokenBucket::new(10);

        // Should succeed 10 times (full bucket)
        for _ in 0..10 {
            assert!(bucket.try_acquire().is_ok());
        }

        // 11th should fail
        assert!(bucket.try_acquire().is_err());
    }

    #[test]
    fn test_token_bucket_acquire_returns_wait_time() {
        let mut bucket = TokenBucket::new(10);

        // Drain all tokens
        for _ in 0..10 {
            bucket.try_acquire().unwrap();
        }

        // Should return wait time
        let err = bucket.try_acquire().unwrap_err();
        // At 10 rpm, 1 token takes 6 seconds to refill
        assert!(err.as_secs() <= 6);
        assert!(err.as_millis() > 0);
    }

    #[test]
    fn test_token_bucket_set_rate_limit() {
        let mut bucket = TokenBucket::new(10);

        // Use some tokens
        for _ in 0..5 {
            bucket.try_acquire().unwrap();
        }
        assert_eq!(bucket.tokens, 5.0);

        // Increase rate limit - tokens should stay at 5
        bucket.set_rate_limit(20);
        assert_eq!(bucket.capacity, 20.0);
        assert_eq!(bucket.tokens, 5.0);

        // Decrease rate limit - tokens should be clamped
        bucket.set_rate_limit(3);
        assert_eq!(bucket.capacity, 3.0);
        assert_eq!(bucket.tokens, 3.0);
    }

    #[test]
    fn test_token_bucket_status() {
        let mut bucket = TokenBucket::new(10);

        let status = bucket.status();
        assert_eq!(status.requests_per_minute, 10);
        assert!(status.tokens_available >= 9.9); // Allow for tiny refill
        assert!(status.next_available_in_ms.is_none());

        // Drain all tokens
        for _ in 0..10 {
            bucket.try_acquire().unwrap();
        }

        let status = bucket.status();
        assert!(status.tokens_available < 1.0);
        assert!(status.next_available_in_ms.is_some());
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(60); // 1 token per second

        // Drain all tokens
        for _ in 0..60 {
            bucket.try_acquire().unwrap();
        }
        assert!(bucket.tokens < 1.0);

        // Wait a bit and check refill
        sleep(Duration::from_millis(100)).await;
        bucket.refill();

        // Should have refilled ~0.1 tokens
        assert!(bucket.tokens > 0.05);
        assert!(bucket.tokens < 0.2);
    }

    #[tokio::test]
    async fn test_rate_limiter_pool_new() {
        let configs = vec![
            IndexerRateLimitConfig {
                name: "indexer1".to_string(),
                rate_limit_rpm: 10,
            },
            IndexerRateLimitConfig {
                name: "indexer2".to_string(),
                rate_limit_rpm: 5,
            },
        ];

        let pool = RateLimiterPool::new(&configs);
        assert!(pool.has_indexer("indexer1").await);
        assert!(pool.has_indexer("indexer2").await);
        assert!(!pool.has_indexer("indexer3").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_pool_try_acquire() {
        let configs = vec![IndexerRateLimitConfig {
            name: "test".to_string(),
            rate_limit_rpm: 2,
        }];

        let pool = RateLimiterPool::new(&configs);

        // Should succeed twice
        assert!(pool.try_acquire("test").await.is_ok());
        assert!(pool.try_acquire("test").await.is_ok());

        // Third should be rate limited
        let err = pool.try_acquire("test").await.unwrap_err();
        match err {
            SearchError::RateLimited { indexer, .. } => {
                assert_eq!(indexer, "test");
            }
            _ => panic!("Expected RateLimited error"),
        }

        // Unknown indexer should error
        let err = pool.try_acquire("unknown").await.unwrap_err();
        match err {
            SearchError::IndexerNotFound(name) => {
                assert_eq!(name, "unknown");
            }
            _ => panic!("Expected IndexerNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_pool_set_rate_limit() {
        let configs = vec![IndexerRateLimitConfig {
            name: "test".to_string(),
            rate_limit_rpm: 10,
        }];

        let pool = RateLimiterPool::new(&configs);

        // Update rate limit
        pool.set_rate_limit("test", 20).await.unwrap();

        let status = pool.get_status("test").await.unwrap();
        assert_eq!(status.requests_per_minute, 20);

        // Unknown indexer should error
        let err = pool.set_rate_limit("unknown", 10).await.unwrap_err();
        match err {
            SearchError::IndexerNotFound(name) => {
                assert_eq!(name, "unknown");
            }
            _ => panic!("Expected IndexerNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_pool_add_remove_indexer() {
        let pool = RateLimiterPool::empty();

        assert!(!pool.has_indexer("test").await);

        // Add indexer
        pool.add_indexer("test", 10).await;
        assert!(pool.has_indexer("test").await);

        // Update existing indexer
        pool.add_indexer("test", 20).await;
        let status = pool.get_status("test").await.unwrap();
        assert_eq!(status.requests_per_minute, 20);

        // Remove indexer
        assert!(pool.remove_indexer("test").await);
        assert!(!pool.has_indexer("test").await);

        // Remove non-existent
        assert!(!pool.remove_indexer("test").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_pool_all_status() {
        let configs = vec![
            IndexerRateLimitConfig {
                name: "a".to_string(),
                rate_limit_rpm: 10,
            },
            IndexerRateLimitConfig {
                name: "b".to_string(),
                rate_limit_rpm: 5,
            },
        ];

        let pool = RateLimiterPool::new(&configs);

        let statuses = pool.all_status().await;
        assert_eq!(statuses.len(), 2);

        let status_map: HashMap<_, _> = statuses.into_iter().collect();
        assert_eq!(status_map.get("a").unwrap().requests_per_minute, 10);
        assert_eq!(status_map.get("b").unwrap().requests_per_minute, 5);
    }

    #[tokio::test]
    async fn test_rate_limiter_pool_indexer_names() {
        let configs = vec![
            IndexerRateLimitConfig {
                name: "alpha".to_string(),
                rate_limit_rpm: 10,
            },
            IndexerRateLimitConfig {
                name: "beta".to_string(),
                rate_limit_rpm: 5,
            },
        ];

        let pool = RateLimiterPool::new(&configs);

        let mut names = pool.indexer_names().await;
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }
}
