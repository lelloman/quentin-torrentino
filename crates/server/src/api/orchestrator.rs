//! Orchestrator API handlers.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

// ============================================================================
// Response Types
// ============================================================================

/// Orchestrator status response
#[derive(Debug, Serialize)]
pub struct OrchestratorStatusResponse {
    /// Whether the orchestrator is available (configured and dependencies met)
    pub available: bool,
    /// Whether the orchestrator is currently running
    pub running: bool,
    /// Number of active downloads being tracked
    pub active_downloads: usize,
    /// Tickets currently being acquired (should be 0 or 1)
    pub acquiring_count: usize,
    /// Tickets waiting for acquisition
    pub pending_count: usize,
    /// Tickets waiting for manual approval
    pub needs_approval_count: usize,
    /// Tickets currently downloading
    pub downloading_count: usize,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct OrchestratorErrorResponse {
    pub error: String,
}

/// Simple message response
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Get orchestrator status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Json<OrchestratorStatusResponse> {
    match state.orchestrator() {
        Some(orch) => {
            let status = orch.status().await;
            Json(OrchestratorStatusResponse {
                available: true,
                running: status.running,
                active_downloads: status.active_downloads,
                acquiring_count: status.acquiring_count,
                pending_count: status.pending_count,
                needs_approval_count: status.needs_approval_count,
                downloading_count: status.downloading_count,
            })
        }
        None => Json(OrchestratorStatusResponse {
            available: false,
            running: false,
            active_downloads: 0,
            acquiring_count: 0,
            pending_count: 0,
            needs_approval_count: 0,
            downloading_count: 0,
        }),
    }
}

/// Start the orchestrator
pub async fn start(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MessageResponse>, impl IntoResponse> {
    match state.orchestrator() {
        Some(orch) => {
            orch.start().await;
            Ok(Json(MessageResponse {
                message: "Orchestrator started".to_string(),
            }))
        }
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(OrchestratorErrorResponse {
                error: "Orchestrator not available. Check that searcher and torrent_client are configured.".to_string(),
            }),
        )),
    }
}

/// Stop the orchestrator
pub async fn stop(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MessageResponse>, impl IntoResponse> {
    match state.orchestrator() {
        Some(orch) => {
            orch.stop().await;
            Ok(Json(MessageResponse {
                message: "Orchestrator stopped".to_string(),
            }))
        }
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(OrchestratorErrorResponse {
                error: "Orchestrator not available".to_string(),
            }),
        )),
    }
}
