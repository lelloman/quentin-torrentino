//! Generic content handling - fallback for unknown content types.
//!
//! Wraps the existing DumbQueryBuilder, DumbMatcher, and DumbFileMapper.

use std::path::Path;

use crate::searcher::{TorrentCandidate, TorrentFile};
use crate::textbrain::{
    CandidateMatcher, DumbFileMapper, DumbMatcher, DumbQueryBuilder, FileMapping, MatchResult,
    QueryBuilder, QueryBuildResult, TextBrainConfig, TextBrainError,
};
use crate::ticket::{QueryContext, Ticket};

use super::types::{ContentError, PostProcessResult};

/// Build queries using the generic (dumb) query builder.
pub async fn build_queries(
    context: &QueryContext,
    _config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    let builder = DumbQueryBuilder::new();
    builder.build_queries(context).await
}

/// Score candidates using the generic (dumb) matcher.
pub async fn score_candidates(
    context: &QueryContext,
    candidates: &[TorrentCandidate],
    _config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    let matcher = DumbMatcher::new();
    matcher.score_candidates(context, candidates).await
}

/// Map files using the generic (dumb) file mapper.
pub fn map_files(context: &QueryContext, files: &[TorrentFile]) -> Vec<FileMapping> {
    let mapper = DumbFileMapper::new();

    match &context.expected {
        Some(expected) => mapper.map_files(files, expected),
        None => vec![], // No expected content, can't map files
    }
}

/// Generic post-processing - does nothing.
///
/// The generic handler doesn't fetch any external assets.
/// Content-specific handlers (music, video) will override this.
pub async fn post_process(
    _ticket: &Ticket,
    _download_path: &Path,
) -> Result<PostProcessResult, ContentError> {
    Ok(PostProcessResult::empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textbrain::TextBrainMode;

    #[tokio::test]
    async fn test_build_queries_generic() {
        let context = QueryContext {
            tags: vec![],
            description: "Test Album by Test Artist".to_string(),
            expected: None,
        };
        let config = TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            ..Default::default()
        };

        let result = build_queries(&context, &config).await.unwrap();
        assert!(!result.queries.is_empty());
        assert_eq!(result.method, "dumb");
    }

    #[tokio::test]
    async fn test_post_process_returns_empty() {
        let now = chrono::Utc::now();
        let ticket = Ticket {
            id: "test-123".to_string(),
            query_context: QueryContext {
                tags: vec![],
                description: "test".to_string(),
                expected: None,
            },
            dest_path: "/tmp/test".to_string(),
            priority: 0,
            state: crate::ticket::TicketState::Pending,
            created_at: now,
            updated_at: now,
            created_by: "test".to_string(),
            output_constraints: None,
        };

        let result = post_process(&ticket, Path::new("/tmp")).await.unwrap();
        assert!(result.cover_art_path.is_none());
        assert!(result.subtitle_paths.is_empty());
    }
}
