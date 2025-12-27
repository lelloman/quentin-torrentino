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
    AnthropicClient, AuditEvent, CompletionRequest, LlmClient, LlmUsage, SearchQuery,
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
