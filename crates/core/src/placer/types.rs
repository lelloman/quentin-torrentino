//! Types for the placer module.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A file placement job.
#[derive(Debug, Clone)]
pub struct PlacementJob {
    /// Unique job ID (usually ticket_id).
    pub job_id: String,
    /// Files to place.
    pub files: Vec<FilePlacement>,
    /// Whether to use atomic moves (rename) when possible.
    pub atomic: bool,
    /// Whether to clean up source files after successful placement.
    pub cleanup_sources: bool,
    /// Whether to create a rollback plan for failure recovery.
    pub enable_rollback: bool,
}

/// A single file placement request.
#[derive(Debug, Clone)]
pub struct FilePlacement {
    /// Item ID (for tracking).
    pub item_id: String,
    /// Source file path.
    pub source: PathBuf,
    /// Destination file path.
    pub destination: PathBuf,
    /// Whether to overwrite if destination exists.
    pub overwrite: bool,
    /// Verify checksum after copy (optional).
    pub verify_checksum: Option<ChecksumType>,
}

/// Type of checksum to verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumType {
    /// SHA-256 checksum.
    Sha256,
    /// MD5 checksum (faster but less secure).
    Md5,
}

/// Result of a successful placement job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementResult {
    /// Job ID.
    pub job_id: String,
    /// Files successfully placed.
    pub files_placed: Vec<PlacedFile>,
    /// Total bytes placed.
    pub total_bytes: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Information about a placed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedFile {
    /// Item ID.
    pub item_id: String,
    /// Final destination path.
    pub destination: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Checksum if verified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// Progress update during placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementProgress {
    /// Job ID.
    pub job_id: String,
    /// Number of files placed so far.
    pub files_placed: usize,
    /// Total files to place.
    pub total_files: usize,
    /// Current file being placed.
    pub current_file: String,
    /// Bytes copied so far.
    pub bytes_copied: u64,
    /// Total bytes to copy.
    pub total_bytes: u64,
}

/// Rollback information for recovering from partial failures.
#[derive(Debug, Clone)]
pub struct RollbackPlan {
    /// Job ID.
    pub job_id: String,
    /// Files that were successfully placed (to be rolled back).
    pub placed_files: Vec<RollbackFile>,
    /// Directories that were created (to be removed).
    pub created_directories: Vec<PathBuf>,
}

/// A file that was placed and may need rollback.
#[derive(Debug, Clone)]
pub struct RollbackFile {
    /// Destination where file was placed.
    pub destination: PathBuf,
    /// Original source (if still available).
    pub source: Option<PathBuf>,
    /// File size for verification.
    pub size_bytes: u64,
}

impl RollbackPlan {
    /// Creates a new empty rollback plan.
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            placed_files: Vec::new(),
            created_directories: Vec::new(),
        }
    }

    /// Records a placed file for potential rollback.
    pub fn record_placement(
        &mut self,
        destination: PathBuf,
        source: Option<PathBuf>,
        size_bytes: u64,
    ) {
        self.placed_files.push(RollbackFile {
            destination,
            source,
            size_bytes,
        });
    }

    /// Records a created directory for potential rollback.
    pub fn record_directory(&mut self, path: PathBuf) {
        self.created_directories.push(path);
    }

    /// Returns true if there's anything to roll back.
    pub fn has_changes(&self) -> bool {
        !self.placed_files.is_empty() || !self.created_directories.is_empty()
    }
}

/// Result of a rollback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    /// Job ID.
    pub job_id: String,
    /// Files that were successfully rolled back.
    pub files_removed: usize,
    /// Directories that were successfully removed.
    pub directories_removed: usize,
    /// Any errors that occurred during rollback.
    pub errors: Vec<String>,
    /// Whether rollback completed successfully.
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_plan_new() {
        let plan = RollbackPlan::new("job-1".to_string());
        assert_eq!(plan.job_id, "job-1");
        assert!(plan.placed_files.is_empty());
        assert!(plan.created_directories.is_empty());
        assert!(!plan.has_changes());
    }

    #[test]
    fn test_rollback_plan_record() {
        let mut plan = RollbackPlan::new("job-1".to_string());

        plan.record_placement(
            PathBuf::from("/dest/file.mp3"),
            Some(PathBuf::from("/src/file.mp3")),
            1024,
        );
        plan.record_directory(PathBuf::from("/dest"));

        assert!(plan.has_changes());
        assert_eq!(plan.placed_files.len(), 1);
        assert_eq!(plan.created_directories.len(), 1);
    }

    #[test]
    fn test_placement_job() {
        let job = PlacementJob {
            job_id: "job-1".to_string(),
            files: vec![FilePlacement {
                item_id: "track-1".to_string(),
                source: PathBuf::from("/tmp/converted.mp3"),
                destination: PathBuf::from("/music/album/01-song.mp3"),
                overwrite: false,
                verify_checksum: Some(ChecksumType::Sha256),
            }],
            atomic: true,
            cleanup_sources: true,
            enable_rollback: true,
        };

        assert_eq!(job.files.len(), 1);
        assert!(job.atomic);
    }
}
