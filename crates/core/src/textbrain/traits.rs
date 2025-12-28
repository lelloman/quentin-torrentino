//! Traits for TextBrain components.

use async_trait::async_trait;
use thiserror::Error;

use crate::searcher::TorrentCandidate;
use crate::ticket::QueryContext;
use crate::textbrain::types::{MatchResult, QueryBuildResult};

/// Errors that can occur during TextBrain operations.
#[derive(Debug, Error)]
pub enum TextBrainError {
    #[error("Query building failed: {0}")]
    QueryBuildFailed(String),

    #[error("No queries generated")]
    NoQueriesGenerated,

    #[error("Matching failed: {0}")]
    MatchingFailed(String),

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("LLM not configured but required")]
    LlmNotConfigured,

    #[error("Search failed: {0}")]
    SearchFailed(String),

    #[error("All strategies exhausted, no suitable match found")]
    Exhausted,

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Trait for building search queries from ticket context.
///
/// Implementations can use templates, heuristics, or LLM to generate queries.
#[async_trait]
pub trait QueryBuilder: Send + Sync {
    /// Name of this query builder for logging/audit.
    fn name(&self) -> &str;

    /// Generate search queries from the ticket context.
    ///
    /// Returns multiple queries in priority order.
    /// The first query should be the most likely to find good results.
    async fn build_queries(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError>;
}

/// Trait for scoring torrent candidates against ticket requirements.
///
/// Implementations can use fuzzy matching, heuristics, or LLM for scoring.
#[async_trait]
pub trait CandidateMatcher: Send + Sync {
    /// Name of this matcher for logging/audit.
    fn name(&self) -> &str;

    /// Score candidates against the ticket context.
    ///
    /// Returns candidates sorted by score (highest first).
    /// Score is 0.0-1.0 where 1.0 is a perfect match.
    async fn score_candidates(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that errors display correctly
    #[test]
    fn test_error_display() {
        let err = TextBrainError::QueryBuildFailed("test error".to_string());
        assert_eq!(err.to_string(), "Query building failed: test error");

        let err = TextBrainError::Exhausted;
        assert_eq!(
            err.to_string(),
            "All strategies exhausted, no suitable match found"
        );
    }
}
