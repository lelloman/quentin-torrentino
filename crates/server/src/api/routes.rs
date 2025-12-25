use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use super::{audit, handlers, tickets};
use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health and config
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/config", get(handlers::get_config))
        // Audit
        .route("/api/v1/audit", get(audit::query_audit))
        // Tickets
        .route("/api/v1/tickets", post(tickets::create_ticket))
        .route("/api/v1/tickets", get(tickets::list_tickets))
        .route("/api/v1/tickets/{id}", get(tickets::get_ticket))
        .route("/api/v1/tickets/{id}", delete(tickets::cancel_ticket))
        .with_state(state)
}
