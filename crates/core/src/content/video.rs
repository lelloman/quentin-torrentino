//! Video content handling (Movie, TvEpisode).
//!
//! Currently delegates to generic implementation.
//! Phase 5d will add video-specific logic:
//! - Query patterns: "{title} {year}", "S01E01", etc.
//! - Scoring: resolution, codec, source quality
//! - Post-processing: subtitle fetching

use std::path::Path;

use crate::searcher::{TorrentCandidate, TorrentFile};
use crate::textbrain::{FileMapping, MatchResult, QueryBuildResult, TextBrainConfig, TextBrainError};
use crate::ticket::{QueryContext, Ticket};

use super::types::{ContentError, PostProcessResult};
use super::generic;

/// Build queries for video content.
///
/// TODO (Phase 5d): Implement video-specific query patterns.
pub async fn build_queries(
    context: &QueryContext,
    config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    // Delegate to generic for now
    generic::build_queries(context, config).await
}

/// Score candidates for video content.
///
/// TODO (Phase 5d): Implement video-specific scoring.
pub async fn score_candidates(
    context: &QueryContext,
    candidates: &[TorrentCandidate],
    config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    // Delegate to generic for now
    generic::score_candidates(context, candidates, config).await
}

/// Map files for video content.
///
/// TODO (Phase 5d): Improve video-specific file mapping.
pub fn map_files(context: &QueryContext, files: &[TorrentFile]) -> Vec<FileMapping> {
    // Delegate to generic for now
    generic::map_files(context, files)
}

/// Post-process video content.
///
/// TODO (Phase 5d): Implement subtitle fetching:
/// - Check for existing subtitles (.srt, .ass, .sub)
/// - Fetch from OpenSubtitles (if configured)
/// - Extract embedded subtitles from MKV
pub async fn post_process(
    ticket: &Ticket,
    download_path: &Path,
) -> Result<PostProcessResult, ContentError> {
    // Delegate to generic for now (no-op)
    generic::post_process(ticket, download_path).await
}
