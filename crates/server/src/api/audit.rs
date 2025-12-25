use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use torrentino_core::{AuditFilter, AuditRecord};

use crate::state::AppState;

/// Maximum allowed limit for audit queries
const MAX_LIMIT: i64 = 1000;

/// Default limit for audit queries
const DEFAULT_LIMIT: i64 = 100;

/// Query parameters for audit endpoint
#[derive(Debug, Deserialize)]
pub struct AuditQueryParams {
    /// Filter by ticket ID
    pub ticket_id: Option<String>,
    /// Filter by event type
    pub event_type: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter events after this timestamp (ISO 8601)
    pub from: Option<DateTime<Utc>>,
    /// Filter events before this timestamp (ISO 8601)
    pub to: Option<DateTime<Utc>>,
    /// Maximum number of events to return (default 100, max 1000)
    pub limit: Option<i64>,
    /// Pagination offset (default 0)
    pub offset: Option<i64>,
}

/// Response for audit query endpoint
#[derive(Debug, Serialize)]
pub struct AuditQueryResponse {
    /// List of audit events
    pub events: Vec<AuditRecord>,
    /// Total number of matching events
    pub total: i64,
    /// Limit used for this query
    pub limit: i64,
    /// Offset used for this query
    pub offset: i64,
}

/// Error response for audit queries
#[derive(Debug, Serialize)]
pub struct AuditErrorResponse {
    pub error: String,
}

/// Query audit events
pub async fn query_audit(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditQueryParams>,
) -> Result<Json<AuditQueryResponse>, impl IntoResponse> {
    // Validate and cap limit
    let limit = params
        .limit
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    let offset = params.offset.unwrap_or(0).max(0);

    // Build base filter (shared between query and count)
    let mut base_filter = AuditFilter::new();

    if let Some(ref ticket_id) = params.ticket_id {
        base_filter = base_filter.with_ticket_id(ticket_id);
    }

    if let Some(ref event_type) = params.event_type {
        base_filter = base_filter.with_event_type(event_type);
    }

    if let Some(ref user_id) = params.user_id {
        base_filter = base_filter.with_user_id(user_id);
    }

    if params.from.is_some() || params.to.is_some() {
        base_filter = base_filter.with_time_range(params.from, params.to);
    }

    // Create query filter with pagination
    let query_filter = AuditFilter {
        limit,
        offset,
        ..base_filter.clone()
    };

    // Query events
    let events = match state.audit_store().query(&query_filter) {
        Ok(events) => events,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuditErrorResponse {
                    error: format!("Failed to query audit events: {}", e),
                }),
            ));
        }
    };

    // Get total count (without limit/offset) using the base filter
    let total = match state.audit_store().count(&base_filter) {
        Ok(count) => count,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuditErrorResponse {
                    error: format!("Failed to count audit events: {}", e),
                }),
            ));
        }
    };

    Ok(Json(AuditQueryResponse {
        events,
        total,
        limit,
        offset,
    }))
}
