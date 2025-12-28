//! Error types for the converter module.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during conversion.
#[derive(Debug, Error)]
pub enum ConverterError {
    /// FFmpeg binary not found.
    #[error("FFmpeg not found at path: {path}")]
    FfmpegNotFound { path: PathBuf },

    /// FFprobe binary not found.
    #[error("FFprobe not found at path: {path}")]
    FfprobeNotFound { path: PathBuf },

    /// Input file not found.
    #[error("Input file not found: {path}")]
    InputNotFound { path: PathBuf },

    /// Input file is not a supported format.
    #[error("Unsupported input format: {format}")]
    UnsupportedInputFormat { format: String },

    /// Output directory does not exist and could not be created.
    #[error("Failed to create output directory: {path}")]
    OutputDirectoryFailed { path: PathBuf },

    /// Conversion process failed.
    #[error("Conversion failed: {reason}")]
    ConversionFailed {
        reason: String,
        stderr: Option<String>,
    },

    /// Conversion timed out.
    #[error("Conversion timed out after {timeout_secs} seconds")]
    Timeout { timeout_secs: u64 },

    /// Failed to probe media file.
    #[error("Failed to probe media file: {reason}")]
    ProbeFailed { reason: String },

    /// Invalid conversion constraints.
    #[error("Invalid constraints: {reason}")]
    InvalidConstraints { reason: String },

    /// Cover art embedding failed.
    #[error("Failed to embed cover art: {reason}")]
    CoverArtFailed { reason: String },

    /// I/O error during conversion.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse FFprobe output.
    #[error("Failed to parse media info: {reason}")]
    ParseError { reason: String },

    /// Job was cancelled.
    #[error("Conversion cancelled")]
    Cancelled,
}

impl ConverterError {
    /// Creates a new conversion failed error with stderr output.
    pub fn conversion_failed(reason: impl Into<String>, stderr: Option<String>) -> Self {
        Self::ConversionFailed {
            reason: reason.into(),
            stderr,
        }
    }

    /// Creates a new probe failed error.
    pub fn probe_failed(reason: impl Into<String>) -> Self {
        Self::ProbeFailed {
            reason: reason.into(),
        }
    }

    /// Creates a new invalid constraints error.
    pub fn invalid_constraints(reason: impl Into<String>) -> Self {
        Self::InvalidConstraints {
            reason: reason.into(),
        }
    }

    /// Whether this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout { .. } | Self::Io(_))
    }
}
