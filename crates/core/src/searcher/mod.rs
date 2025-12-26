//! Torrent search abstraction.
//!
//! This module provides a `Searcher` trait for searching torrents across
//! various backends (Jackett, Prowlarr, etc.) with per-indexer rate limiting.

mod types;

pub use types::*;

// Rate limiter will be added in Task 2
// Jackett implementation will be added in Task 4
