//! Searcher API handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
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

#[derive(Debug, Deserialize)]
pub struct UpdateIndexerRequest {
    #[serde(default)]
    pub rate_limit_rpm: Option<u32>,
    #[serde(default)]
    pub enabled: Option<bool>,
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

/// PATCH /api/v1/searcher/indexers/{name}
///
/// Update an indexer's settings (rate limit, enabled state).
pub async fn update_indexer(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<UpdateIndexerRequest>,
) -> Result<Json<IndexerStatus>, impl IntoResponse> {
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

    // Get current status to find old values for audit
    let current_status: Option<IndexerStatus> = searcher
        .indexer_status()
        .await
        .into_iter()
        .find(|i| i.name == name);

    let current = match current_status {
        Some(s) => s,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Indexer '{}' not found", name),
                }),
            ))
        }
    };

    // Update rate limit if provided
    if let Some(new_rpm) = body.rate_limit_rpm {
        let old_rpm = current.rate_limit.requests_per_minute;
        if let Err(e) = searcher.update_indexer_rate_limit(&name, new_rpm).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }

        state.audit().try_emit(AuditEvent::IndexerRateLimitUpdated {
            user_id: "anonymous".to_string(), // TODO: Get from auth
            indexer: name.clone(),
            old_rpm,
            new_rpm,
        });
    }

    // Update enabled state if provided
    if let Some(enabled) = body.enabled {
        if enabled != current.enabled {
            if let Err(e) = searcher.set_indexer_enabled(&name, enabled).await {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                ));
            }

            state.audit().try_emit(AuditEvent::IndexerEnabledChanged {
                user_id: "anonymous".to_string(), // TODO: Get from auth
                indexer: name.clone(),
                enabled,
            });
        }
    }

    // Get updated status
    let updated_status: Option<IndexerStatus> = searcher
        .indexer_status()
        .await
        .into_iter()
        .find(|i| i.name == name);

    match updated_status {
        Some(status) => Ok(Json(status)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Indexer '{}' not found", name),
            }),
        )),
    }
}
