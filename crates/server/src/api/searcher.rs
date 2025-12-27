//! Searcher API handlers.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use torrentino_core::{
    AuditEvent, CatalogSearchQuery, IndexerStatus, SearchCategory, SearchMode, SearchQuery,
    TorrentCandidate,
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
    /// Search mode: cache_only, external_only, or both (default)
    #[serde(default)]
    pub mode: SearchMode,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: SearchQueryResponse,
    pub candidates: Vec<TorrentCandidate>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub indexer_errors: std::collections::HashMap<String, String>,
    /// Number of results from cache
    pub cache_hits: usize,
    /// Number of results from external search
    pub external_hits: usize,
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
/// Execute a search across configured indexers and/or the local cache.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, impl IntoResponse> {
    let start = std::time::Instant::now();
    let catalog = state.catalog();
    let limit = body.limit.unwrap_or(100);

    let mut cached_results: Vec<TorrentCandidate> = Vec::new();
    let mut external_results: Vec<TorrentCandidate> = Vec::new();
    let mut indexer_errors = std::collections::HashMap::new();

    // 1. Search catalog (if mode allows)
    if body.mode != SearchMode::ExternalOnly {
        let catalog_query = CatalogSearchQuery {
            query: body.query.clone(),
            limit,
        };

        match catalog.search(&catalog_query) {
            Ok(cached) => {
                // Convert CachedTorrent to TorrentCandidate with from_cache = true
                for ct in cached {
                    cached_results.push(TorrentCandidate {
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
                                .map(|f| torrentino_core::TorrentFile {
                                    path: f.path,
                                    size_bytes: f.size_bytes,
                                })
                                .collect()
                        }),
                        sources: ct
                            .sources
                            .into_iter()
                            .map(|s| torrentino_core::TorrentSource {
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
            Err(e) => {
                tracing::warn!("Catalog search failed: {}", e);
            }
        }
    }

    // 2. Search external (if mode allows and searcher is configured)
    if body.mode != SearchMode::CacheOnly {
        if let Some(searcher) = state.searcher() {
            let query = SearchQuery {
                query: body.query.clone(),
                indexers: body.indexers.clone(),
                categories: body.categories.clone(),
                limit: body.limit,
            };

            match searcher.search(&query).await {
                Ok(result) => {
                    // Store results in catalog for future searches
                    if let Err(e) = catalog.store(&result.candidates) {
                        tracing::warn!("Failed to store results in catalog: {}", e);
                    }

                    external_results = result.candidates;
                    indexer_errors = result.indexer_errors;
                }
                Err(e) => {
                    // If we have cache results, don't fail completely
                    if cached_results.is_empty() {
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: e.to_string(),
                            }),
                        ));
                    }
                    tracing::warn!("External search failed, using cache only: {}", e);
                }
            }
        } else if body.mode == SearchMode::ExternalOnly {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Search backend not configured".to_string(),
                }),
            ));
        }
    }

    let cache_hits = cached_results.len();
    let external_hits = external_results.len();

    // 3. Merge and deduplicate by info_hash
    // External results take precedence (fresher seeder counts)
    let mut seen_hashes: HashSet<String> = HashSet::new();
    let mut combined: Vec<TorrentCandidate> = Vec::new();

    // Add external results first (they have fresher data)
    for mut candidate in external_results {
        if !candidate.info_hash.is_empty() {
            seen_hashes.insert(candidate.info_hash.to_lowercase());
        }
        candidate.from_cache = false;
        combined.push(candidate);
    }

    // Add cached results that aren't duplicates
    for mut candidate in cached_results {
        let hash = candidate.info_hash.to_lowercase();
        if hash.is_empty() || !seen_hashes.contains(&hash) {
            if !hash.is_empty() {
                seen_hashes.insert(hash);
            }
            candidate.from_cache = true;
            combined.push(candidate);
        }
    }

    // Sort by seeders descending
    combined.sort_by(|a, b| b.seeders.cmp(&a.seeders));

    // Apply limit
    if combined.len() > limit as usize {
        combined.truncate(limit as usize);
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    // Emit audit event
    let indexers_queried: Vec<String> = combined
        .iter()
        .flat_map(|c| c.sources.iter().map(|s| s.indexer.clone()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    state.audit().try_emit(AuditEvent::SearchExecuted {
        user_id: "anonymous".to_string(), // TODO: Get from auth
        searcher: state
            .searcher()
            .map(|s| s.name().to_string())
            .unwrap_or_else(|| "cache_only".to_string()),
        query: body.query.clone(),
        indexers_queried,
        results_count: combined.len() as u32,
        duration_ms,
        indexer_errors: indexer_errors.clone(),
    });

    Ok(Json(SearchResponse {
        query: SearchQueryResponse {
            query: body.query,
            indexers: body.indexers,
            categories: body.categories,
            limit: body.limit,
        },
        candidates: combined,
        duration_ms,
        indexer_errors,
        cache_hits,
        external_hits,
    }))
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
