//! Prometheus metrics for observability.
//!
//! This module provides metrics for monitoring the Quentin Torrentino server:
//! - HTTP request metrics (latency, counts, errors)
//! - WebSocket connection metrics
//! - Ticket state transition metrics
//! - Orchestrator and pipeline status (collected dynamically)

use once_cell::sync::Lazy;
use prometheus::{
    self, Encoder, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    Opts, Registry, TextEncoder,
};

/// Global metrics registry.
pub static REGISTRY: Lazy<Registry> = Lazy::new(|| {
    let registry = Registry::new();
    register_metrics(&registry);
    registry
});

// =============================================================================
// HTTP Request Metrics
// =============================================================================

/// HTTP request duration in seconds.
pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_http_request_duration_seconds",
            "HTTP request duration in seconds",
        )
        .buckets(vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]),
        &["method", "path", "status"],
    )
    .unwrap()
});

/// HTTP requests total count.
pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("quentin_http_requests_total", "Total HTTP requests"),
        &["method", "path", "status"],
    )
    .unwrap()
});

/// HTTP requests currently in flight.
pub static HTTP_REQUESTS_IN_FLIGHT: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_http_requests_in_flight",
        "Number of HTTP requests currently being processed",
    )
    .unwrap()
});

/// Authentication failures.
pub static AUTH_FAILURES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new(
            "quentin_auth_failures_total",
            "Total authentication failures",
        ),
        &["reason"],
    )
    .unwrap()
});

// =============================================================================
// WebSocket Metrics
// =============================================================================

/// Active WebSocket connections.
pub static WS_CONNECTIONS_ACTIVE: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_ws_connections_active",
        "Number of active WebSocket connections",
    )
    .unwrap()
});

/// Total WebSocket connections (cumulative).
pub static WS_CONNECTIONS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_ws_connections_total",
        "Total WebSocket connections since startup",
    )
    .unwrap()
});

/// WebSocket messages sent by type.
pub static WS_MESSAGES_SENT: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("quentin_ws_messages_sent_total", "WebSocket messages sent"),
        &["type"],
    )
    .unwrap()
});

/// WebSocket lag events (when client falls behind).
pub static WS_LAG_EVENTS: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_ws_lag_events_total",
        "WebSocket lag events (client fell behind)",
    )
    .unwrap()
});

// =============================================================================
// Ticket Metrics
// =============================================================================

/// Tickets by current state (collected dynamically).
pub static TICKETS_BY_STATE: Lazy<IntGaugeVec> = Lazy::new(|| {
    IntGaugeVec::new(
        Opts::new("quentin_tickets_by_state", "Current ticket count by state"),
        &["state"],
    )
    .unwrap()
});

/// Ticket state transitions.
pub static TICKET_STATE_TRANSITIONS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new(
            "quentin_ticket_state_transitions_total",
            "Ticket state transitions",
        ),
        &["from_state", "to_state"],
    )
    .unwrap()
});

/// Tickets created total.
pub static TICKETS_CREATED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_tickets_created_total",
        "Total tickets created since startup",
    )
    .unwrap()
});

/// Tickets failed total.
pub static TICKETS_FAILED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_tickets_failed_total",
        "Total tickets that failed (terminal)",
    )
    .unwrap()
});

// =============================================================================
// Orchestrator Metrics (collected dynamically)
// =============================================================================

/// Orchestrator running state (1 = running, 0 = stopped).
pub static ORCHESTRATOR_RUNNING: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_orchestrator_running",
        "Whether the orchestrator is running (1) or stopped (0)",
    )
    .unwrap()
});

/// Active downloads gauge (collected dynamically).
pub static DOWNLOADS_ACTIVE: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_downloads_active",
        "Number of currently active downloads",
    )
    .unwrap()
});

// =============================================================================
// Pipeline Metrics (collected dynamically)
// =============================================================================

/// Conversion pool active jobs.
pub static CONVERSION_POOL_ACTIVE: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_conversion_pool_active",
        "Number of active conversion jobs",
    )
    .unwrap()
});

/// Conversion pool queued jobs.
pub static CONVERSION_POOL_QUEUED: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_conversion_pool_queued",
        "Number of queued conversion jobs",
    )
    .unwrap()
});

/// Placement pool active jobs.
pub static PLACEMENT_POOL_ACTIVE: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_placement_pool_active",
        "Number of active placement jobs",
    )
    .unwrap()
});

/// Placement pool queued jobs.
pub static PLACEMENT_POOL_QUEUED: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_placement_pool_queued",
        "Number of queued placement jobs",
    )
    .unwrap()
});

// =============================================================================
// Catalog Metrics (collected dynamically)
// =============================================================================

/// Torrent catalog entries.
pub static CATALOG_ENTRIES: Lazy<IntGauge> = Lazy::new(|| {
    IntGauge::new(
        "quentin_catalog_entries",
        "Number of entries in the torrent catalog",
    )
    .unwrap()
});

// =============================================================================
// Registration
// =============================================================================

fn register_metrics(registry: &Registry) {
    // HTTP
    registry
        .register(Box::new(HTTP_REQUEST_DURATION.clone()))
        .unwrap();
    registry
        .register(Box::new(HTTP_REQUESTS_TOTAL.clone()))
        .unwrap();
    registry
        .register(Box::new(HTTP_REQUESTS_IN_FLIGHT.clone()))
        .unwrap();
    registry
        .register(Box::new(AUTH_FAILURES_TOTAL.clone()))
        .unwrap();

    // WebSocket
    registry
        .register(Box::new(WS_CONNECTIONS_ACTIVE.clone()))
        .unwrap();
    registry
        .register(Box::new(WS_CONNECTIONS_TOTAL.clone()))
        .unwrap();
    registry
        .register(Box::new(WS_MESSAGES_SENT.clone()))
        .unwrap();
    registry.register(Box::new(WS_LAG_EVENTS.clone())).unwrap();

    // Tickets
    registry
        .register(Box::new(TICKETS_BY_STATE.clone()))
        .unwrap();
    registry
        .register(Box::new(TICKET_STATE_TRANSITIONS.clone()))
        .unwrap();
    registry
        .register(Box::new(TICKETS_CREATED_TOTAL.clone()))
        .unwrap();
    registry
        .register(Box::new(TICKETS_FAILED_TOTAL.clone()))
        .unwrap();

    // Orchestrator
    registry
        .register(Box::new(ORCHESTRATOR_RUNNING.clone()))
        .unwrap();
    registry
        .register(Box::new(DOWNLOADS_ACTIVE.clone()))
        .unwrap();

    // Pipeline
    registry
        .register(Box::new(CONVERSION_POOL_ACTIVE.clone()))
        .unwrap();
    registry
        .register(Box::new(CONVERSION_POOL_QUEUED.clone()))
        .unwrap();
    registry
        .register(Box::new(PLACEMENT_POOL_ACTIVE.clone()))
        .unwrap();
    registry
        .register(Box::new(PLACEMENT_POOL_QUEUED.clone()))
        .unwrap();

    // Catalog
    registry
        .register(Box::new(CATALOG_ENTRIES.clone()))
        .unwrap();

    // Core metrics (orchestrator, pipeline, external services)
    for metric in torrentino_core::metrics::all_metrics() {
        registry.register(metric).unwrap();
    }
}

/// Encode all metrics as Prometheus text format.
pub fn encode_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// Collect dynamic metrics from current application state.
///
/// This is called before encoding metrics to update gauges with current values
/// from the orchestrator, pipeline, and other components.
pub async fn collect_dynamic_metrics(state: &crate::state::AppState) {
    // Update orchestrator metrics
    if let Some(orchestrator) = state.orchestrator() {
        let status = orchestrator.status().await;
        ORCHESTRATOR_RUNNING.set(if status.running { 1 } else { 0 });
        DOWNLOADS_ACTIVE.set(status.active_downloads as i64);
    }

    // Update pipeline metrics
    if let Some(pipeline) = state.pipeline() {
        let status = pipeline.status().await;
        CONVERSION_POOL_ACTIVE.set(status.conversion_pool.active_jobs as i64);
        CONVERSION_POOL_QUEUED.set(status.conversion_pool.queued_jobs as i64);
        PLACEMENT_POOL_ACTIVE.set(status.placement_pool.active_jobs as i64);
        PLACEMENT_POOL_QUEUED.set(status.placement_pool.queued_jobs as i64);
    }

    // Update catalog metrics
    if let Ok(stats) = state.catalog().stats() {
        CATALOG_ENTRIES.set(stats.total_torrents as i64);
    }

    // Update ticket counts by state
    let ticket_store = state.ticket_store();
    for state_type in [
        "pending",
        "acquiring",
        "acquisition_failed",
        "needs_approval",
        "auto_approved",
        "approved",
        "downloading",
        "converting",
        "placing",
        "completed",
        "pending_retry",
        "failed",
        "rejected",
        "cancelled",
    ] {
        let filter = torrentino_core::TicketFilter::new().with_state(state_type);
        if let Ok(count) = ticket_store.count(&filter) {
            TICKETS_BY_STATE.with_label_values(&[state_type]).set(count);
        }
    }
}

/// Normalize a path for metric labels (replace IDs with placeholders).
pub fn normalize_path(path: &str) -> String {
    // Replace UUIDs and hashes with placeholders
    let uuid_regex = regex_lite::Regex::new(
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    )
    .unwrap();
    let hash_regex = regex_lite::Regex::new(r"[0-9a-fA-F]{40}").unwrap();
    let numeric_regex = regex_lite::Regex::new(r"/\d+(/|$)").unwrap();

    let result = uuid_regex.replace_all(path, "{id}");
    let result = hash_regex.replace_all(&result, "{hash}");
    let result = numeric_regex.replace_all(&result, "/{id}$1");
    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_uuid() {
        let path = "/api/v1/tickets/550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(normalize_path(path), "/api/v1/tickets/{id}");
    }

    #[test]
    fn test_normalize_path_hash() {
        let path = "/api/v1/torrents/a94a8fe5ccb19ba61c4c0873d391e987982fbbd3";
        assert_eq!(normalize_path(path), "/api/v1/torrents/{hash}");
    }

    #[test]
    fn test_normalize_path_numeric() {
        let path = "/api/v1/external-catalog/tmdb/movies/12345";
        assert_eq!(
            normalize_path(path),
            "/api/v1/external-catalog/tmdb/movies/{id}"
        );
    }

    #[test]
    fn test_normalize_path_numeric_middle() {
        let path = "/api/v1/external-catalog/tmdb/tv/12345/season/2";
        assert_eq!(
            normalize_path(path),
            "/api/v1/external-catalog/tmdb/tv/{id}/season/{id}"
        );
    }

    #[test]
    fn test_normalize_path_no_ids() {
        let path = "/api/v1/health";
        assert_eq!(normalize_path(path), "/api/v1/health");
    }

    #[test]
    fn test_encode_metrics_returns_prometheus_format() {
        // Access metrics to ensure they're initialized
        HTTP_REQUESTS_TOTAL
            .with_label_values(&["GET", "/test", "200"])
            .inc();

        let output = encode_metrics();
        assert!(output.contains("quentin_http_requests_total"));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }

    #[test]
    fn test_registry_contains_all_metrics() {
        // Touch all metrics to ensure they appear in output
        // (Prometheus only outputs metrics that have been accessed)
        HTTP_REQUEST_DURATION
            .with_label_values(&["GET", "/test", "200"])
            .observe(0.1);
        HTTP_REQUESTS_IN_FLIGHT.set(0);
        WS_CONNECTIONS_ACTIVE.set(0);
        WS_CONNECTIONS_TOTAL.inc();
        TICKETS_BY_STATE.with_label_values(&["pending"]).set(0);
        TICKETS_CREATED_TOTAL.inc();
        ORCHESTRATOR_RUNNING.set(0);
        DOWNLOADS_ACTIVE.set(0);
        CONVERSION_POOL_ACTIVE.set(0);
        PLACEMENT_POOL_ACTIVE.set(0);

        let output = encode_metrics();

        // HTTP metrics
        assert!(output.contains("quentin_http_request_duration_seconds"));
        assert!(output.contains("quentin_http_requests_total"));
        assert!(output.contains("quentin_http_requests_in_flight"));

        // WebSocket metrics
        assert!(output.contains("quentin_ws_connections_active"));
        assert!(output.contains("quentin_ws_connections_total"));

        // Ticket metrics
        assert!(output.contains("quentin_tickets_by_state"));
        assert!(output.contains("quentin_tickets_created_total"));

        // Orchestrator metrics
        assert!(output.contains("quentin_orchestrator_running"));
        assert!(output.contains("quentin_downloads_active"));

        // Pipeline metrics
        assert!(output.contains("quentin_conversion_pool_active"));
        assert!(output.contains("quentin_placement_pool_active"));
    }
}
