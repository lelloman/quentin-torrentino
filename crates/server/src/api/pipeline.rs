//! Pipeline API endpoints for Phase 4.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use torrentino_core::{
    AudioConstraints, AudioFormat, ConversionConstraints, PipelineJob, SourceFile, TicketState,
};

use crate::state::AppState;

/// Response for pipeline status endpoint.
#[derive(Debug, Serialize)]
pub struct PipelineStatusResponse {
    /// Whether the pipeline is available.
    pub available: bool,
    /// Whether the pipeline is running.
    pub running: bool,
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

/// Request to process a ticket through the pipeline.
#[derive(Debug, Deserialize)]
pub struct ProcessTicketRequest {
    /// Source files to process (paths from downloaded torrent).
    pub source_files: Vec<SourceFileRequest>,
    /// Destination directory for final files.
    pub dest_dir: String,
    /// Output format (default: ogg_vorbis).
    #[serde(default = "default_output_format")]
    pub output_format: String,
    /// Output bitrate in kbps (default: 320).
    #[serde(default = "default_bitrate")]
    pub bitrate_kbps: u32,
}

fn default_output_format() -> String {
    "ogg_vorbis".to_string()
}

fn default_bitrate() -> u32 {
    320
}

/// Source file in request.
#[derive(Debug, Deserialize)]
pub struct SourceFileRequest {
    /// Path to source file.
    pub path: String,
    /// Item ID (e.g., track ID).
    pub item_id: String,
    /// Destination filename.
    pub dest_filename: String,
}

/// Response for process ticket endpoint.
#[derive(Debug, Serialize)]
pub struct ProcessTicketResponse {
    /// Whether the job was submitted successfully.
    pub success: bool,
    /// Message.
    pub message: String,
    /// Ticket ID.
    pub ticket_id: String,
}

/// Get pipeline status.
///
/// Returns the current status of the processing pipeline including
/// conversion and placement pool statistics.
pub async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.pipeline() {
        Some(pipeline) => {
            let status = pipeline.status().await;
            let response = PipelineStatusResponse {
                available: true,
                running: status.running,
                message: if status.running {
                    "Pipeline is running".to_string()
                } else {
                    "Pipeline is stopped".to_string()
                },
                conversion_pool: Some(PoolStatusResponse {
                    name: status.conversion_pool.name,
                    active_jobs: status.conversion_pool.active_jobs,
                    max_concurrent: status.conversion_pool.max_concurrent,
                    queued_jobs: status.conversion_pool.queued_jobs,
                    total_processed: status.conversion_pool.total_processed,
                    total_failed: status.conversion_pool.total_failed,
                }),
                placement_pool: Some(PoolStatusResponse {
                    name: status.placement_pool.name,
                    active_jobs: status.placement_pool.active_jobs,
                    max_concurrent: status.placement_pool.max_concurrent,
                    queued_jobs: status.placement_pool.queued_jobs,
                    total_processed: status.placement_pool.total_processed,
                    total_failed: status.placement_pool.total_failed,
                }),
                converting_tickets: status.converting_tickets,
                placing_tickets: status.placing_tickets,
            };
            Json(response)
        }
        None => {
            let response = PipelineStatusResponse {
                available: false,
                running: false,
                message: "Pipeline not initialized".to_string(),
                conversion_pool: None,
                placement_pool: None,
                converting_tickets: vec![],
                placing_tickets: vec![],
            };
            Json(response)
        }
    }
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

/// Process a ticket through the conversion and placement pipeline.
///
/// This endpoint submits a ticket for processing. The ticket must be in an
/// appropriate state (e.g., downloaded, ready for conversion).
pub async fn process_ticket(
    State(state): State<Arc<AppState>>,
    Path(ticket_id): Path<String>,
    Json(request): Json<ProcessTicketRequest>,
) -> impl IntoResponse {
    // Check if pipeline is available
    let pipeline = match state.pipeline() {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ProcessTicketResponse {
                    success: false,
                    message: "Pipeline not initialized".to_string(),
                    ticket_id,
                }),
            );
        }
    };

    // Verify ticket exists
    let ticket_store = state.ticket_store();
    match ticket_store.get(&ticket_id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ProcessTicketResponse {
                    success: false,
                    message: format!("Ticket not found: {}", ticket_id),
                    ticket_id,
                }),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProcessTicketResponse {
                    success: false,
                    message: format!("Failed to get ticket: {}", e),
                    ticket_id,
                }),
            );
        }
    };

    // Parse output format
    let audio_format = match request.output_format.as_str() {
        "ogg_vorbis" | "ogg" => AudioFormat::OggVorbis,
        "mp3" => AudioFormat::Mp3,
        "flac" => AudioFormat::Flac,
        "opus" => AudioFormat::Opus,
        "aac" | "m4a" => AudioFormat::Aac,
        "wav" => AudioFormat::Wav,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProcessTicketResponse {
                    success: false,
                    message: format!("Unsupported output format: {}", other),
                    ticket_id,
                }),
            );
        }
    };

    // Build source files
    let source_files: Vec<SourceFile> = request
        .source_files
        .into_iter()
        .map(|f| SourceFile {
            path: PathBuf::from(f.path),
            item_id: f.item_id,
            dest_filename: f.dest_filename,
        })
        .collect();

    // Build pipeline job
    let job = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files,
        file_mappings: vec![], // Will be populated from TextBrain results
        constraints: Some(ConversionConstraints::Audio(AudioConstraints {
            format: audio_format,
            bitrate_kbps: Some(request.bitrate_kbps),
            sample_rate_hz: None,
            channels: None,
            compression_level: None,
        })),
        dest_dir: PathBuf::from(request.dest_dir),
        metadata: None, // TODO: Extract from ticket
    };

    // Submit job to pipeline
    // The pipeline processor handles state transitions internally
    match pipeline.process(job, None).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(ProcessTicketResponse {
                success: true,
                message: "Job submitted to pipeline".to_string(),
                ticket_id,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ProcessTicketResponse {
                success: false,
                message: format!("Failed to submit job: {}", e),
                ticket_id,
            }),
        ),
    }
}

/// Progress response for a ticket.
#[derive(Debug, Serialize)]
pub struct TicketProgressResponse {
    /// Ticket ID.
    pub ticket_id: String,
    /// Current phase.
    pub phase: String,
    /// Progress details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<ProgressDetails>,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Progress details.
#[derive(Debug, Serialize)]
pub struct ProgressDetails {
    /// Current file index.
    pub current_file: usize,
    /// Total files.
    pub total_files: usize,
    /// Current file name.
    pub current_file_name: String,
    /// Percent complete.
    pub percent: f32,
}

/// Get progress for a ticket being processed.
pub async fn get_progress(
    State(state): State<Arc<AppState>>,
    Path(ticket_id): Path<String>,
) -> impl IntoResponse {
    // Get ticket state
    let ticket_store = state.ticket_store();
    let ticket = match ticket_store.get(&ticket_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(TicketProgressResponse {
                    ticket_id,
                    phase: "unknown".to_string(),
                    progress: None,
                    error: Some("Ticket not found".to_string()),
                }),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketProgressResponse {
                    ticket_id,
                    phase: "error".to_string(),
                    progress: None,
                    error: Some(format!("Failed to get ticket: {}", e)),
                }),
            );
        }
    };

    // Map ticket state to progress response
    let response = match &ticket.state {
        TicketState::Converting {
            current_idx,
            total,
            current_name,
            ..
        } => TicketProgressResponse {
            ticket_id,
            phase: "converting".to_string(),
            progress: Some(ProgressDetails {
                current_file: *current_idx,
                total_files: *total,
                current_file_name: current_name.clone(),
                percent: if *total > 0 {
                    (*current_idx as f32 / *total as f32) * 100.0
                } else {
                    0.0
                },
            }),
            error: None,
        },
        TicketState::Placing {
            files_placed,
            total_files,
            ..
        } => TicketProgressResponse {
            ticket_id,
            phase: "placing".to_string(),
            progress: Some(ProgressDetails {
                current_file: *files_placed,
                total_files: *total_files,
                current_file_name: "".to_string(),
                percent: if *total_files > 0 {
                    (*files_placed as f32 / *total_files as f32) * 100.0
                } else {
                    0.0
                },
            }),
            error: None,
        },
        TicketState::Completed { .. } => TicketProgressResponse {
            ticket_id,
            phase: "completed".to_string(),
            progress: Some(ProgressDetails {
                current_file: 0,
                total_files: 0,
                current_file_name: "".to_string(),
                percent: 100.0,
            }),
            error: None,
        },
        TicketState::Failed { error, .. } => TicketProgressResponse {
            ticket_id,
            phase: "failed".to_string(),
            progress: None,
            error: Some(error.clone()),
        },
        other => TicketProgressResponse {
            ticket_id,
            phase: format!("{:?}", other).to_lowercase(),
            progress: None,
            error: None,
        },
    };

    (StatusCode::OK, Json(response))
}
