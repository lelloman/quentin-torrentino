//! Types for content-specific processing.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during content processing.
#[derive(Debug, Error)]
pub enum ContentError {
    /// Failed to fetch external asset.
    #[error("failed to fetch {asset_type}: {reason}")]
    FetchFailed { asset_type: String, reason: String },

    /// IO error during post-processing.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP error during external API call.
    #[error("HTTP error: {0}")]
    Http(String),

    /// External API returned an error.
    #[error("API error from {service}: {message}")]
    ApiError { service: String, message: String },

    /// Content not found in external service.
    #[error("{content_type} not found: {query}")]
    NotFound { content_type: String, query: String },
}

/// Result of post-processing after download completes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostProcessResult {
    /// Path to fetched cover art (for music).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_art_path: Option<PathBuf>,

    /// Paths to fetched subtitle files (for video).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtitle_paths: Vec<PathBuf>,

    /// Any additional metadata fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,

    /// Warnings/notes from post-processing (non-fatal issues).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl PostProcessResult {
    /// Create an empty result (no post-processing done).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create a result with cover art.
    pub fn with_cover_art(path: PathBuf) -> Self {
        Self {
            cover_art_path: Some(path),
            ..Default::default()
        }
    }

    /// Create a result with subtitles.
    pub fn with_subtitles(paths: Vec<PathBuf>) -> Self {
        Self {
            subtitle_paths: paths,
            ..Default::default()
        }
    }

    /// Add a warning to the result.
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_process_result_default() {
        let result = PostProcessResult::default();
        assert!(result.cover_art_path.is_none());
        assert!(result.subtitle_paths.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_post_process_result_with_cover_art() {
        let result = PostProcessResult::with_cover_art(PathBuf::from("/tmp/cover.jpg"));
        assert_eq!(
            result.cover_art_path,
            Some(PathBuf::from("/tmp/cover.jpg"))
        );
    }

    #[test]
    fn test_content_error_display() {
        let err = ContentError::FetchFailed {
            asset_type: "cover art".to_string(),
            reason: "404 not found".to_string(),
        };
        assert_eq!(err.to_string(), "failed to fetch cover art: 404 not found");
    }

    #[test]
    fn test_post_process_result_serialization() {
        let result = PostProcessResult {
            cover_art_path: Some(PathBuf::from("/tmp/cover.jpg")),
            subtitle_paths: vec![],
            metadata: None,
            warnings: vec!["test warning".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("cover_art_path"));
        assert!(json.contains("warnings"));
        // Empty vecs should be skipped
        assert!(!json.contains("subtitle_paths"));
    }
}
