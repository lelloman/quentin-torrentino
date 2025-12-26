use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

use super::{audit, handlers, searcher, tickets};
use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Dashboard static files path (configurable via env)
    let dashboard_dir =
        std::env::var("DASHBOARD_DIR").unwrap_or_else(|_| "crates/dashboard/dist".to_string());

    // API routes
    let api_routes = Router::new()
        // Health and config
        .route("/health", get(handlers::health))
        .route("/config", get(handlers::get_config))
        // Audit
        .route("/audit", get(audit::query_audit))
        // Tickets
        .route("/tickets", post(tickets::create_ticket))
        .route("/tickets", get(tickets::list_tickets))
        .route("/tickets/{id}", get(tickets::get_ticket))
        .route("/tickets/{id}", delete(tickets::cancel_ticket))
        // Search
        .route("/search", post(searcher::search))
        .route("/searcher/status", get(searcher::get_status))
        .route("/searcher/indexers", get(searcher::list_indexers))
        .route(
            "/searcher/indexers/{name}",
            patch(searcher::update_indexer),
        )
        .with_state(state);

    // Serve dashboard with SPA fallback
    let index_path = format!("{}/index.html", dashboard_dir);
    let serve_dir = ServeDir::new(&dashboard_dir).fallback(ServeFile::new(&index_path));

    Router::new()
        .nest("/api/v1", api_routes)
        .fallback_service(serve_dir)
}
