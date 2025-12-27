//! Torrent client API handlers.

use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use torrentino_core::{
    AddTorrentRequest, AuditEvent, TorrentFilters, TorrentInfo, TorrentState,
};

use crate::state::AppState;

// ============================================================================
// Request/Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AddMagnetRequest {
    pub uri: String,
    #[serde(default)]
    pub download_path: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub paused: bool,
    /// Associated ticket ID (for audit trail)
    #[serde(default)]
    pub ticket_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddFromUrlRequest {
    pub url: String,
    #[serde(default)]
    pub download_path: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub ticket_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TorrentFilterParams {
    #[serde(default)]
    pub state: Option<TorrentState>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RemoveTorrentParams {
    #[serde(default)]
    pub delete_files: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetLimitRequest {
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct AddTorrentResponse {
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TorrentListResponse {
    pub torrents: Vec<TorrentInfo>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct TorrentClientStatusResponse {
    pub backend: String,
    pub configured: bool,
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

/// GET /api/v1/torrents/status
///
/// Get torrent client status and configuration.
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<TorrentClientStatusResponse> {
    match state.torrent_client() {
        Some(client) => Json(TorrentClientStatusResponse {
            backend: client.name().to_string(),
            configured: true,
        }),
        None => Json(TorrentClientStatusResponse {
            backend: "none".to_string(),
            configured: false,
        }),
    }
}

/// GET /api/v1/torrents
///
/// List all torrents, optionally filtered.
pub async fn list_torrents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TorrentFilterParams>,
) -> Result<Json<TorrentListResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    let filters = TorrentFilters {
        state: params.state,
        category: params.category,
        search: params.search,
    };

    match client.list_torrents(&filters).await {
        Ok(torrents) => {
            let count = torrents.len();
            Ok(Json(TorrentListResponse { torrents, count }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// GET /api/v1/torrents/{hash}
///
/// Get a specific torrent by hash.
pub async fn get_torrent(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<TorrentInfo>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    match client.get_torrent(&hash).await {
        Ok(torrent) => Ok(Json(torrent)),
        Err(torrentino_core::TorrentClientError::TorrentNotFound(_)) => Err((
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

/// POST /api/v1/torrents/add/magnet
///
/// Add a torrent via magnet URI.
pub async fn add_magnet(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddMagnetRequest>,
) -> Result<Json<AddTorrentResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    let request = AddTorrentRequest::Magnet {
        uri: body.uri.clone(),
        download_path: body.download_path,
        category: body.category,
        paused: body.paused,
    };

    match client.add_torrent(request).await {
        Ok(result) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentAdded {
                user_id: "anonymous".to_string(), // TODO: Get from auth
                hash: result.hash.clone(),
                name: result.name.clone(),
                source: "magnet".to_string(),
                ticket_id: body.ticket_id,
            });

            Ok(Json(AddTorrentResponse {
                hash: result.hash,
                name: result.name,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// POST /api/v1/torrents/add/file
///
/// Add a torrent via .torrent file upload.
pub async fn add_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<AddTorrentResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Parse multipart form
    let mut torrent_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut download_path: Option<String> = None;
    let mut category: Option<String> = None;
    let mut paused = false;
    let mut ticket_id: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => torrent_data = Some(bytes.to_vec()),
                    Err(e) => {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: format!("Failed to read file: {}", e),
                            }),
                        ))
                    }
                }
            }
            "download_path" => {
                if let Ok(text) = field.text().await {
                    if !text.is_empty() {
                        download_path = Some(text);
                    }
                }
            }
            "category" => {
                if let Ok(text) = field.text().await {
                    if !text.is_empty() {
                        category = Some(text);
                    }
                }
            }
            "paused" => {
                if let Ok(text) = field.text().await {
                    paused = text == "true" || text == "1";
                }
            }
            "ticket_id" => {
                if let Ok(text) = field.text().await {
                    if !text.is_empty() {
                        ticket_id = Some(text);
                    }
                }
            }
            _ => {}
        }
    }

    let data = match torrent_data {
        Some(d) if !d.is_empty() => d,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No torrent file provided".to_string(),
                }),
            ))
        }
    };

    let request = AddTorrentRequest::TorrentFile {
        data,
        filename: filename.clone(),
        download_path,
        category,
        paused,
    };

    match client.add_torrent(request).await {
        Ok(result) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentAdded {
                user_id: "anonymous".to_string(), // TODO: Get from auth
                hash: result.hash.clone(),
                name: result.name.clone().or(filename),
                source: "file".to_string(),
                ticket_id,
            });

            Ok(Json(AddTorrentResponse {
                hash: result.hash,
                name: result.name,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// POST /api/v1/torrents/add/url
///
/// Add a torrent by fetching from a URL (handles redirects including magnet links).
pub async fn add_from_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddFromUrlRequest>,
) -> Result<Json<AddTorrentResponse>, impl IntoResponse> {
    tracing::info!(url = %body.url, "Fetching torrent from URL");

    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Create HTTP client that doesn't follow redirects automatically
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create HTTP client: {}", e),
                }),
            )
        })?;

    // Fetch the URL
    tracing::debug!("Sending GET request to URL");
    let response = http_client.get(&body.url).send().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to fetch URL");
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Failed to fetch URL: {}", e),
            }),
        )
    })?;
    tracing::debug!(status = %response.status(), "Got response from URL");

    // Check for redirect to magnet link
    if response.status().is_redirection() {
        if let Some(location) = response.headers().get("location") {
            let location_str = location.to_str().unwrap_or("");
            if location_str.starts_with("magnet:") {
                // It's a magnet link redirect - use it directly
                let request = AddTorrentRequest::Magnet {
                    uri: location_str.to_string(),
                    download_path: body.download_path,
                    category: body.category,
                    paused: body.paused,
                };

                return match client.add_torrent(request).await {
                    Ok(result) => {
                        state.audit().try_emit(AuditEvent::TorrentAdded {
                            user_id: "anonymous".to_string(),
                            hash: result.hash.clone(),
                            name: result.name.clone(),
                            source: "url".to_string(),
                            ticket_id: body.ticket_id,
                        });

                        Ok(Json(AddTorrentResponse {
                            hash: result.hash,
                            name: result.name,
                        }))
                    }
                    Err(e) => Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: e.to_string(),
                        }),
                    )),
                };
            }
        }
    }

    // Not a magnet redirect - try to get the response body as a .torrent file
    if !response.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("URL returned status: {}", response.status()),
            }),
        ));
    }

    let data = response.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Failed to read response body: {}", e),
            }),
        )
    })?;

    if data.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "Empty response from URL".to_string(),
            }),
        ));
    }

    let request = AddTorrentRequest::TorrentFile {
        data: data.to_vec(),
        filename: None,
        download_path: body.download_path,
        category: body.category,
        paused: body.paused,
    };

    match client.add_torrent(request).await {
        Ok(result) => {
            state.audit().try_emit(AuditEvent::TorrentAdded {
                user_id: "anonymous".to_string(),
                hash: result.hash.clone(),
                name: result.name.clone(),
                source: "url".to_string(),
                ticket_id: body.ticket_id,
            });

            Ok(Json(AddTorrentResponse {
                hash: result.hash,
                name: result.name,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// DELETE /api/v1/torrents/{hash}
///
/// Remove a torrent.
pub async fn remove_torrent(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(params): Query<RemoveTorrentParams>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Get torrent name for audit before removing
    let torrent_name = client
        .get_torrent(&hash)
        .await
        .map(|t| t.name)
        .unwrap_or_else(|_| hash.clone());

    match client.remove_torrent(&hash, params.delete_files).await {
        Ok(()) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentRemoved {
                user_id: "anonymous".to_string(), // TODO: Get from auth
                hash: hash.clone(),
                name: torrent_name,
                delete_files: params.delete_files,
            });

            Ok(Json(SuccessResponse {
                message: format!("Torrent {} removed", hash),
            }))
        }
        Err(torrentino_core::TorrentClientError::TorrentNotFound(_)) => Err((
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

/// POST /api/v1/torrents/{hash}/pause
///
/// Pause a torrent.
pub async fn pause_torrent(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Get torrent name for audit
    let torrent_name = client
        .get_torrent(&hash)
        .await
        .map(|t| t.name)
        .unwrap_or_else(|_| hash.clone());

    match client.pause_torrent(&hash).await {
        Ok(()) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentPaused {
                user_id: "anonymous".to_string(),
                hash: hash.clone(),
                name: torrent_name,
            });

            Ok(Json(SuccessResponse {
                message: format!("Torrent {} paused", hash),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// POST /api/v1/torrents/{hash}/resume
///
/// Resume a paused torrent.
pub async fn resume_torrent(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Get torrent name for audit
    let torrent_name = client
        .get_torrent(&hash)
        .await
        .map(|t| t.name)
        .unwrap_or_else(|_| hash.clone());

    match client.resume_torrent(&hash).await {
        Ok(()) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentResumed {
                user_id: "anonymous".to_string(),
                hash: hash.clone(),
                name: torrent_name,
            });

            Ok(Json(SuccessResponse {
                message: format!("Torrent {} resumed", hash),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// POST /api/v1/torrents/{hash}/upload-limit
///
/// Set upload speed limit for a torrent.
pub async fn set_upload_limit(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Json(body): Json<SetLimitRequest>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Get torrent info for audit
    let torrent = match client.get_torrent(&hash).await {
        Ok(t) => t,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ))
        }
    };
    let old_limit = torrent.upload_limit;

    match client.set_upload_limit(&hash, body.limit).await {
        Ok(()) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentLimitChanged {
                user_id: "anonymous".to_string(),
                hash: hash.clone(),
                name: torrent.name,
                limit_type: "upload".to_string(),
                old_limit,
                new_limit: body.limit,
            });

            Ok(Json(SuccessResponse {
                message: format!("Upload limit set to {} for {}", body.limit, hash),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// POST /api/v1/torrents/{hash}/download-limit
///
/// Set download speed limit for a torrent.
pub async fn set_download_limit(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Json(body): Json<SetLimitRequest>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Get torrent info for audit
    let torrent = match client.get_torrent(&hash).await {
        Ok(t) => t,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ))
        }
    };
    let old_limit = torrent.download_limit;

    match client.set_download_limit(&hash, body.limit).await {
        Ok(()) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentLimitChanged {
                user_id: "anonymous".to_string(),
                hash: hash.clone(),
                name: torrent.name,
                limit_type: "download".to_string(),
                old_limit,
                new_limit: body.limit,
            });

            Ok(Json(SuccessResponse {
                message: format!("Download limit set to {} for {}", body.limit, hash),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// POST /api/v1/torrents/{hash}/recheck
///
/// Recheck/verify torrent files.
pub async fn recheck_torrent(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<SuccessResponse>, impl IntoResponse> {
    let client = match state.torrent_client() {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Torrent client not configured".to_string(),
                }),
            ))
        }
    };

    // Get torrent name for audit
    let torrent_name = client
        .get_torrent(&hash)
        .await
        .map(|t| t.name)
        .unwrap_or_else(|_| hash.clone());

    match client.recheck_torrent(&hash).await {
        Ok(()) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TorrentRechecked {
                user_id: "anonymous".to_string(),
                hash: hash.clone(),
                name: torrent_name,
            });

            Ok(Json(SuccessResponse {
                message: format!("Recheck started for {}", hash),
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}
