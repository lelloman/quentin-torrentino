//! Catalog API handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use torrentino_core::{CachedTorrent, CatalogSearchQuery, CatalogStats};

use crate::state::AppState;

// ============================================================================
// Request/Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CatalogQueryParams {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    100
}

#[derive(Debug, Serialize)]
pub struct CatalogListResponse {
    pub entries: Vec<CachedTorrent>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/v1/catalog
///
/// Search or list cached torrents.
pub async fn list_catalog(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CatalogQueryParams>,
) -> Result<Json<CatalogListResponse>, impl IntoResponse> {
    let catalog = state.catalog();

    let query = CatalogSearchQuery {
        query: params.query.unwrap_or_default(),
        limit: params.limit,
    };

    match catalog.search(&query) {
        Ok(entries) => {
            let total = entries.len();
            Ok(Json(CatalogListResponse { entries, total }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/catalog/stats
///
/// Get catalog statistics.
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CatalogStats>, impl IntoResponse> {
    let catalog = state.catalog();

    match catalog.stats() {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/catalog/{hash}
///
/// Get a specific cached torrent by info hash.
pub async fn get_entry(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<CachedTorrent>, impl IntoResponse> {
    let catalog = state.catalog();

    match catalog.get(&hash) {
        Ok(entry) => Ok(Json(entry)),
        Err(torrentino_core::CatalogError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Torrent not found: {}", hash),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// DELETE /api/v1/catalog/{hash}
///
/// Remove a torrent from the cache.
pub async fn remove_entry(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let catalog = state.catalog();

    match catalog.remove(&hash) {
        Ok(()) => Ok(Json(SuccessResponse {
            message: format!("Removed {} from catalog", hash),
        })),
        Err(torrentino_core::CatalogError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Torrent not found: {}", hash),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// DELETE /api/v1/catalog
///
/// Clear the entire catalog.
pub async fn clear_catalog(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let catalog = state.catalog();

    match catalog.clear() {
        Ok(()) => Ok(Json(SuccessResponse {
            message: "Catalog cleared".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}
