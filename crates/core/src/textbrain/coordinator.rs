//! TextBrain coordinator - central orchestrator for query building, search, and matching.

use std::sync::Arc;
use std::time::Instant;

use crate::audit::{AuditEvent, AuditHandle};
use crate::searcher::{SearchQuery, Searcher, TorrentCandidate};
use crate::ticket::QueryContext;
use crate::textbrain::{
    config::{TextBrainConfig, TextBrainMode},
    llm::LlmUsage,
    traits::{CandidateMatcher, QueryBuilder, TextBrainError},
    types::{AcquisitionResult, MatchResult, QueryBuildResult, ScoredCandidate},
};

/// Context for audit event emission during acquisition.
pub struct AcquisitionAuditContext {
    /// The ticket ID being processed
    pub ticket_id: String,
    /// The audit handle for emitting events
    pub audit: AuditHandle,
}

/// TextBrain - the central intelligence for torrent acquisition.
///
/// Coordinates query building, searching, and candidate scoring
/// using a configurable mix of heuristic ("dumb") and LLM-powered methods.
pub struct TextBrain {
    config: TextBrainConfig,
    dumb_query_builder: Option<Arc<dyn QueryBuilder>>,
    llm_query_builder: Option<Arc<dyn QueryBuilder>>,
    dumb_matcher: Option<Arc<dyn CandidateMatcher>>,
    llm_matcher: Option<Arc<dyn CandidateMatcher>>,
}

impl TextBrain {
    /// Create a new TextBrain with the given configuration.
    ///
    /// Note: Query builders and matchers must be set separately via `with_*` methods.
    pub fn new(config: TextBrainConfig) -> Self {
        Self {
            config,
            dumb_query_builder: None,
            llm_query_builder: None,
            dumb_matcher: None,
            llm_matcher: None,
        }
    }

    /// Set the dumb (heuristic) query builder.
    pub fn with_dumb_query_builder(mut self, builder: Arc<dyn QueryBuilder>) -> Self {
        self.dumb_query_builder = Some(builder);
        self
    }

    /// Set the LLM-powered query builder.
    pub fn with_llm_query_builder(mut self, builder: Arc<dyn QueryBuilder>) -> Self {
        self.llm_query_builder = Some(builder);
        self
    }

    /// Set the dumb (heuristic) matcher.
    pub fn with_dumb_matcher(mut self, matcher: Arc<dyn CandidateMatcher>) -> Self {
        self.dumb_matcher = Some(matcher);
        self
    }

    /// Set the LLM-powered matcher.
    pub fn with_llm_matcher(mut self, matcher: Arc<dyn CandidateMatcher>) -> Self {
        self.llm_matcher = Some(matcher);
        self
    }

    /// Get the configuration.
    pub fn config(&self) -> &TextBrainConfig {
        &self.config
    }

    /// Build search queries from the ticket context.
    ///
    /// Uses the configured mode to determine which builders to use.
    pub async fn build_queries(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
        match self.config.mode {
            TextBrainMode::DumbOnly => self.build_queries_dumb(context).await,
            TextBrainMode::DumbFirst => self.build_queries_dumb_first(context).await,
            TextBrainMode::LlmFirst => self.build_queries_llm_first(context).await,
            TextBrainMode::LlmOnly => self.build_queries_llm(context).await,
        }
    }

    /// Score candidates against the ticket context.
    ///
    /// Uses the configured mode to determine which matchers to use.
    pub async fn score_candidates(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        if candidates.is_empty() {
            return Ok(MatchResult {
                candidates: vec![],
                method: "none".to_string(),
                llm_usage: None,
            });
        }

        match self.config.mode {
            TextBrainMode::DumbOnly => self.score_dumb(context, candidates).await,
            TextBrainMode::DumbFirst => self.score_dumb_first(context, candidates).await,
            TextBrainMode::LlmFirst => self.score_llm_first(context, candidates).await,
            TextBrainMode::LlmOnly => self.score_llm(context, candidates).await,
        }
    }

    /// Full acquisition flow: query building -> search -> scoring.
    ///
    /// Iterates through queries until a match is found or all queries are exhausted.
    /// Returns the best candidate if score >= auto_approve_threshold.
    pub async fn acquire(
        &self,
        context: &QueryContext,
        searcher: &dyn Searcher,
    ) -> Result<AcquisitionResult, TextBrainError> {
        let start = Instant::now();
        let mut total_llm_usage = LlmUsage::default();
        let mut all_candidates: Vec<ScoredCandidate> = Vec::new();
        let mut queries_tried: Vec<String> = Vec::new();
        let mut candidates_evaluated: u32 = 0;

        // Step 1: Build queries
        let query_result = self.build_queries(context).await?;
        if let Some(usage) = &query_result.llm_usage {
            total_llm_usage.input_tokens += usage.input_tokens;
            total_llm_usage.output_tokens += usage.output_tokens;
        }
        let query_method = query_result.method.clone();

        if query_result.queries.is_empty() {
            return Err(TextBrainError::NoQueriesGenerated);
        }

        // Step 2: Iterate through queries
        let max_queries = self.config.max_queries as usize;
        for query_str in query_result.queries.iter().take(max_queries) {
            queries_tried.push(query_str.clone());

            // Execute search
            let search_query = SearchQuery {
                query: query_str.clone(),
                indexers: None,
                categories: None, // Could be derived from context.tags
                limit: Some(50),
            };

            let search_result = searcher
                .search(&search_query)
                .await
                .map_err(|e| TextBrainError::SearchFailed(e.to_string()))?;

            if search_result.candidates.is_empty() {
                continue; // Try next query
            }

            candidates_evaluated += search_result.candidates.len() as u32;

            // Score candidates
            let match_result = self.score_candidates(context, &search_result.candidates).await?;
            if let Some(usage) = &match_result.llm_usage {
                total_llm_usage.input_tokens += usage.input_tokens;
                total_llm_usage.output_tokens += usage.output_tokens;
            }
            let score_method = match_result.method.clone();

            // Merge scored candidates (avoid duplicates by info_hash)
            for scored in match_result.candidates {
                if !all_candidates
                    .iter()
                    .any(|c| c.candidate.info_hash == scored.candidate.info_hash)
                {
                    all_candidates.push(scored);
                }
            }

            // Check if we have a good enough match
            if let Some(best) = all_candidates.first() {
                if best.score >= self.config.auto_approve_threshold {
                    // Found a good match, we're done
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(AcquisitionResult {
                        best_candidate: Some(best.clone()),
                        all_candidates,
                        queries_tried,
                        candidates_evaluated,
                        query_method,
                        score_method,
                        auto_approved: true,
                        llm_usage: if total_llm_usage.input_tokens > 0 {
                            Some(total_llm_usage)
                        } else {
                            None
                        },
                        duration_ms,
                    });
                }
            }
        }

        // Re-sort all candidates by score
        all_candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let duration_ms = start.elapsed().as_millis() as u64;
        let best_candidate = all_candidates.first().cloned();
        let auto_approved = best_candidate
            .as_ref()
            .map(|c| c.score >= self.config.auto_approve_threshold)
            .unwrap_or(false);

        Ok(AcquisitionResult {
            best_candidate,
            all_candidates,
            queries_tried,
            candidates_evaluated,
            query_method,
            score_method: "dumb".to_string(), // Will be updated properly when we track this
            auto_approved,
            llm_usage: if total_llm_usage.input_tokens > 0 {
                Some(total_llm_usage)
            } else {
                None
            },
            duration_ms,
        })
    }

    /// Full acquisition flow with audit event emission.
    ///
    /// Same as `acquire`, but emits detailed audit events for observability.
    pub async fn acquire_with_audit(
        &self,
        context: &QueryContext,
        searcher: &dyn Searcher,
        audit_ctx: &AcquisitionAuditContext,
    ) -> Result<AcquisitionResult, TextBrainError> {
        let start = Instant::now();
        let mut total_llm_usage = LlmUsage::default();
        let mut all_candidates: Vec<ScoredCandidate> = Vec::new();
        let mut queries_tried: Vec<String> = Vec::new();
        let mut candidates_evaluated: u32 = 0;
        let mut last_score_method = "none".to_string();

        // Emit acquisition started event
        audit_ctx.audit.emit(AuditEvent::AcquisitionStarted {
            ticket_id: audit_ctx.ticket_id.clone(),
            mode: format!("{:?}", self.config.mode),
            description: context.description.clone(),
        }).await;

        // Step 1: Build queries
        let method_name = format!("{:?}", self.config.mode);
        audit_ctx.audit.emit(AuditEvent::QueryBuildingStarted {
            ticket_id: audit_ctx.ticket_id.clone(),
            method: method_name.clone(),
        }).await;

        let query_start = Instant::now();
        let query_result = self.build_queries(context).await;
        let query_duration = query_start.elapsed().as_millis() as u64;

        let query_result = match query_result {
            Ok(result) => {
                audit_ctx.audit.emit(AuditEvent::QueryBuildingCompleted {
                    ticket_id: audit_ctx.ticket_id.clone(),
                    queries: result.queries.clone(),
                    method: result.method.clone(),
                    duration_ms: query_duration,
                }).await;
                result
            }
            Err(e) => {
                // Emit acquisition completed with failure
                audit_ctx.audit.emit(AuditEvent::AcquisitionCompleted {
                    ticket_id: audit_ctx.ticket_id.clone(),
                    success: false,
                    queries_tried: 0,
                    candidates_evaluated: 0,
                    best_score: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    outcome: "failed".to_string(),
                }).await;
                return Err(e);
            }
        };

        if let Some(usage) = &query_result.llm_usage {
            total_llm_usage.input_tokens += usage.input_tokens;
            total_llm_usage.output_tokens += usage.output_tokens;
        }
        let query_method = query_result.method.clone();

        if query_result.queries.is_empty() {
            audit_ctx.audit.emit(AuditEvent::AcquisitionCompleted {
                ticket_id: audit_ctx.ticket_id.clone(),
                success: false,
                queries_tried: 0,
                candidates_evaluated: 0,
                best_score: None,
                duration_ms: start.elapsed().as_millis() as u64,
                outcome: "no_queries".to_string(),
            }).await;
            return Err(TextBrainError::NoQueriesGenerated);
        }

        // Step 2: Iterate through queries
        let max_queries = self.config.max_queries as usize;
        let total_queries = query_result.queries.len().min(max_queries) as u32;

        for (idx, query_str) in query_result.queries.iter().take(max_queries).enumerate() {
            queries_tried.push(query_str.clone());

            // Emit search started event
            audit_ctx.audit.emit(AuditEvent::SearchStarted {
                ticket_id: audit_ctx.ticket_id.clone(),
                query: query_str.clone(),
                query_index: (idx + 1) as u32,
                total_queries,
            }).await;

            // Execute search
            let search_query = SearchQuery {
                query: query_str.clone(),
                indexers: None,
                categories: None,
                limit: Some(50),
            };

            let search_start = Instant::now();
            let search_result = searcher
                .search(&search_query)
                .await
                .map_err(|e| TextBrainError::SearchFailed(e.to_string()))?;
            let search_duration = search_start.elapsed().as_millis() as u64;

            // Emit search completed event
            audit_ctx.audit.emit(AuditEvent::SearchCompleted {
                ticket_id: audit_ctx.ticket_id.clone(),
                query: query_str.clone(),
                candidates_found: search_result.candidates.len() as u32,
                duration_ms: search_duration,
            }).await;

            if search_result.candidates.is_empty() {
                continue; // Try next query
            }

            candidates_evaluated += search_result.candidates.len() as u32;

            // Emit scoring started event
            audit_ctx.audit.emit(AuditEvent::ScoringStarted {
                ticket_id: audit_ctx.ticket_id.clone(),
                candidates_count: search_result.candidates.len() as u32,
                method: format!("{:?}", self.config.mode),
            }).await;

            // Score candidates
            let score_start = Instant::now();
            let match_result = self.score_candidates(context, &search_result.candidates).await?;
            let score_duration = score_start.elapsed().as_millis() as u64;

            // Emit scoring completed event
            let top_candidate = match_result.candidates.first();
            audit_ctx.audit.emit(AuditEvent::ScoringCompleted {
                ticket_id: audit_ctx.ticket_id.clone(),
                candidates_count: match_result.candidates.len() as u32,
                top_candidate_hash: top_candidate.map(|c| c.candidate.info_hash.clone()),
                top_candidate_score: top_candidate.map(|c| c.score),
                method: match_result.method.clone(),
                duration_ms: score_duration,
            }).await;

            if let Some(usage) = &match_result.llm_usage {
                total_llm_usage.input_tokens += usage.input_tokens;
                total_llm_usage.output_tokens += usage.output_tokens;
            }
            last_score_method = match_result.method.clone();

            // Merge scored candidates (avoid duplicates by info_hash)
            for scored in match_result.candidates {
                if !all_candidates
                    .iter()
                    .any(|c| c.candidate.info_hash == scored.candidate.info_hash)
                {
                    all_candidates.push(scored);
                }
            }

            // Check if we have a good enough match
            if let Some(best) = all_candidates.first() {
                if best.score >= self.config.auto_approve_threshold {
                    // Found a good match, we're done
                    let duration_ms = start.elapsed().as_millis() as u64;

                    audit_ctx.audit.emit(AuditEvent::AcquisitionCompleted {
                        ticket_id: audit_ctx.ticket_id.clone(),
                        success: true,
                        queries_tried: queries_tried.len() as u32,
                        candidates_evaluated,
                        best_score: Some(best.score),
                        duration_ms,
                        outcome: "auto_approved".to_string(),
                    }).await;

                    return Ok(AcquisitionResult {
                        best_candidate: Some(best.clone()),
                        all_candidates,
                        queries_tried,
                        candidates_evaluated,
                        query_method,
                        score_method: last_score_method,
                        auto_approved: true,
                        llm_usage: if total_llm_usage.input_tokens > 0 {
                            Some(total_llm_usage)
                        } else {
                            None
                        },
                        duration_ms,
                    });
                }
            }
        }

        // Re-sort all candidates by score
        all_candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let duration_ms = start.elapsed().as_millis() as u64;
        let best_candidate = all_candidates.first().cloned();
        let auto_approved = best_candidate
            .as_ref()
            .map(|c| c.score >= self.config.auto_approve_threshold)
            .unwrap_or(false);

        // Determine outcome
        let outcome = if best_candidate.is_none() {
            "no_candidates"
        } else if auto_approved {
            "auto_approved"
        } else {
            "needs_approval"
        };

        audit_ctx.audit.emit(AuditEvent::AcquisitionCompleted {
            ticket_id: audit_ctx.ticket_id.clone(),
            success: best_candidate.is_some(),
            queries_tried: queries_tried.len() as u32,
            candidates_evaluated,
            best_score: best_candidate.as_ref().map(|c| c.score),
            duration_ms,
            outcome: outcome.to_string(),
        }).await;

        Ok(AcquisitionResult {
            best_candidate,
            all_candidates,
            queries_tried,
            candidates_evaluated,
            query_method,
            score_method: last_score_method,
            auto_approved,
            llm_usage: if total_llm_usage.input_tokens > 0 {
                Some(total_llm_usage)
            } else {
                None
            },
            duration_ms,
        })
    }

    // ========================================================================
    // Query Building Strategies
    // ========================================================================

    async fn build_queries_dumb(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
        let builder = self
            .dumb_query_builder
            .as_ref()
            .ok_or_else(|| TextBrainError::ConfigError("Dumb query builder not configured".to_string()))?;

        builder.build_queries(context).await
    }

    async fn build_queries_llm(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
        let builder = self
            .llm_query_builder
            .as_ref()
            .ok_or(TextBrainError::LlmNotConfigured)?;

        builder.build_queries(context).await
    }

    async fn build_queries_dumb_first(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
        // Try dumb first
        if let Some(builder) = &self.dumb_query_builder {
            let result = builder.build_queries(context).await?;

            // If confidence is high enough, use it
            if result.confidence >= self.config.confidence_threshold {
                return Ok(result);
            }

            // Try LLM if available
            if let Some(llm_builder) = &self.llm_query_builder {
                match llm_builder.build_queries(context).await {
                    Ok(llm_result) => {
                        // Merge: LLM queries first, then dumb queries
                        let mut merged_queries = llm_result.queries;
                        for q in result.queries {
                            if !merged_queries.contains(&q) {
                                merged_queries.push(q);
                            }
                        }

                        let total_usage = merge_llm_usage(result.llm_usage, llm_result.llm_usage);

                        return Ok(QueryBuildResult {
                            queries: merged_queries,
                            method: "dumb_then_llm".to_string(),
                            confidence: llm_result.confidence.max(result.confidence),
                            llm_usage: total_usage,
                        });
                    }
                    Err(_) => {
                        // LLM failed, return dumb result
                        return Ok(result);
                    }
                }
            }

            return Ok(result);
        }

        // No dumb builder, try LLM
        self.build_queries_llm(context).await
    }

    async fn build_queries_llm_first(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
        // Try LLM first
        if let Some(builder) = &self.llm_query_builder {
            match builder.build_queries(context).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Log error and fall back to dumb
                    tracing::warn!("LLM query building failed, falling back to dumb: {}", e);
                }
            }
        }

        // Fall back to dumb
        self.build_queries_dumb(context).await
    }

    // ========================================================================
    // Scoring Strategies
    // ========================================================================

    async fn score_dumb(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        let matcher = self
            .dumb_matcher
            .as_ref()
            .ok_or_else(|| TextBrainError::ConfigError("Dumb matcher not configured".to_string()))?;

        matcher.score_candidates(context, candidates).await
    }

    async fn score_llm(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        let matcher = self
            .llm_matcher
            .as_ref()
            .ok_or(TextBrainError::LlmNotConfigured)?;

        matcher.score_candidates(context, candidates).await
    }

    async fn score_dumb_first(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        // Try dumb first
        if let Some(matcher) = &self.dumb_matcher {
            let result = matcher.score_candidates(context, candidates).await?;

            // If top score is high enough, use it
            if let Some(best) = result.candidates.first() {
                if best.score >= self.config.confidence_threshold {
                    return Ok(result);
                }
            }

            // Try LLM if available
            if let Some(llm_matcher) = &self.llm_matcher {
                match llm_matcher.score_candidates(context, candidates).await {
                    Ok(llm_result) => {
                        let total_usage = merge_llm_usage(result.llm_usage, llm_result.llm_usage);
                        return Ok(MatchResult {
                            candidates: llm_result.candidates,
                            method: "dumb_then_llm".to_string(),
                            llm_usage: total_usage,
                        });
                    }
                    Err(_) => {
                        // LLM failed, return dumb result
                        return Ok(result);
                    }
                }
            }

            return Ok(result);
        }

        // No dumb matcher, try LLM
        self.score_llm(context, candidates).await
    }

    async fn score_llm_first(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        // Try LLM first
        if let Some(matcher) = &self.llm_matcher {
            match matcher.score_candidates(context, candidates).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("LLM scoring failed, falling back to dumb: {}", e);
                }
            }
        }

        // Fall back to dumb
        self.score_dumb(context, candidates).await
    }
}

/// Merge two optional LlmUsage values.
fn merge_llm_usage(a: Option<LlmUsage>, b: Option<LlmUsage>) -> Option<LlmUsage> {
    match (a, b) {
        (Some(a), Some(b)) => Some(LlmUsage {
            input_tokens: a.input_tokens + b.input_tokens,
            output_tokens: a.output_tokens + b.output_tokens,
        }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::{SearchError, SearchResult, TorrentSource, IndexerStatus};
    use crate::textbrain::types::ScoredCandidate;

    // Mock query builder for testing
    struct MockQueryBuilder {
        queries: Vec<String>,
        confidence: f32,
    }

    #[async_trait::async_trait]
    impl QueryBuilder for MockQueryBuilder {
        fn name(&self) -> &str {
            "mock"
        }

        async fn build_queries(&self, _context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
            Ok(QueryBuildResult {
                queries: self.queries.clone(),
                method: "mock".to_string(),
                confidence: self.confidence,
                llm_usage: None,
            })
        }
    }

    // Mock matcher for testing
    struct MockMatcher {
        score: f32,
    }

    #[async_trait::async_trait]
    impl CandidateMatcher for MockMatcher {
        fn name(&self) -> &str {
            "mock"
        }

        async fn score_candidates(
            &self,
            _context: &QueryContext,
            candidates: &[TorrentCandidate],
        ) -> Result<MatchResult, TextBrainError> {
            let scored: Vec<ScoredCandidate> = candidates
                .iter()
                .map(|c| ScoredCandidate {
                    candidate: c.clone(),
                    score: self.score,
                    reasoning: "Mock score".to_string(),
                    file_mappings: vec![],
                })
                .collect();

            Ok(MatchResult {
                candidates: scored,
                method: "mock".to_string(),
                llm_usage: None,
            })
        }
    }

    // Mock searcher for testing
    struct MockSearcher {
        candidates: Vec<TorrentCandidate>,
    }

    #[async_trait::async_trait]
    impl Searcher for MockSearcher {
        fn name(&self) -> &str {
            "mock"
        }

        async fn search(&self, _query: &SearchQuery) -> Result<SearchResult, SearchError> {
            Ok(SearchResult {
                query: SearchQuery {
                    query: "test".to_string(),
                    indexers: None,
                    categories: None,
                    limit: None,
                },
                candidates: self.candidates.clone(),
                duration_ms: 100,
                indexer_errors: std::collections::HashMap::new(),
            })
        }

        async fn indexer_status(&self) -> Vec<IndexerStatus> {
            vec![]
        }
    }

    fn make_candidate(title: &str) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: format!("hash_{}", title),
            size_bytes: 1024,
            seeders: 10,
            leechers: 5,
            category: None,
            publish_date: None,
            files: None,
            sources: vec![TorrentSource {
                indexer: "test".to_string(),
                magnet_uri: Some("magnet:?xt=urn:btih:abc".to_string()),
                torrent_url: None,
                seeders: 10,
                leechers: 5,
                details_url: None,
            }],
            from_cache: false,
        }
    }

    #[tokio::test]
    async fn test_build_queries_dumb_only() {
        let config = TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            ..Default::default()
        };

        let brain = TextBrain::new(config).with_dumb_query_builder(Arc::new(MockQueryBuilder {
            queries: vec!["test query".to_string()],
            confidence: 0.8,
        }));

        let context = QueryContext::new(vec!["music".to_string()], "Test description");
        let result = brain.build_queries(&context).await.unwrap();

        assert_eq!(result.queries, vec!["test query"]);
        assert_eq!(result.method, "mock");
    }

    #[tokio::test]
    async fn test_score_candidates_empty() {
        let config = TextBrainConfig::default();
        let brain = TextBrain::new(config);

        let context = QueryContext::new(vec![], "Test");
        let result = brain.score_candidates(&context, &[]).await.unwrap();

        assert!(result.candidates.is_empty());
        assert_eq!(result.method, "none");
    }

    #[tokio::test]
    async fn test_acquire_auto_approve() {
        let config = TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            auto_approve_threshold: 0.8,
            ..Default::default()
        };

        let brain = TextBrain::new(config)
            .with_dumb_query_builder(Arc::new(MockQueryBuilder {
                queries: vec!["test query".to_string()],
                confidence: 0.9,
            }))
            .with_dumb_matcher(Arc::new(MockMatcher { score: 0.95 }));

        let searcher = MockSearcher {
            candidates: vec![make_candidate("Test Torrent")],
        };

        let context = QueryContext::new(vec!["music".to_string()], "Test description");
        let result = brain.acquire(&context, &searcher).await.unwrap();

        assert!(result.auto_approved);
        assert!(result.best_candidate.is_some());
        assert_eq!(result.best_candidate.unwrap().score, 0.95);
    }

    #[tokio::test]
    async fn test_acquire_no_auto_approve() {
        let config = TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            auto_approve_threshold: 0.9,
            ..Default::default()
        };

        let brain = TextBrain::new(config)
            .with_dumb_query_builder(Arc::new(MockQueryBuilder {
                queries: vec!["test query".to_string()],
                confidence: 0.9,
            }))
            .with_dumb_matcher(Arc::new(MockMatcher { score: 0.7 })); // Below threshold

        let searcher = MockSearcher {
            candidates: vec![make_candidate("Test Torrent")],
        };

        let context = QueryContext::new(vec!["music".to_string()], "Test description");
        let result = brain.acquire(&context, &searcher).await.unwrap();

        assert!(!result.auto_approved);
        assert!(result.best_candidate.is_some());
    }
}
