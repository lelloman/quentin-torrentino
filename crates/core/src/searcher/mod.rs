//! Torrent search abstraction.
//!
//! This module provides a `Searcher` trait for searching torrents across
//! various backends (Jackett, Prowlarr, etc.) with per-indexer rate limiting.

mod dedup;
mod jackett;
mod rate_limiter;
mod types;

pub use dedup::deduplicate_results;
pub use jackett::JackettSearcher;
pub use rate_limiter::{IndexerRateLimitConfig, RateLimiterPool, TokenBucket};
pub use types::*;
