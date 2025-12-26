//! Torrent search abstraction.
//!
//! This module provides a `Searcher` trait for searching torrents across
//! various backends (Jackett, Prowlarr, etc.) with per-indexer rate limiting.

mod rate_limiter;
mod types;

pub use rate_limiter::{IndexerRateLimitConfig, RateLimiterPool, TokenBucket};
pub use types::*;

// Jackett implementation will be added in Task 4
