use axum::{routing::get, Router};
use std::sync::Arc;

use super::{audit, handlers};
use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/config", get(handlers::get_config))
        .route("/api/v1/audit", get(audit::query_audit))
        .with_state(state)
}
