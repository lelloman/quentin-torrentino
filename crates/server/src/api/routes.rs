use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

use super::{audit, catalog, handlers, searcher, textbrain, tickets, torrents};
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
        // Search (read-only, indexers configured in Jackett)
        .route("/search", post(searcher::search))
        .route("/searcher/status", get(searcher::get_status))
        .route("/searcher/indexers", get(searcher::list_indexers))
        // Torrent client
        .route("/torrents/status", get(torrents::get_status))
        .route("/torrents", get(torrents::list_torrents))
        .route("/torrents/{hash}", get(torrents::get_torrent))
        .route("/torrents/{hash}", delete(torrents::remove_torrent))
        .route("/torrents/add/magnet", post(torrents::add_magnet))
        .route("/torrents/add/file", post(torrents::add_file))
        .route("/torrents/add/url", post(torrents::add_from_url))
        .route("/torrents/{hash}/pause", post(torrents::pause_torrent))
        .route("/torrents/{hash}/resume", post(torrents::resume_torrent))
        .route("/torrents/{hash}/upload-limit", post(torrents::set_upload_limit))
        .route("/torrents/{hash}/download-limit", post(torrents::set_download_limit))
        .route("/torrents/{hash}/recheck", post(torrents::recheck_torrent))
        // Catalog (search result cache)
        .route("/catalog", get(catalog::list_catalog))
        .route("/catalog", delete(catalog::clear_catalog))
        .route("/catalog/stats", get(catalog::get_stats))
        .route("/catalog/{hash}", get(catalog::get_entry))
        .route("/catalog/{hash}", delete(catalog::remove_entry))
        // TextBrain (LLM experimentation)
        .route("/textbrain/complete", post(textbrain::complete))
        .route("/textbrain/process/{ticket_id}", post(textbrain::process_ticket))
        .with_state(state);

    // Serve dashboard with SPA fallback
    let index_path = format!("{}/index.html", dashboard_dir);
    let serve_dir = ServeDir::new(&dashboard_dir).fallback(ServeFile::new(&index_path));

    Router::new()
        .nest("/api/v1", api_routes)
        .fallback_service(serve_dir)
}
