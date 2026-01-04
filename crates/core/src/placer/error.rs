//! Error types for the placer module.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during file placement.
#[derive(Debug, Error)]
pub enum PlacerError {
    /// Source file not found.
    #[error("Source file not found: {path}")]
    SourceNotFound { path: PathBuf },

    /// Destination already exists and overwrite is disabled.
    #[error("Destination already exists: {path}")]
    DestinationExists { path: PathBuf },

    /// Failed to create destination directory.
    #[error("Failed to create directory: {path}")]
    DirectoryCreationFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to copy file.
    #[error("Failed to copy file from {source} to {destination}")]
    CopyFailed {
        source: PathBuf,
        destination: PathBuf,
        #[source]
        error: std::io::Error,
    },

    /// Failed to move/rename file.
    #[error("Failed to move file from {source} to {destination}")]
    MoveFailed {
        source: PathBuf,
        destination: PathBuf,
        #[source]
        error: std::io::Error,
    },

    /// Checksum verification failed.
    #[error("Checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    /// Failed to calculate checksum.
    #[error("Failed to calculate checksum for {path}")]
    ChecksumCalculationFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to delete source file during cleanup.
    #[error("Failed to cleanup source file: {path}")]
    CleanupFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Placement failed partially and rollback is needed.
    #[error("Placement failed after {files_placed} files, rollback required: {reason}")]
    PartialFailure { files_placed: usize, reason: String },

    /// Rollback failed.
    #[error("Rollback failed: {reason}")]
    RollbackFailed { reason: String },

    /// Insufficient disk space.
    #[error(
        "Insufficient disk space at {path}: need {required_bytes} bytes, have {available_bytes}"
    )]
    InsufficientSpace {
        path: PathBuf,
        required_bytes: u64,
        available_bytes: u64,
    },

    /// Permission denied.
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Operation was cancelled.
    #[error("Placement cancelled")]
    Cancelled,
}

impl PlacerError {
    /// Creates a copy failed error.
    pub fn copy_failed(source: PathBuf, destination: PathBuf, error: std::io::Error) -> Self {
        Self::CopyFailed {
            source,
            destination,
            error,
        }
    }

    /// Creates a move failed error.
    pub fn move_failed(source: PathBuf, destination: PathBuf, error: std::io::Error) -> Self {
        Self::MoveFailed {
            source,
            destination,
            error,
        }
    }

    /// Whether this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Io(_) | Self::CopyFailed { .. } | Self::MoveFailed { .. }
        )
    }

    /// Whether this error requires rollback.
    pub fn requires_rollback(&self) -> bool {
        matches!(self, Self::PartialFailure { .. })
    }
}
