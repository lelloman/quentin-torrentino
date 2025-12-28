//! Pipeline API endpoints for Phase 4.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

/// Response for pipeline status endpoint.
#[derive(Debug, Serialize)]
pub struct PipelineStatusResponse {
    /// Whether the pipeline is available.
    pub available: bool,
    /// Status message.
    pub message: String,
    /// Conversion pool status (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion_pool: Option<PoolStatusResponse>,
    /// Placement pool status (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement_pool: Option<PoolStatusResponse>,
    /// Tickets currently being converted.
    pub converting_tickets: Vec<String>,
    /// Tickets currently being placed.
    pub placing_tickets: Vec<String>,
}

/// Pool status in response.
#[derive(Debug, Serialize)]
pub struct PoolStatusResponse {
    /// Pool name.
    pub name: String,
    /// Number of active jobs.
    pub active_jobs: usize,
    /// Maximum concurrent jobs.
    pub max_concurrent: usize,
    /// Number of queued jobs.
    pub queued_jobs: usize,
    /// Total jobs processed since startup.
    pub total_processed: u64,
    /// Total jobs failed since startup.
    pub total_failed: u64,
}

/// Converter capabilities response.
#[derive(Debug, Serialize)]
pub struct ConverterInfoResponse {
    /// Whether converter is available.
    pub available: bool,
    /// Converter name.
    pub name: String,
    /// Supported input formats.
    pub supported_input_formats: Vec<String>,
    /// Supported output formats.
    pub supported_output_formats: Vec<String>,
    /// Configuration.
    pub config: ConverterConfigResponse,
}

/// Converter configuration in response.
#[derive(Debug, Serialize)]
pub struct ConverterConfigResponse {
    /// Max parallel conversions.
    pub max_parallel_conversions: usize,
    /// Timeout in seconds.
    pub timeout_secs: u64,
    /// Temp directory.
    pub temp_dir: String,
}

/// Placer capabilities response.
#[derive(Debug, Serialize)]
pub struct PlacerInfoResponse {
    /// Whether placer is available.
    pub available: bool,
    /// Placer name.
    pub name: String,
    /// Configuration.
    pub config: PlacerConfigResponse,
}

/// Placer configuration in response.
#[derive(Debug, Serialize)]
pub struct PlacerConfigResponse {
    /// Whether atomic moves are preferred.
    pub prefer_atomic_moves: bool,
    /// Whether checksums are verified.
    pub verify_checksums: bool,
    /// Max parallel operations.
    pub max_parallel_operations: usize,
}

/// Get pipeline status.
///
/// Returns the current status of the processing pipeline including
/// conversion and placement pool statistics.
pub async fn get_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    // Pipeline is not yet wired in AppState - return placeholder status
    let response = PipelineStatusResponse {
        available: false,
        message: "Pipeline not yet initialized. Phase 4 components are ready but not wired to server state.".to_string(),
        conversion_pool: None,
        placement_pool: None,
        converting_tickets: vec![],
        placing_tickets: vec![],
    };

    Json(response)
}

/// Get converter information.
///
/// Returns information about the configured converter including
/// supported formats and configuration.
pub async fn get_converter_info(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    // Return information about ffmpeg converter capabilities
    let response = ConverterInfoResponse {
        available: true,
        name: "ffmpeg".to_string(),
        supported_input_formats: vec![
            "flac".to_string(),
            "mp3".to_string(),
            "m4a".to_string(),
            "aac".to_string(),
            "ogg".to_string(),
            "opus".to_string(),
            "wav".to_string(),
            "wma".to_string(),
            "ape".to_string(),
            "mkv".to_string(),
            "mp4".to_string(),
            "avi".to_string(),
            "mov".to_string(),
            "webm".to_string(),
        ],
        supported_output_formats: vec![
            "flac".to_string(),
            "mp3".to_string(),
            "m4a".to_string(),
            "ogg".to_string(),
            "opus".to_string(),
            "wav".to_string(),
            "mkv".to_string(),
            "mp4".to_string(),
            "webm".to_string(),
        ],
        config: ConverterConfigResponse {
            max_parallel_conversions: 4,
            timeout_secs: 3600,
            temp_dir: std::env::temp_dir()
                .join("quentin-converter")
                .to_string_lossy()
                .to_string(),
        },
    };

    Json(response)
}

/// Get placer information.
///
/// Returns information about the configured placer including
/// configuration options.
pub async fn get_placer_info(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let response = PlacerInfoResponse {
        available: true,
        name: "fs".to_string(),
        config: PlacerConfigResponse {
            prefer_atomic_moves: true,
            verify_checksums: false,
            max_parallel_operations: 8,
        },
    };

    Json(response)
}

/// Validate ffmpeg availability.
///
/// Checks if ffmpeg and ffprobe are available on the system.
pub async fn validate_ffmpeg(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    use tokio::process::Command;

    let ffmpeg_check = Command::new("ffmpeg").arg("-version").output().await;
    let ffprobe_check = Command::new("ffprobe").arg("-version").output().await;

    let ffmpeg_available = ffmpeg_check.map(|o| o.status.success()).unwrap_or(false);
    let ffprobe_available = ffprobe_check.map(|o| o.status.success()).unwrap_or(false);

    #[derive(Serialize)]
    struct ValidationResponse {
        valid: bool,
        ffmpeg_available: bool,
        ffprobe_available: bool,
        message: String,
    }

    let message = if ffmpeg_available && ffprobe_available {
        "FFmpeg and FFprobe are available".to_string()
    } else if !ffmpeg_available && !ffprobe_available {
        "Neither FFmpeg nor FFprobe found".to_string()
    } else if !ffmpeg_available {
        "FFmpeg not found".to_string()
    } else {
        "FFprobe not found".to_string()
    };

    let response = ValidationResponse {
        valid: ffmpeg_available && ffprobe_available,
        ffmpeg_available,
        ffprobe_available,
        message,
    };

    if response.valid {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}
