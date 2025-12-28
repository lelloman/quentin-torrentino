//! Types for the processor module.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::converter::ConversionConstraints;
use crate::textbrain::FileMapping;

/// Status of a processing pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    /// Pool name (e.g., "conversion", "placement").
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

/// Overall pipeline status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    /// Whether the pipeline is running.
    pub running: bool,
    /// Status of the conversion pool.
    pub conversion_pool: PoolStatus,
    /// Status of the placement pool.
    pub placement_pool: PoolStatus,
    /// Tickets currently in conversion phase.
    pub converting_tickets: Vec<String>,
    /// Tickets currently in placement phase.
    pub placing_tickets: Vec<String>,
}

/// A job to process a ticket through the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineJob {
    /// Ticket ID.
    pub ticket_id: String,
    /// Source files to convert (from torrent download).
    pub source_files: Vec<SourceFile>,
    /// File mappings from TextBrain.
    pub file_mappings: Vec<FileMapping>,
    /// Conversion constraints.
    pub constraints: ConversionConstraints,
    /// Destination directory.
    pub dest_dir: PathBuf,
    /// Metadata to embed (optional).
    pub metadata: Option<PipelineMetadata>,
}

/// A source file to convert.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Path to the source file.
    pub path: PathBuf,
    /// Item ID from ticket (track ID, episode ID, etc.).
    pub item_id: String,
    /// Destination filename (without directory).
    pub dest_filename: String,
}

/// Metadata to embed during conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetadata {
    /// Title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Artist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Album.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// Album artist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    /// Year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Track number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u16>,
    /// Total tracks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_total: Option<u16>,
    /// Disc number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u16>,
    /// Total discs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_total: Option<u16>,
    /// Genre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// Comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Cover art path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<PathBuf>,
}

/// Result of processing a ticket through the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// Ticket ID.
    pub ticket_id: String,
    /// Whether processing was successful.
    pub success: bool,
    /// Files that were placed.
    pub files_placed: Vec<PlacedFileInfo>,
    /// Conversion duration in milliseconds.
    pub conversion_duration_ms: u64,
    /// Placement duration in milliseconds.
    pub placement_duration_ms: u64,
    /// Total bytes output.
    pub total_bytes: u64,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Information about a placed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedFileInfo {
    /// Item ID.
    pub item_id: String,
    /// Final path.
    pub path: PathBuf,
    /// Size in bytes.
    pub size_bytes: u64,
}

/// Progress update for pipeline processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PipelineProgress {
    /// Converting files.
    Converting {
        ticket_id: String,
        current_file: usize,
        total_files: usize,
        current_file_name: String,
        percent: f32,
    },
    /// Placing files.
    Placing {
        ticket_id: String,
        current_file: usize,
        total_files: usize,
        current_file_name: String,
        bytes_placed: u64,
    },
    /// Completed.
    Completed {
        ticket_id: String,
        files_placed: usize,
        total_bytes: u64,
    },
    /// Failed.
    Failed {
        ticket_id: String,
        error: String,
        failed_phase: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_status() {
        let status = PoolStatus {
            name: "conversion".to_string(),
            active_jobs: 2,
            max_concurrent: 4,
            queued_jobs: 5,
            total_processed: 100,
            total_failed: 3,
        };

        assert_eq!(status.name, "conversion");
        assert!(status.active_jobs < status.max_concurrent);
    }

    #[test]
    fn test_pipeline_progress_serialization() {
        let progress = PipelineProgress::Converting {
            ticket_id: "t-1".to_string(),
            current_file: 1,
            total_files: 10,
            current_file_name: "track01.flac".to_string(),
            percent: 45.5,
        };

        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"status\":\"converting\""));
        assert!(json.contains("\"ticket_id\":\"t-1\""));
    }
}
