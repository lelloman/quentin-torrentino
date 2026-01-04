//! Torrent search abstraction.
//!
//! This module provides a `Searcher` trait for searching torrents across
//! various backends (Jackett, Prowlarr, etc.).

mod dedup;
mod file_enricher;
mod jackett;
mod torrent_parser;
mod types;

// Rate limiter kept for potential future use
#[allow(dead_code)]
mod rate_limiter;

pub use dedup::deduplicate_results;
pub use file_enricher::{EnrichmentStats, FileEnricher, FileEnricherConfig};
pub use jackett::JackettSearcher;
pub use torrent_parser::{parse_torrent_files, parse_torrent_info_hash, TorrentParseError};
pub use types::*;
