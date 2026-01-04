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
    AuditEvent, CatalogReference, CreateTicketRequest, ExpectedContent, OutputConstraints,
    QueryContext, SearchConstraints, SelectedCandidate, Ticket, TicketError, TicketFilter,
    TicketState,
};

use crate::api::AuthUser;
use crate::metrics::{TICKETS_CREATED_TOTAL, TICKETS_FAILED_TOTAL, TICKET_STATE_TRANSITIONS};
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
    /// Output format constraints (None = keep original, no conversion)
    pub output_constraints: Option<OutputConstraints>,
}

/// Query context in request body
#[derive(Debug, Deserialize)]
pub struct QueryContextBody {
    /// Structured tags for categorization
    pub tags: Vec<String>,
    /// Freeform description for matching
    pub description: String,
    /// Expected content (tracks, episodes, etc.)
    #[serde(default)]
    pub expected: Option<ExpectedContent>,
    /// Reference to external catalog (MusicBrainz, TMDB)
    #[serde(default)]
    pub catalog_reference: Option<CatalogReference>,
    /// Search constraints (preferred formats, quality, etc.)
    #[serde(default)]
    pub search_constraints: Option<SearchConstraints>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_constraints: Option<OutputConstraints>,
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
            output_constraints: ticket.output_constraints,
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
    AuthUser(user_id): AuthUser,
    Json(body): Json<CreateTicketBody>,
) -> Result<(StatusCode, Json<TicketResponse>), impl IntoResponse> {
    // Build query context with optional catalog fields
    let mut query_context =
        QueryContext::new(body.query_context.tags.clone(), &body.query_context.description);
    if let Some(expected) = body.query_context.expected {
        query_context = query_context.with_expected(expected);
    }
    if let Some(catalog_ref) = body.query_context.catalog_reference {
        query_context = query_context.with_catalog_reference(catalog_ref);
    }
    if let Some(constraints) = body.query_context.search_constraints {
        query_context = query_context.with_search_constraints(constraints);
    }

    let request = CreateTicketRequest {
        created_by: user_id,
        priority: body.priority.unwrap_or(0),
        query_context,
        dest_path: body.dest_path.clone(),
        output_constraints: body.output_constraints,
    };

    match state.ticket_store().create(request) {
        Ok(ticket) => {
            // Track metrics
            TICKETS_CREATED_TOTAL.inc();

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

            // Broadcast WebSocket update
            state
                .ws_broadcaster()
                .ticket_updated(&ticket.id, ticket.state.state_type());

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
    AuthUser(user_id): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<CancelTicketBody>>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    let reason = body.and_then(|b| b.reason.clone());
    let cancelled_by = user_id;

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
            // Track state transition
            TICKET_STATE_TRANSITIONS
                .with_label_values(&[&previous_state, "cancelled"])
                .inc();

            // Emit audit event
            state
                .audit()
                .try_emit(AuditEvent::TicketCancelled {
                    ticket_id: ticket.id.clone(),
                    cancelled_by,
                    reason,
                    previous_state,
                });

            // Broadcast WebSocket update
            state
                .ws_broadcaster()
                .ticket_updated(&ticket.id, ticket.state.state_type());

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

/// Query parameters for hard delete
#[derive(Debug, Deserialize)]
pub struct DeleteTicketParams {
    /// Confirmation flag - must be "true" to actually delete
    pub confirm: Option<String>,
}

/// Permanently delete a ticket (hard delete)
pub async fn delete_ticket(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<String>,
    Query(params): Query<DeleteTicketParams>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    // Require explicit confirmation
    if params.confirm.as_deref() != Some("true") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(TicketErrorResponse {
                error: "Hard delete requires confirmation. Add ?confirm=true to permanently delete this ticket.".to_string(),
            }),
        ));
    }

    let deleted_by = user_id;

    match state.ticket_store().delete(&id) {
        Ok(ticket) => {
            // Emit audit event
            state
                .audit()
                .try_emit(AuditEvent::TicketDeleted {
                    ticket_id: ticket.id.clone(),
                    deleted_by,
                    previous_state: ticket.state.state_type().to_string(),
                });

            // Broadcast WebSocket delete notification
            state.ws_broadcaster().ticket_deleted(&ticket.id);

            Ok(Json(TicketResponse::from(ticket)))
        }
        Err(TicketError::NotFound(_)) => Err((
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

/// Approve a ticket (for tickets in NeedsApproval state)
pub async fn approve_ticket(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<ApproveTicketBody>>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    let candidate_idx = body.and_then(|b| b.candidate_idx).unwrap_or(0);
    let approved_by = user_id;

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
        torrent_url: None, // Will be populated from catalog if available
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
                torrent_url: None,
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
            // Track state transition
            TICKET_STATE_TRANSITIONS
                .with_label_values(&[&previous_state, "approved"])
                .inc();

            // Emit audit event
            state.audit().try_emit(AuditEvent::TicketStateChanged {
                ticket_id: ticket.id.clone(),
                from_state: previous_state,
                to_state: "approved".to_string(),
                reason: Some(format!("Approved candidate {} by {}", candidate_idx, approved_by)),
            });

            // Broadcast WebSocket update
            state
                .ws_broadcaster()
                .ticket_updated(&ticket.id, ticket.state.state_type());

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

/// Retry a failed ticket (resets to Pending state)
pub async fn retry_ticket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
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

    // Check that ticket is in a retryable state
    // Manual retry allows any Failed state, not just retryable: true
    let can_retry = matches!(
        &current_ticket.state,
        TicketState::Failed { .. }
            | TicketState::AcquisitionFailed { .. }
            | TicketState::Rejected { .. }
            | TicketState::Cancelled { .. }
    );

    if !can_retry {
        return Err((
            StatusCode::CONFLICT,
            Json(TicketErrorResponse {
                error: format!(
                    "Cannot retry ticket: current state is {}",
                    current_ticket.state.state_type()
                ),
            }),
        ));
    }

    let previous_state = current_ticket.state.state_type().to_string();

    // Reset to Pending state
    match state.ticket_store().update_state(&id, TicketState::Pending) {
        Ok(ticket) => {
            // Track state transition
            TICKET_STATE_TRANSITIONS
                .with_label_values(&[&previous_state, "pending"])
                .inc();

            // Emit audit event
            state.audit().try_emit(AuditEvent::TicketStateChanged {
                ticket_id: ticket.id.clone(),
                from_state: previous_state,
                to_state: "pending".to_string(),
                reason: Some("Manual retry".to_string()),
            });

            // Broadcast WebSocket update
            state
                .ws_broadcaster()
                .ticket_updated(&ticket.id, ticket.state.state_type());

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
    AuthUser(user_id): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<RejectTicketBody>>,
) -> Result<Json<TicketResponse>, impl IntoResponse> {
    let reason = body.and_then(|b| b.reason.clone());
    let rejected_by = user_id;

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
            // Track state transition and failure
            TICKET_STATE_TRANSITIONS
                .with_label_values(&[&previous_state, "rejected"])
                .inc();
            TICKETS_FAILED_TOTAL.inc();

            // Emit audit event
            state.audit().try_emit(AuditEvent::TicketStateChanged {
                ticket_id: ticket.id.clone(),
                from_state: previous_state,
                to_state: "rejected".to_string(),
                reason,
            });

            // Broadcast WebSocket update
            state
                .ws_broadcaster()
                .ticket_updated(&ticket.id, ticket.state.state_type());

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
