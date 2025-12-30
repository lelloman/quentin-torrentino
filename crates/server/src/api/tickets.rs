//! Ticket API handlers.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use torrentino_core::{
    AuditEvent, CreateTicketRequest, QueryContext, SelectedCandidate, Ticket, TicketError,
    TicketFilter, TicketState,
};

use crate::state::AppState;

/// Maximum allowed limit for ticket queries
const MAX_LIMIT: i64 = 1000;

/// Default limit for ticket queries
const DEFAULT_LIMIT: i64 = 100;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request body for creating a ticket
#[derive(Debug, Deserialize)]
pub struct CreateTicketBody {
    /// Priority for queue ordering (higher = more urgent)
    pub priority: Option<u16>,
    /// Query context for search/matching
    pub query_context: QueryContextBody,
    /// Destination path for final output
    pub dest_path: String,
}

/// Query context in request body
#[derive(Debug, Deserialize)]
pub struct QueryContextBody {
    /// Structured tags for categorization
    pub tags: Vec<String>,
    /// Freeform description for matching
    pub description: String,
}

/// Query parameters for listing tickets
#[derive(Debug, Deserialize)]
pub struct ListTicketsParams {
    /// Filter by state type
    pub state: Option<String>,
    /// Filter by creator
    pub created_by: Option<String>,
    /// Maximum number of tickets to return
    pub limit: Option<i64>,
    /// Pagination offset
    pub offset: Option<i64>,
}

/// Request body for cancelling a ticket
#[derive(Debug, Deserialize)]
pub struct CancelTicketBody {
    /// Optional reason for cancellation
    pub reason: Option<String>,
}

/// Request body for approving a ticket
#[derive(Debug, Deserialize)]
pub struct ApproveTicketBody {
    /// Index of the candidate to approve (0-based).
    /// Defaults to 0 (the recommended candidate).
    pub candidate_idx: Option<usize>,
}

/// Request body for rejecting a ticket
#[derive(Debug, Deserialize)]
pub struct RejectTicketBody {
    /// Optional reason for rejection
    pub reason: Option<String>,
}

/// Response for ticket operations
#[derive(Debug, Serialize)]
pub struct TicketResponse {
    pub id: String,
    pub created_at: String,
    pub created_by: String,
    pub state: TicketState,
    pub priority: u16,
    pub query_context: QueryContext,
    pub dest_path: String,
    pub updated_at: String,
}

impl From<Ticket> for TicketResponse {
    fn from(ticket: Ticket) -> Self {
        Self {
            id: ticket.id,
            created_at: ticket.created_at.to_rfc3339(),
            created_by: ticket.created_by,
            state: ticket.state,
            priority: ticket.priority,
            query_context: ticket.query_context,
            dest_path: ticket.dest_path,
            updated_at: ticket.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing tickets
#[derive(Debug, Serialize)]
pub struct ListTicketsResponse {
    pub tickets: Vec<TicketResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct TicketErrorResponse {
    pub error: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Create a new ticket
pub async fn create_ticket(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateTicketBody>,
) -> Result<(StatusCode, Json<TicketResponse>), impl IntoResponse> {
    let request = CreateTicketRequest {
        created_by: "anonymous".to_string(), // TODO: Get from auth
        priority: body.priority.unwrap_or(0),
        query_context: QueryContext::new(body.query_context.tags.clone(), &body.query_context.description),
        dest_path: body.dest_path.clone(),
    };

    match state.ticket_store().create(request) {
        Ok(ticket) => {
            // Emit audit event
            state
                .audit()
                .try_emit(AuditEvent::TicketCreated {
                    ticket_id: ticket.id.clone(),
                    requested_by: ticket.created_by.clone(),
                    priority: ticket.priority,
                    tags: ticket.query_context.tags.clone(),
                    description: ticket.query_context.description.clone(),
                    dest_path: ticket.dest_path.clone(),
                });

            Ok((StatusCode::CREATED, Json(TicketResponse::from(ticket))))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TicketErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Get a ticket by ID
pub async fn get_ticket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    match state.ticket_store().get(&id) {
        Ok(Some(ticket)) => Ok(Json(TicketResponse::from(ticket))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(TicketErrorResponse {
                error: format!("Ticket not found: {}", id),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TicketErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// List tickets with optional filters
pub async fn list_tickets(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTicketsParams>,
) -> Result<Json<ListTicketsResponse>, impl IntoResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut filter = TicketFilter::new().with_limit(limit).with_offset(offset);

    if let Some(ref state_filter) = params.state {
        filter = filter.with_state(state_filter);
    }

    if let Some(ref created_by) = params.created_by {
        filter = filter.with_created_by(created_by);
    }

    let tickets = match state.ticket_store().list(&filter) {
        Ok(tickets) => tickets,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    // Get total count (without pagination)
    let count_filter = TicketFilter {
        limit: i64::MAX,
        offset: 0,
        ..filter.clone()
    };

    let total = match state.ticket_store().count(&count_filter) {
        Ok(count) => count,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    Ok(Json(ListTicketsResponse {
        tickets: tickets.into_iter().map(TicketResponse::from).collect(),
        total,
        limit,
        offset,
    }))
}

/// Cancel a ticket (DELETE endpoint)
pub async fn cancel_ticket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<CancelTicketBody>>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    let reason = body.and_then(|b| b.reason.clone());
    let cancelled_by = "anonymous".to_string(); // TODO: Get from auth

    // First get the current ticket to check state
    let current_ticket = match state.ticket_store().get(&id) {
        Ok(Some(ticket)) => ticket,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(TicketErrorResponse {
                    error: format!("Ticket not found: {}", id),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    let previous_state = current_ticket.state.state_type().to_string();

    let new_state = TicketState::Cancelled {
        cancelled_by: cancelled_by.clone(),
        reason: reason.clone(),
        cancelled_at: Utc::now(),
    };

    match state.ticket_store().update_state(&id, new_state) {
        Ok(ticket) => {
            // Emit audit event
            state
                .audit()
                .try_emit(AuditEvent::TicketCancelled {
                    ticket_id: ticket.id.clone(),
                    cancelled_by,
                    reason,
                    previous_state,
                });

            Ok(Json(TicketResponse::from(ticket)))
        }
        Err(TicketError::NotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(TicketErrorResponse {
                error: format!("Ticket not found: {}", id),
            }),
        )),
        Err(TicketError::InvalidState {
            current_state,
            operation,
            ..
        }) => Err((
            StatusCode::CONFLICT,
            Json(TicketErrorResponse {
                error: format!(
                    "Cannot {} ticket: current state is {}",
                    operation, current_state
                ),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TicketErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Approve a ticket (for tickets in NeedsApproval state)
pub async fn approve_ticket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<ApproveTicketBody>>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    let candidate_idx = body.and_then(|b| b.candidate_idx).unwrap_or(0);
    let approved_by = "anonymous".to_string(); // TODO: Get from auth

    // Get the current ticket
    let current_ticket = match state.ticket_store().get(&id) {
        Ok(Some(ticket)) => ticket,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(TicketErrorResponse {
                    error: format!("Ticket not found: {}", id),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    // Check that ticket is in NeedsApproval state
    let candidates = match &current_ticket.state {
        TicketState::NeedsApproval { candidates, .. } => candidates,
        _ => {
            return Err((
                StatusCode::CONFLICT,
                Json(TicketErrorResponse {
                    error: format!(
                        "Cannot approve ticket: current state is {}",
                        current_ticket.state.state_type()
                    ),
                }),
            ));
        }
    };

    // Check that candidate index is valid
    if candidate_idx >= candidates.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(TicketErrorResponse {
                error: format!(
                    "Invalid candidate index: {} (only {} candidates available)",
                    candidate_idx,
                    candidates.len()
                ),
            }),
        ));
    }

    let selected_summary = &candidates[candidate_idx];

    // Look up the full candidate from the catalog to get the magnet URI
    let magnet_uri = match state.catalog().get(&selected_summary.info_hash) {
        Ok(cached) => {
            // Find a source with a magnet URI
            cached
                .sources
                .iter()
                .find_map(|s| s.magnet_uri.clone())
                .unwrap_or_else(|| {
                    // Fall back to constructing from info hash
                    format!(
                        "magnet:?xt=urn:btih:{}&dn={}",
                        selected_summary.info_hash, selected_summary.title
                    )
                })
        }
        Err(_) => {
            // Fall back to constructing from info hash
            format!(
                "magnet:?xt=urn:btih:{}&dn={}",
                selected_summary.info_hash, selected_summary.title
            )
        }
    };

    // Build the selected candidate
    let selected = SelectedCandidate {
        title: selected_summary.title.clone(),
        info_hash: selected_summary.info_hash.clone(),
        magnet_uri,
        size_bytes: selected_summary.size_bytes,
        score: selected_summary.score,
        file_mappings: vec![], // TODO: Get from file mapper
    };

    // Build all candidates for failover (convert summaries to SelectedCandidate)
    let all_candidates: Vec<SelectedCandidate> = candidates
        .iter()
        .map(|c| {
            let magnet = match state.catalog().get(&c.info_hash) {
                Ok(cached) => cached
                    .sources
                    .iter()
                    .find_map(|s| s.magnet_uri.clone())
                    .unwrap_or_else(|| {
                        format!("magnet:?xt=urn:btih:{}&dn={}", c.info_hash, c.title)
                    }),
                Err(_) => format!("magnet:?xt=urn:btih:{}&dn={}", c.info_hash, c.title),
            };
            SelectedCandidate {
                title: c.title.clone(),
                info_hash: c.info_hash.clone(),
                magnet_uri: magnet,
                size_bytes: c.size_bytes,
                score: c.score,
                file_mappings: vec![],
            }
        })
        .collect();

    let previous_state = current_ticket.state.state_type().to_string();

    let new_state = TicketState::Approved {
        selected,
        candidates: all_candidates,
        approved_by: approved_by.clone(),
        approved_at: Utc::now(),
    };

    match state.ticket_store().update_state(&id, new_state) {
        Ok(ticket) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TicketStateChanged {
                ticket_id: ticket.id.clone(),
                from_state: previous_state,
                to_state: "approved".to_string(),
                reason: Some(format!("Approved candidate {} by {}", candidate_idx, approved_by)),
            });

            Ok(Json(TicketResponse::from(ticket)))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TicketErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Reject a ticket (for tickets in NeedsApproval state)
pub async fn reject_ticket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<RejectTicketBody>>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    let reason = body.and_then(|b| b.reason.clone());
    let rejected_by = "anonymous".to_string(); // TODO: Get from auth

    // Get the current ticket
    let current_ticket = match state.ticket_store().get(&id) {
        Ok(Some(ticket)) => ticket,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(TicketErrorResponse {
                    error: format!("Ticket not found: {}", id),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    // Check that ticket is in NeedsApproval state
    if !matches!(current_ticket.state, TicketState::NeedsApproval { .. }) {
        return Err((
            StatusCode::CONFLICT,
            Json(TicketErrorResponse {
                error: format!(
                    "Cannot reject ticket: current state is {}",
                    current_ticket.state.state_type()
                ),
            }),
        ));
    }

    let previous_state = current_ticket.state.state_type().to_string();

    let new_state = TicketState::Rejected {
        rejected_by: rejected_by.clone(),
        reason: reason.clone(),
        rejected_at: Utc::now(),
    };

    match state.ticket_store().update_state(&id, new_state) {
        Ok(ticket) => {
            // Emit audit event
            state.audit().try_emit(AuditEvent::TicketStateChanged {
                ticket_id: ticket.id.clone(),
                from_state: previous_state,
                to_state: "rejected".to_string(),
                reason,
            });

            Ok(Json(TicketResponse::from(ticket)))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TicketErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}
