//! Searcher API handlers.

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use torrentino_core::{
    AuditEvent, IndexerStatus, SearchCategory, SearchQuery, SearchResult, TorrentCandidate,
};

use crate::state::AppState;

// ============================================================================
// Request/Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub indexers: Option<Vec<String>>,
    #[serde(default)]
    pub categories: Option<Vec<SearchCategory>>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: SearchQueryResponse,
    pub candidates: Vec<TorrentCandidate>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub indexer_errors: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct SearchQueryResponse {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<SearchCategory>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl From<SearchResult> for SearchResponse {
    fn from(result: SearchResult) -> Self {
        Self {
            query: SearchQueryResponse {
                query: result.query.query,
                indexers: result.query.indexers,
                categories: result.query.categories,
                limit: result.query.limit,
            },
            candidates: result.candidates,
            duration_ms: result.duration_ms,
            indexer_errors: result.indexer_errors,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SearcherStatusResponse {
    pub backend: String,
    pub configured: bool,
    pub indexers_count: usize,
    pub indexers_enabled: usize,
}

#[derive(Debug, Serialize)]
pub struct IndexersResponse {
    pub indexers: Vec<IndexerStatus>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/v1/search
///
/// Execute a search across configured indexers.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, impl IntoResponse> {
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

    let query = SearchQuery {
        query: body.query.clone(),
        indexers: body.indexers.clone(),
        categories: body.categories.clone(),
        limit: body.limit,
    };

    match searcher.search(&query).await {
        Ok(result) => {
            // Collect indexers that were actually queried
            let indexers_queried: Vec<String> = result
                .candidates
                .iter()
                .flat_map(|c| c.sources.iter().map(|s| s.indexer.clone()))
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            // Emit audit event
            state.audit().try_emit(AuditEvent::SearchExecuted {
                user_id: "anonymous".to_string(), // TODO: Get from auth
                searcher: searcher.name().to_string(),
                query: body.query,
                indexers_queried,
                results_count: result.candidates.len() as u32,
                duration_ms: result.duration_ms,
                indexer_errors: result.indexer_errors.clone(),
            });

            Ok(Json(SearchResponse::from(result)))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/searcher/status
///
/// Get searcher status and configuration.
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<SearcherStatusResponse> {
    match state.searcher() {
        Some(searcher) => {
            let indexers = searcher.indexer_status().await;
            let enabled_count = indexers.iter().filter(|i| i.enabled).count();

            Json(SearcherStatusResponse {
                backend: searcher.name().to_string(),
                configured: true,
                indexers_count: indexers.len(),
                indexers_enabled: enabled_count,
            })
        }
        None => Json(SearcherStatusResponse {
            backend: "none".to_string(),
            configured: false,
            indexers_count: 0,
            indexers_enabled: 0,
        }),
    }
}

/// GET /api/v1/searcher/indexers
///
/// List all configured indexers with their status.
/// Note: Indexers are read-only and configured in Jackett.
pub async fn list_indexers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<IndexersResponse>, impl IntoResponse> {
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

    let indexers = searcher.indexer_status().await;
    Ok(Json(IndexersResponse { indexers }))
}
