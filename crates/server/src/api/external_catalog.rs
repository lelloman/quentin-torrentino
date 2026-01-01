//! External catalog API handlers for MusicBrainz and TMDB lookups.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use torrentino_core::{MusicBrainzRelease, TmdbMovie, TmdbSeason, TmdbSeries};

use crate::state::AppState;

// ============================================================================
// Request/Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct MusicBrainzSearchParams {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize)]
pub struct TmdbMovieSearchParams {
    pub query: String,
    #[serde(default)]
    pub year: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbTvSearchParams {
    pub query: String,
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct ExternalCatalogStatus {
    pub musicbrainz_available: bool,
    pub tmdb_available: bool,
}

// ============================================================================
// MusicBrainz Handlers
// ============================================================================

/// GET /api/v1/external-catalog/status
///
/// Check which external catalogs are available.
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<ExternalCatalogStatus> {
    let catalog = state.external_catalog();
    Json(ExternalCatalogStatus {
        // Both are available if the external catalog client is configured
        musicbrainz_available: catalog.is_some(),
        tmdb_available: catalog.is_some(),
    })
}

/// GET /api/v1/external-catalog/musicbrainz/search
///
/// Search MusicBrainz for releases.
pub async fn search_musicbrainz(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MusicBrainzSearchParams>,
) -> Result<Json<Vec<MusicBrainzRelease>>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.search_releases(&params.query, params.limit).await {
        Ok(releases) => Ok(Json(releases)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/external-catalog/musicbrainz/release/{mbid}
///
/// Get a specific release by MusicBrainz ID.
pub async fn get_musicbrainz_release(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> Result<Json<MusicBrainzRelease>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.get_release(&mbid).await {
        Ok(release) => Ok(Json(release)),
        Err(torrentino_core::ExternalCatalogError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Release not found: {}", mbid),
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

// ============================================================================
// TMDB Handlers
// ============================================================================

/// GET /api/v1/external-catalog/tmdb/movies/search
///
/// Search TMDB for movies.
pub async fn search_tmdb_movies(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TmdbMovieSearchParams>,
) -> Result<Json<Vec<TmdbMovie>>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.search_movies(&params.query, params.year).await {
        Ok(movies) => Ok(Json(movies)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/external-catalog/tmdb/movies/{id}
///
/// Get a specific movie by TMDB ID.
pub async fn get_tmdb_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<TmdbMovie>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.get_movie(id).await {
        Ok(movie) => Ok(Json(movie)),
        Err(torrentino_core::ExternalCatalogError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Movie not found: {}", id),
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

/// GET /api/v1/external-catalog/tmdb/tv/search
///
/// Search TMDB for TV series.
pub async fn search_tmdb_tv(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TmdbTvSearchParams>,
) -> Result<Json<Vec<TmdbSeries>>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.search_tv(&params.query).await {
        Ok(series) => Ok(Json(series)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/external-catalog/tmdb/tv/{id}
///
/// Get a specific TV series by TMDB ID.
pub async fn get_tmdb_tv(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<TmdbSeries>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.get_tv(id).await {
        Ok(series) => Ok(Json(series)),
        Err(torrentino_core::ExternalCatalogError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("TV series not found: {}", id),
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

/// GET /api/v1/external-catalog/tmdb/tv/{id}/season/{season}
///
/// Get a specific TV season.
pub async fn get_tmdb_season(
    State(state): State<Arc<AppState>>,
    Path((id, season)): Path<(u32, u32)>,
) -> Result<Json<TmdbSeason>, impl IntoResponse> {
    let Some(catalog) = state.external_catalog() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "External catalog not configured".to_string(),
            }),
        ));
    };

    match catalog.get_tv_season(id, season).await {
        Ok(season_data) => Ok(Json(season_data)),
        Err(torrentino_core::ExternalCatalogError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Season {} not found for TV series {}", season, id),
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
