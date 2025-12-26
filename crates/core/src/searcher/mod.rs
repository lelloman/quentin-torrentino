//! Torrent search abstraction.
//!
//! This module provides a `Searcher` trait for searching torrents across
//! various backends (Jackett, Prowlarr, etc.).

mod dedup;
mod jackett;
mod types;

// Rate limiter kept for potential future use
#[allow(dead_code)]
mod rate_limiter;

pub use dedup::deduplicate_results;
pub use jackett::JackettSearcher;
pub use types::*;
