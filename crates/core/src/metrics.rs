//! Prometheus metrics for core components.
//!
//! This module provides metrics for:
//! - Orchestrator (acquisition, downloads, failovers, retries)
//! - Pipeline (conversions, placements)
//! - External services (Jackett, torrent client, LLM)

use once_cell::sync::Lazy;
use prometheus::{HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Opts};

// =============================================================================
// Orchestrator - Acquisition Metrics
// =============================================================================

/// Acquisition attempts total by result.
pub static ACQUISITION_ATTEMPTS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new(
            "quentin_acquisition_attempts_total",
            "Total acquisition attempts",
        ),
        &["result"], // "auto_approved", "needs_approval", "failed"
    )
    .unwrap()
});

/// Acquisition duration in seconds.
pub static ACQUISITION_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_acquisition_duration_seconds",
            "Duration of acquisition phase",
        )
        .buckets(vec![0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]),
        &["result"],
    )
    .unwrap()
});

/// Queries generated per acquisition.
pub static QUERIES_GENERATED: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_queries_generated",
            "Number of search queries generated per acquisition",
        )
        .buckets(vec![1.0, 2.0, 3.0, 5.0, 10.0, 20.0]),
        &[],
    )
    .unwrap()
});

/// Candidates found per acquisition.
pub static CANDIDATES_FOUND: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_candidates_found",
            "Number of candidates found per acquisition",
        )
        .buckets(vec![0.0, 1.0, 5.0, 10.0, 25.0, 50.0, 100.0]),
        &[],
    )
    .unwrap()
});

/// Best match confidence scores.
pub static MATCH_CONFIDENCE: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_match_confidence",
            "Distribution of best match confidence scores",
        )
        .buckets(vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0]),
        &[],
    )
    .unwrap()
});

// =============================================================================
// Orchestrator - Download Metrics
// =============================================================================

/// Downloads started total.
pub static DOWNLOADS_STARTED: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new("quentin_downloads_started_total", "Total downloads started").unwrap()
});

/// Downloads completed total.
pub static DOWNLOADS_COMPLETED: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_downloads_completed_total",
        "Total downloads completed successfully",
    )
    .unwrap()
});

/// Downloads failed total.
pub static DOWNLOADS_FAILED: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_downloads_failed_total",
        "Total downloads that failed",
    )
    .unwrap()
});

/// Download duration in seconds.
pub static DOWNLOAD_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new("quentin_download_duration_seconds", "Duration of downloads").buckets(
            vec![
                30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 14400.0,
            ],
        ),
        &["result"], // "success", "failed"
    )
    .unwrap()
});

/// Stall detections total.
pub static STALL_DETECTIONS: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_stall_detections_total",
        "Total download stall detections",
    )
    .unwrap()
});

/// Failover attempts total.
pub static FAILOVER_ATTEMPTS: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_failover_attempts_total",
        "Total download failover attempts",
    )
    .unwrap()
});

/// Retry attempts total by phase.
pub static RETRY_ATTEMPTS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("quentin_retry_attempts_total", "Total retry attempts"),
        &["phase"], // "acquisition", "download", "conversion", "placement"
    )
    .unwrap()
});

// =============================================================================
// Pipeline Metrics
// =============================================================================

/// Conversions total by result.
pub static CONVERSIONS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("quentin_conversions_total", "Total file conversions"),
        &["result"], // "success", "failed", "skipped"
    )
    .unwrap()
});

/// Conversion duration in seconds.
pub static CONVERSION_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_conversion_duration_seconds",
            "Duration of file conversions",
        )
        .buckets(vec![
            1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
        ]),
        &[],
    )
    .unwrap()
});

/// Placements total by result.
pub static PLACEMENTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("quentin_placements_total", "Total file placements"),
        &["result"], // "success", "failed", "rollback"
    )
    .unwrap()
});

/// Files placed total.
pub static FILES_PLACED: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_files_placed_total",
        "Total files placed to destination",
    )
    .unwrap()
});

/// Tickets completed (reached Completed state).
pub static TICKETS_COMPLETED: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "quentin_tickets_completed_total",
        "Total tickets completed successfully",
    )
    .unwrap()
});

// =============================================================================
// External Service Metrics
// =============================================================================

/// External service request duration.
pub static EXTERNAL_SERVICE_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_external_service_duration_seconds",
            "Duration of external service calls",
        )
        .buckets(vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]),
        &["service", "operation"],
    )
    .unwrap()
});

/// External service requests total.
pub static EXTERNAL_SERVICE_REQUESTS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new(
            "quentin_external_service_requests_total",
            "Total external service requests",
        ),
        &["service", "operation", "status"], // status: "success", "error"
    )
    .unwrap()
});

/// Search results returned from indexers.
pub static SEARCH_RESULTS: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "quentin_search_results",
            "Number of search results returned per query",
        )
        .buckets(vec![0.0, 1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0]),
        &[],
    )
    .unwrap()
});

/// LLM tokens used.
pub static LLM_TOKENS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("quentin_llm_tokens_total", "Total LLM tokens used"),
        &["provider", "direction"], // direction: "input", "output"
    )
    .unwrap()
});

// =============================================================================
// Helper functions
// =============================================================================

/// Get all core metrics for registration in a registry.
pub fn all_metrics() -> Vec<Box<dyn prometheus::core::Collector>> {
    vec![
        // Acquisition
        Box::new(ACQUISITION_ATTEMPTS.clone()),
        Box::new(ACQUISITION_DURATION.clone()),
        Box::new(QUERIES_GENERATED.clone()),
        Box::new(CANDIDATES_FOUND.clone()),
        Box::new(MATCH_CONFIDENCE.clone()),
        // Downloads
        Box::new(DOWNLOADS_STARTED.clone()),
        Box::new(DOWNLOADS_COMPLETED.clone()),
        Box::new(DOWNLOADS_FAILED.clone()),
        Box::new(DOWNLOAD_DURATION.clone()),
        Box::new(STALL_DETECTIONS.clone()),
        Box::new(FAILOVER_ATTEMPTS.clone()),
        Box::new(RETRY_ATTEMPTS.clone()),
        // Pipeline
        Box::new(CONVERSIONS_TOTAL.clone()),
        Box::new(CONVERSION_DURATION.clone()),
        Box::new(PLACEMENTS_TOTAL.clone()),
        Box::new(FILES_PLACED.clone()),
        Box::new(TICKETS_COMPLETED.clone()),
        // External services
        Box::new(EXTERNAL_SERVICE_DURATION.clone()),
        Box::new(EXTERNAL_SERVICE_REQUESTS.clone()),
        Box::new(SEARCH_RESULTS.clone()),
        Box::new(LLM_TOKENS.clone()),
    ]
}
