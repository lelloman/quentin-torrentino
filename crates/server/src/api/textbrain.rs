//! TextBrain API handlers for experimentation.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use torrentino_core::{
    AnthropicClient, AuditEvent, CandidateMatcher, CompletionRequest, DumbMatcher, DumbQueryBuilder,
    ExpectedContent, ExpectedTrack, FileEnricher, LlmClient, LlmUsage, QueryBuilder, QueryContext,
    SearchQuery, TextBrain, TextBrainConfig, TextBrainMode,
};

use crate::state::AppState;

// ============================================================================
// Request/Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CompleteRequest {
    /// The prompt to send
    pub prompt: String,
    /// Optional system prompt
    pub system: Option<String>,
    /// API key (for experimentation - will move to config later)
    #[serde(default)]
    pub api_key: String,
    /// Model to use (default: claude-3-haiku-20240307)
    #[serde(default = "default_model")]
    pub model: String,
    /// Provider (default: anthropic)
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Max tokens (default: 1024)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Temperature (default: 0.0)
    #[serde(default)]
    pub temperature: f32,
    /// Custom API base URL (e.g., http://localhost:5000 for claude proxy)
    pub api_base: Option<String>,
}

fn default_model() -> String {
    "claude-3-haiku-20240307".to_string()
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_max_tokens() -> u32 {
    1024
}

#[derive(Debug, Serialize)]
pub struct CompleteResponse {
    pub text: String,
    pub usage: LlmUsage,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/v1/textbrain/complete
///
/// Send a prompt to the LLM and get a response.
/// This is for experimentation - API key is passed in request.
pub async fn complete(
    Json(body): Json<CompleteRequest>,
) -> Result<Json<CompleteResponse>, impl IntoResponse> {
    // For now, only support Anthropic (or claude proxy with same API format)
    if body.provider != "anthropic" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Unsupported provider: {}. Only 'anthropic' is supported.", body.provider),
            }),
        ));
    }

    let mut client = AnthropicClient::new(&body.api_key, &body.model);
    if let Some(api_base) = &body.api_base {
        client = client.with_api_base(api_base);
    }

    let mut request = CompletionRequest::new(&body.prompt)
        .with_max_tokens(body.max_tokens)
        .with_temperature(body.temperature);

    if let Some(system) = &body.system {
        request = request.with_system(system);
    }

    match client.complete(request).await {
        Ok(response) => Ok(Json(CompleteResponse {
            text: response.text,
            usage: response.usage,
            model: response.model,
            provider: body.provider,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

// ============================================================================
// Process Ticket - Full TextBrain Flow
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ProcessTicketRequest {
    /// API base URL for LLM (e.g., http://localhost:5000 for claude proxy)
    pub api_base: Option<String>,
    /// API key (optional if using proxy)
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct ScoredCandidate {
    pub title: String,
    pub info_hash: String,
    pub size_bytes: u64,
    pub seeders: u32,
    pub score: u32,
    pub reasoning: String,
}

#[derive(Debug, Serialize)]
pub struct ProcessTicketResponse {
    pub ticket_id: String,
    pub description: String,
    pub queries_generated: Vec<String>,
    pub search_results_count: usize,
    pub scored_candidates: Vec<ScoredCandidate>,
    pub llm_usage: LlmUsage,
}

/// POST /api/v1/textbrain/process/{ticket_id}
///
/// Run the full TextBrain flow for a ticket:
/// 1. Fetch ticket
/// 2. Generate search queries via LLM
/// 3. Execute search
/// 4. Score candidates via LLM
/// 5. Return scored candidates
pub async fn process_ticket(
    State(state): State<Arc<AppState>>,
    Path(ticket_id): Path<String>,
    Json(body): Json<ProcessTicketRequest>,
) -> Result<Json<ProcessTicketResponse>, impl IntoResponse> {
    let start = std::time::Instant::now();
    let mut total_usage = LlmUsage::default();

    // 1. Fetch the ticket
    let ticket = match state.ticket_store().get(&ticket_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Ticket not found: {}", ticket_id),
                }),
            ))
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ))
        }
    };

    // 2. Generate search queries via LLM
    let client = if let Some(api_base) = &body.api_base {
        AnthropicClient::new(&body.api_key, "claude-cli").with_api_base(api_base)
    } else {
        AnthropicClient::new(&body.api_key, "claude-3-haiku-20240307")
    };

    let query_prompt = format!(
        "Generate search queries to find this content on torrent sites.\n\
         Description: {}\n\
         Tags: {}\n\n\
         Return ONLY a JSON array of 3-5 search query strings, no explanation.",
        ticket.query_context.description,
        ticket.query_context.tags.join(", ")
    );

    let query_request = CompletionRequest::new(&query_prompt)
        .with_system("You generate torrent search queries. Return only valid JSON arrays.")
        .with_max_tokens(256);

    let queries: Vec<String> = match client.complete(query_request).await {
        Ok(response) => {
            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;

            // Parse JSON from response (handle markdown code blocks)
            let text = response.text.trim();
            let json_text = if text.starts_with("```") {
                text.lines()
                    .skip(1)
                    .take_while(|l| !l.starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                text.to_string()
            };

            serde_json::from_str(&json_text).unwrap_or_else(|_| {
                vec![ticket.query_context.description.clone()]
            })
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Query generation failed: {}", e),
                }),
            ))
        }
    };

    let query_duration = start.elapsed().as_millis() as u64;

    // Emit audit event for query generation
    state.audit().try_emit(AuditEvent::QueriesGenerated {
        ticket_id: ticket_id.clone(),
        queries: queries.clone(),
        method: "llm".to_string(),
        llm_input_tokens: Some(total_usage.input_tokens),
        llm_output_tokens: Some(total_usage.output_tokens),
        duration_ms: query_duration,
    });

    // 3. Execute search with first query
    let searcher = match state.searcher() {
        Some(s) => s,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Search backend not configured".to_string(),
                }),
            ))
        }
    };

    let search_query = SearchQuery {
        query: queries.first().cloned().unwrap_or_default(),
        indexers: None,
        categories: None,
        limit: Some(20),
    };

    let search_result = match searcher.search(&search_query).await {
        Ok(r) => r,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Search failed: {}", e),
                }),
            ))
        }
    };

    let candidates = search_result.candidates;
    let search_results_count = candidates.len();

    if candidates.is_empty() {
        return Ok(Json(ProcessTicketResponse {
            ticket_id,
            description: ticket.query_context.description,
            queries_generated: queries,
            search_results_count: 0,
            scored_candidates: vec![],
            llm_usage: total_usage,
        }));
    }

    // 4. Score candidates via LLM
    let candidates_text: String = candidates
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, c)| {
            format!(
                "{}. \"{}\" - {}MB, {} seeders",
                i + 1,
                c.title,
                c.size_bytes / 1_000_000,
                c.seeders
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let score_prompt = format!(
        "Score these torrent candidates against the request.\n\n\
         REQUEST:\n{}\nTags: {}\n\n\
         CANDIDATES:\n{}\n\n\
         Return JSON array with objects: {{\"index\": N, \"score\": 0-100, \"reasoning\": \"brief\"}}",
        ticket.query_context.description,
        ticket.query_context.tags.join(", "),
        candidates_text
    );

    let score_request = CompletionRequest::new(&score_prompt)
        .with_system("You score torrent search results. Return only valid JSON.")
        .with_max_tokens(1024);

    let score_start = std::time::Instant::now();

    let scored: Vec<ScoredCandidate> = match client.complete(score_request).await {
        Ok(response) => {
            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;

            // Parse JSON from response
            let text = response.text.trim();
            let json_text = if text.starts_with("```") {
                text.lines()
                    .skip(1)
                    .take_while(|l| !l.starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                text.to_string()
            };

            #[derive(Deserialize)]
            struct ScoreItem {
                index: usize,
                score: u32,
                reasoning: String,
            }

            let scores: Vec<ScoreItem> =
                serde_json::from_str(&json_text).unwrap_or_default();

            let mut result: Vec<ScoredCandidate> = scores
                .into_iter()
                .filter_map(|s| {
                    candidates.get(s.index.saturating_sub(1)).map(|c| ScoredCandidate {
                        title: c.title.clone(),
                        info_hash: c.info_hash.clone(),
                        size_bytes: c.size_bytes,
                        seeders: c.seeders,
                        score: s.score,
                        reasoning: s.reasoning,
                    })
                })
                .collect();

            result.sort_by(|a, b| b.score.cmp(&a.score));
            result
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Scoring failed: {}", e),
                }),
            ))
        }
    };

    let score_duration = score_start.elapsed().as_millis() as u64;

    // Emit audit event for scoring
    let top = scored.first();
    state.audit().try_emit(AuditEvent::CandidatesScored {
        ticket_id: ticket_id.clone(),
        candidates_count: scored.len() as u32,
        top_candidate_hash: top.map(|t| t.info_hash.clone()),
        top_candidate_score: top.map(|t| t.score),
        method: "llm".to_string(),
        llm_input_tokens: Some(total_usage.input_tokens),
        llm_output_tokens: Some(total_usage.output_tokens),
        duration_ms: score_duration,
    });

    Ok(Json(ProcessTicketResponse {
        ticket_id,
        description: ticket.query_context.description,
        queries_generated: queries,
        search_results_count,
        scored_candidates: scored,
        llm_usage: total_usage,
    }))
}

// ============================================================================
// Acquire - Test TextBrain with DumbQueryBuilder/DumbMatcher
// ============================================================================

/// Expected track for album expectations.
#[derive(Debug, Deserialize)]
pub struct ExpectedTrackRequest {
    pub number: u32,
    pub title: String,
    #[serde(default)]
    pub duration_secs: Option<u32>,
}

/// Expected content structure for file validation.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectedContentRequest {
    Album {
        #[serde(default)]
        artist: Option<String>,
        title: String,
        tracks: Vec<ExpectedTrackRequest>,
    },
    Track {
        #[serde(default)]
        artist: Option<String>,
        title: String,
    },
    Movie {
        title: String,
        #[serde(default)]
        year: Option<u32>,
    },
    TvEpisode {
        series: String,
        season: u32,
        episodes: Vec<u32>,
    },
}

impl From<ExpectedContentRequest> for ExpectedContent {
    fn from(req: ExpectedContentRequest) -> Self {
        match req {
            ExpectedContentRequest::Album { artist, title, tracks } => {
                let tracks: Vec<ExpectedTrack> = tracks
                    .into_iter()
                    .map(|t| {
                        let mut track = ExpectedTrack::new(t.number, t.title);
                        if let Some(d) = t.duration_secs {
                            track = track.with_duration(d);
                        }
                        track
                    })
                    .collect();
                if let Some(artist) = artist {
                    ExpectedContent::album_by(artist, title, tracks)
                } else {
                    ExpectedContent::album(title, tracks)
                }
            }
            ExpectedContentRequest::Track { artist, title } => {
                if let Some(artist) = artist {
                    ExpectedContent::track_by(artist, title)
                } else {
                    ExpectedContent::track(title)
                }
            }
            ExpectedContentRequest::Movie { title, year } => {
                if let Some(year) = year {
                    ExpectedContent::movie_year(title, year)
                } else {
                    ExpectedContent::movie(title)
                }
            }
            ExpectedContentRequest::TvEpisode { series, season, episodes } => {
                ExpectedContent::tv_episodes(series, season, episodes)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AcquireRequest {
    /// Freeform description of what to find.
    pub description: String,
    /// Tags for categorization (e.g., ["music", "album", "flac"]).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Expected content structure (optional).
    #[serde(default)]
    pub expected: Option<ExpectedContentRequest>,
    /// Auto-approve threshold (default: 0.85).
    #[serde(default = "default_threshold")]
    pub auto_approve_threshold: f32,
    /// Use cache-only search (no live search).
    #[serde(default)]
    pub cache_only: bool,
}

fn default_threshold() -> f32 {
    0.85
}

#[derive(Debug, Serialize)]
pub struct AcquireCandidateResponse {
    pub title: String,
    pub info_hash: String,
    pub size_bytes: u64,
    pub seeders: u32,
    pub score: f32,
    pub reasoning: String,
}

#[derive(Debug, Serialize)]
pub struct AcquireResponse {
    /// Queries that were tried.
    pub queries_tried: Vec<String>,
    /// Total candidates evaluated.
    pub candidates_evaluated: u32,
    /// All scored candidates (sorted by score).
    pub candidates: Vec<AcquireCandidateResponse>,
    /// Best candidate (if any).
    pub best_candidate: Option<AcquireCandidateResponse>,
    /// Whether the best candidate was auto-approved.
    pub auto_approved: bool,
    /// Method used for query building.
    pub query_method: String,
    /// Method used for scoring.
    pub score_method: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// POST /api/v1/textbrain/acquire
///
/// Test the full TextBrain acquisition flow:
/// 1. Build queries using DumbQueryBuilder
/// 2. Search (live or cache-only)
/// 3. Score candidates using DumbMatcher
/// 4. Return scored results
pub async fn acquire(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AcquireRequest>,
) -> Result<Json<AcquireResponse>, impl IntoResponse> {
    use std::time::Instant;
    use torrentino_core::{CatalogSearchQuery, TorrentCandidate, TorrentFile, TorrentSource};

    let start = Instant::now();

    // Build QueryContext
    let mut context = QueryContext::new(body.tags, &body.description);
    if let Some(expected) = body.expected {
        context = context.with_expected(expected.into());
    }

    // If cache_only, we handle search differently
    if body.cache_only {
        // Build queries using DumbQueryBuilder
        let query_builder = DumbQueryBuilder::new();
        let query_result = match query_builder.build_queries(&context).await {
            Ok(r) => r,
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Query building failed: {}", e),
                    }),
                ))
            }
        };

        if query_result.queries.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No queries could be generated".to_string(),
                }),
            ));
        }

        // Search cache with each query
        let catalog = state.catalog();
        let mut all_candidates: Vec<TorrentCandidate> = Vec::new();
        let mut queries_tried: Vec<String> = Vec::new();
        let mut seen_hashes = std::collections::HashSet::new();

        for query_str in query_result.queries.iter().take(5) {
            queries_tried.push(query_str.clone());

            let catalog_query = CatalogSearchQuery {
                query: query_str.clone(),
                limit: 50,
            };

            if let Ok(cached) = catalog.search(&catalog_query) {
                for ct in cached {
                    if seen_hashes.insert(ct.info_hash.clone()) {
                        all_candidates.push(TorrentCandidate {
                            title: ct.title,
                            info_hash: ct.info_hash,
                            size_bytes: ct.size_bytes,
                            seeders: ct.sources.iter().map(|s| s.seeders).sum(),
                            leechers: ct.sources.iter().map(|s| s.leechers).sum(),
                            category: ct.category,
                            publish_date: None,
                            files: ct.files.map(|files| {
                                files
                                    .into_iter()
                                    .map(|f| TorrentFile {
                                        path: f.path,
                                        size_bytes: f.size_bytes,
                                    })
                                    .collect()
                            }),
                            sources: ct
                                .sources
                                .into_iter()
                                .map(|s| TorrentSource {
                                    indexer: s.indexer,
                                    magnet_uri: s.magnet_uri,
                                    torrent_url: s.torrent_url,
                                    seeders: s.seeders,
                                    leechers: s.leechers,
                                    details_url: s.details_url,
                                })
                                .collect(),
                            from_cache: true,
                        });
                    }
                }
            }
        }

        let candidates_evaluated = all_candidates.len() as u32;

        // Score candidates using DumbMatcher
        let matcher = DumbMatcher::new();
        let match_result = match matcher.score_candidates(&context, &all_candidates).await {
            Ok(r) => r,
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Scoring failed: {}", e),
                    }),
                ))
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        let candidates: Vec<AcquireCandidateResponse> = match_result
            .candidates
            .iter()
            .map(|c| AcquireCandidateResponse {
                title: c.candidate.title.clone(),
                info_hash: c.candidate.info_hash.clone(),
                size_bytes: c.candidate.size_bytes,
                seeders: c.candidate.seeders,
                score: c.score,
                reasoning: c.reasoning.clone(),
            })
            .collect();

        let best_candidate = match_result.candidates.first().map(|c| AcquireCandidateResponse {
            title: c.candidate.title.clone(),
            info_hash: c.candidate.info_hash.clone(),
            size_bytes: c.candidate.size_bytes,
            seeders: c.candidate.seeders,
            score: c.score,
            reasoning: c.reasoning.clone(),
        });

        let auto_approved = best_candidate
            .as_ref()
            .map(|c| c.score >= body.auto_approve_threshold)
            .unwrap_or(false);

        return Ok(Json(AcquireResponse {
            queries_tried,
            candidates_evaluated,
            candidates,
            best_candidate,
            auto_approved,
            query_method: query_result.method,
            score_method: match_result.method,
            duration_ms,
        }));
    }

    // Normal flow: use TextBrain with live searcher
    let searcher = match state.searcher() {
        Some(s) => s,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Search backend not configured".to_string(),
                }),
            ))
        }
    };

    // Create TextBrain with dumb implementations and file enricher
    let config = TextBrainConfig {
        mode: TextBrainMode::DumbOnly,
        auto_approve_threshold: body.auto_approve_threshold,
        ..Default::default()
    };

    // Create file enricher using catalog from state
    let catalog = Arc::clone(state.catalog());
    let enricher = FileEnricher::new(catalog, config.file_enrichment.clone());

    let brain = TextBrain::new(config)
        .with_dumb_query_builder(Arc::new(DumbQueryBuilder::new()))
        .with_dumb_matcher(Arc::new(DumbMatcher::new()))
        .with_file_enricher(Arc::new(enricher));

    // Run acquisition
    match brain.acquire(&context, searcher.as_ref()).await {
        Ok(result) => {
            let candidates: Vec<AcquireCandidateResponse> = result
                .all_candidates
                .iter()
                .map(|c| AcquireCandidateResponse {
                    title: c.candidate.title.clone(),
                    info_hash: c.candidate.info_hash.clone(),
                    size_bytes: c.candidate.size_bytes,
                    seeders: c.candidate.seeders,
                    score: c.score,
                    reasoning: c.reasoning.clone(),
                })
                .collect();

            let best_candidate = result.best_candidate.map(|c| AcquireCandidateResponse {
                title: c.candidate.title.clone(),
                info_hash: c.candidate.info_hash.clone(),
                size_bytes: c.candidate.size_bytes,
                seeders: c.candidate.seeders,
                score: c.score,
                reasoning: c.reasoning.clone(),
            });

            Ok(Json(AcquireResponse {
                queries_tried: result.queries_tried,
                candidates_evaluated: result.candidates_evaluated,
                candidates,
                best_candidate,
                auto_approved: result.auto_approved,
                query_method: result.query_method,
                score_method: result.score_method,
                duration_ms: result.duration_ms,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Acquisition failed: {}", e),
            }),
        )),
    }
}

// ============================================================================
// Config endpoint
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub mode: String,
    pub auto_approve_threshold: f32,
    pub llm_configured: bool,
    pub llm_provider: Option<String>,
}

/// GET /api/v1/textbrain/config
///
/// Get TextBrain configuration status.
pub async fn get_config() -> Json<ConfigResponse> {
    // For now, return default dumb-only config
    // In the future, this would read from actual config
    Json(ConfigResponse {
        mode: "dumb_only".to_string(),
        auto_approve_threshold: 0.85,
        llm_configured: false,
        llm_provider: None,
    })
}

// ============================================================================
// Query building endpoint
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct BuildQueriesRequest {
    pub context: ContextRequest,
}

#[derive(Debug, Deserialize)]
pub struct ContextRequest {
    pub tags: Vec<String>,
    pub description: String,
    #[serde(default)]
    pub expected: Option<ExpectedContentRequest>,
}

#[derive(Debug, Serialize)]
pub struct BuildQueriesResponse {
    pub result: QueryBuildResultResponse,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct QueryBuildResultResponse {
    pub queries: Vec<String>,
    pub method: String,
    pub confidence: f32,
    pub llm_usage: Option<LlmUsage>,
}

/// POST /api/v1/textbrain/queries
///
/// Build search queries from context.
pub async fn build_queries(
    Json(body): Json<BuildQueriesRequest>,
) -> Result<Json<BuildQueriesResponse>, impl IntoResponse> {
    use std::time::Instant;

    let start = Instant::now();

    // Build QueryContext
    let mut context = QueryContext::new(body.context.tags, &body.context.description);
    if let Some(expected) = body.context.expected {
        context = context.with_expected(expected.into());
    }

    // Use DumbQueryBuilder
    let builder = DumbQueryBuilder::new();
    match builder.build_queries(&context).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(Json(BuildQueriesResponse {
                result: QueryBuildResultResponse {
                    queries: result.queries,
                    method: result.method,
                    confidence: result.confidence,
                    llm_usage: result.llm_usage,
                },
                duration_ms,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Query building failed: {}", e),
            }),
        )),
    }
}

// ============================================================================
// Scoring endpoint
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ScoreRequest {
    pub context: ContextRequest,
    pub candidates: Vec<CandidateRequest>,
}

#[derive(Debug, Deserialize)]
pub struct CandidateRequest {
    pub title: String,
    pub info_hash: String,
    pub size_bytes: u64,
    pub seeders: u32,
    pub leechers: u32,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub files: Option<Vec<FileRequest>>,
}

#[derive(Debug, Deserialize)]
pub struct FileRequest {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ScoreResponse {
    pub result: MatchResultResponse,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct MatchResultResponse {
    pub candidates: Vec<ScoredCandidateResponse>,
    pub method: String,
    pub llm_usage: Option<LlmUsage>,
}

#[derive(Debug, Serialize)]
pub struct ScoredCandidateResponse {
    pub candidate: CandidateResponse,
    pub score: f32,
    pub reasoning: String,
    pub file_mappings: Vec<FileMappingResponse>,
}

#[derive(Debug, Serialize)]
pub struct CandidateResponse {
    pub title: String,
    pub info_hash: String,
    pub size_bytes: u64,
    pub seeders: u32,
    pub leechers: u32,
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileMappingResponse {
    pub torrent_file_path: String,
    pub ticket_item_id: String,
    pub confidence: f32,
}

/// POST /api/v1/textbrain/score
///
/// Score candidates against context.
pub async fn score_candidates(
    Json(body): Json<ScoreRequest>,
) -> Result<Json<ScoreResponse>, impl IntoResponse> {
    use std::time::Instant;
    use torrentino_core::{TorrentCandidate, TorrentFile, TorrentSource};

    let start = Instant::now();

    // Build QueryContext
    let mut context = QueryContext::new(body.context.tags, &body.context.description);
    if let Some(expected) = body.context.expected {
        context = context.with_expected(expected.into());
    }

    // Convert candidates
    let candidates: Vec<TorrentCandidate> = body
        .candidates
        .into_iter()
        .map(|c| TorrentCandidate {
            title: c.title,
            info_hash: c.info_hash,
            size_bytes: c.size_bytes,
            seeders: c.seeders,
            leechers: c.leechers,
            category: c.category,
            publish_date: None,
            files: c.files.map(|files| {
                files
                    .into_iter()
                    .map(|f| TorrentFile {
                        path: f.path,
                        size_bytes: f.size_bytes,
                    })
                    .collect()
            }),
            sources: vec![TorrentSource {
                indexer: "manual".to_string(),
                magnet_uri: None,
                torrent_url: None,
                seeders: 0,
                leechers: 0,
                details_url: None,
            }],
            from_cache: false,
        })
        .collect();

    // Use DumbMatcher
    let matcher = DumbMatcher::new();
    match matcher.score_candidates(&context, &candidates).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;

            let scored_candidates: Vec<ScoredCandidateResponse> = result
                .candidates
                .into_iter()
                .map(|sc| ScoredCandidateResponse {
                    candidate: CandidateResponse {
                        title: sc.candidate.title,
                        info_hash: sc.candidate.info_hash,
                        size_bytes: sc.candidate.size_bytes,
                        seeders: sc.candidate.seeders,
                        leechers: sc.candidate.leechers,
                        category: sc.candidate.category,
                    },
                    score: sc.score,
                    reasoning: sc.reasoning,
                    file_mappings: sc
                        .file_mappings
                        .into_iter()
                        .map(|fm| FileMappingResponse {
                            torrent_file_path: fm.torrent_file_path,
                            ticket_item_id: fm.ticket_item_id,
                            confidence: fm.confidence,
                        })
                        .collect(),
                })
                .collect();

            Ok(Json(ScoreResponse {
                result: MatchResultResponse {
                    candidates: scored_candidates,
                    method: result.method,
                    llm_usage: result.llm_usage,
                },
                duration_ms,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Scoring failed: {}", e),
            }),
        )),
    }
}
