//! Types for the ticket orchestrator.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during orchestration.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// Ticket not found.
    #[error("ticket not found: {0}")]
    TicketNotFound(String),

    /// Invalid ticket state for operation.
    #[error("invalid ticket state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },

    /// Ticket store error.
    #[error("ticket store error: {0}")]
    TicketStore(#[from] crate::ticket::TicketError),

    /// Torrent client error.
    #[error("torrent client error: {0}")]
    TorrentClient(#[from] crate::torrent_client::TorrentClientError),

    /// Searcher error.
    #[error("searcher error: {0}")]
    Searcher(#[from] crate::searcher::SearchError),

    /// Pipeline error.
    #[error("pipeline error: {0}")]
    Pipeline(#[from] crate::processor::PipelineError),

    /// TextBrain error.
    #[error("textbrain error: {0}")]
    TextBrain(#[from] crate::textbrain::TextBrainError),

    /// Missing required data in ticket state.
    #[error("missing data in ticket: {0}")]
    MissingData(String),
}

/// An active download being tracked by the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDownload {
    /// Ticket ID this download belongs to.
    pub ticket_id: String,
    /// Info hash of the torrent.
    pub info_hash: String,
    /// When the download started.
    pub started_at: DateTime<Utc>,
}

/// Current status of the orchestrator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrchestratorStatus {
    /// Whether the orchestrator is running.
    pub running: bool,
    /// Number of active downloads being tracked.
    pub active_downloads: usize,
    /// Tickets currently being acquired (should be 0 or 1).
    pub acquiring_count: usize,
    /// Tickets waiting for acquisition.
    pub pending_count: usize,
    /// Tickets waiting for approval.
    pub needs_approval_count: usize,
    /// Tickets currently downloading.
    pub downloading_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_download_serialization() {
        let download = ActiveDownload {
            ticket_id: "ticket-123".to_string(),
            info_hash: "abc123def456".to_string(),
            started_at: Utc::now(),
        };

        let json = serde_json::to_string(&download).unwrap();
        let parsed: ActiveDownload = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.ticket_id, "ticket-123");
        assert_eq!(parsed.info_hash, "abc123def456");
    }

    #[test]
    fn test_orchestrator_status_default() {
        let status = OrchestratorStatus::default();
        assert!(!status.running);
        assert_eq!(status.active_downloads, 0);
        assert_eq!(status.pending_count, 0);
    }

    #[test]
    fn test_error_display() {
        let err = OrchestratorError::TicketNotFound("ticket-456".to_string());
        assert_eq!(err.to_string(), "ticket not found: ticket-456");

        let err = OrchestratorError::InvalidState {
            expected: "Pending".to_string(),
            actual: "Completed".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid ticket state: expected Pending, got Completed"
        );
    }
}
