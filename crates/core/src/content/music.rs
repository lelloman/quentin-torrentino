//! Music content handling (Album, Track).
//!
//! Currently delegates to generic implementation.
//! Phase 5c will add music-specific logic:
//! - Query patterns: "{artist} {album}", "{artist} FLAC", etc.
//! - Scoring: track count, audio format, red flags
//! - Post-processing: cover art fetching

use std::path::Path;

use crate::searcher::{TorrentCandidate, TorrentFile};
use crate::textbrain::{FileMapping, MatchResult, QueryBuildResult, TextBrainConfig, TextBrainError};
use crate::ticket::{QueryContext, Ticket};

use super::types::{ContentError, PostProcessResult};
use super::generic;

/// Build queries for music content.
///
/// TODO (Phase 5c): Implement music-specific query patterns.
pub async fn build_queries(
    context: &QueryContext,
    config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    // Delegate to generic for now
    generic::build_queries(context, config).await
}

/// Score candidates for music content.
///
/// TODO (Phase 5c): Implement music-specific scoring.
pub async fn score_candidates(
    context: &QueryContext,
    candidates: &[TorrentCandidate],
    config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    // Delegate to generic for now
    generic::score_candidates(context, candidates, config).await
}

/// Map files for music content.
///
/// TODO (Phase 5c): Improve music-specific file mapping.
pub fn map_files(context: &QueryContext, files: &[TorrentFile]) -> Vec<FileMapping> {
    // Delegate to generic for now
    generic::map_files(context, files)
}

/// Post-process music content.
///
/// TODO (Phase 5c): Implement cover art fetching:
/// - Check for existing cover (folder.jpg, cover.*)
/// - Fetch from MusicBrainz Cover Art Archive
/// - Fetch from Discogs (if configured)
pub async fn post_process(
    ticket: &Ticket,
    download_path: &Path,
) -> Result<PostProcessResult, ContentError> {
    // Delegate to generic for now (no-op)
    generic::post_process(ticket, download_path).await
}
