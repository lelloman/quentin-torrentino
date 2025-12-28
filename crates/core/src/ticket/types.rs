//! Core ticket data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::textbrain::ScoredCandidateSummary;

/// Query context for search and matching.
///
/// Provides both structured tags for routing/categorization and
/// freeform description for LLM-based intelligent matching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryContext {
    /// Structured tags for categorization and routing.
    /// Examples: ["music", "flac", "album"] or ["movie", "1080p"]
    pub tags: Vec<String>,

    /// Freeform description for LLM-based matching.
    /// Example: "Abbey Road by The Beatles, prefer 2019 remaster"
    pub description: String,
}

impl QueryContext {
    /// Create a new query context.
    pub fn new(tags: Vec<String>, description: impl Into<String>) -> Self {
        Self {
            tags,
            description: description.into(),
        }
    }
}

/// Current phase within the Acquiring state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum AcquisitionPhase {
    /// Building search queries from ticket context.
    QueryBuilding,
    /// Executing search with a specific query.
    Searching { query: String },
    /// Scoring candidates against ticket requirements.
    Scoring { candidates_count: u32 },
}

/// Summary of the selected candidate for storage in ticket state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedCandidate {
    /// Torrent title.
    pub title: String,
    /// Info hash for identification.
    pub info_hash: String,
    /// Magnet URI for downloading.
    pub magnet_uri: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Match score (0.0-1.0).
    pub score: f32,
}

/// Statistics for a completed ticket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompletionStats {
    /// Total bytes downloaded.
    pub total_download_bytes: u64,
    /// Time spent downloading in seconds.
    pub download_duration_secs: u32,
    /// Time spent converting in seconds.
    pub conversion_duration_secs: u32,
    /// Final size of placed files in bytes.
    pub final_size_bytes: u64,
    /// Number of files placed.
    pub files_placed: u32,
}

/// Current state of a ticket.
///
/// State machine flow:
/// ```text
/// Pending -> Acquiring -> NeedsApproval/AutoApproved -> Approved -> Downloading
///                |                                         |
///                v                                         v
///         AcquisitionFailed                            Rejected
///
/// Downloading -> Converting -> Placing -> Completed
///
/// Any non-terminal state can transition to Failed or Cancelled.
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TicketState {
    // ========================================================================
    // Initial state
    // ========================================================================

    /// Ticket created, waiting to be processed.
    Pending,

    // ========================================================================
    // Acquisition phase (query building + search + scoring)
    // ========================================================================

    /// TextBrain is acquiring a torrent (building queries, searching, scoring).
    Acquiring {
        started_at: DateTime<Utc>,
        /// Queries that have been tried so far.
        queries_tried: Vec<String>,
        /// Number of candidates found across all queries.
        candidates_found: u32,
        /// Current phase within acquisition.
        phase: AcquisitionPhase,
    },

    /// Acquisition failed - no suitable torrent found after exhausting all strategies.
    /// Can be retried with force-search or force-magnet.
    AcquisitionFailed {
        /// Queries that were tried.
        queries_tried: Vec<String>,
        /// Total candidates evaluated.
        candidates_seen: u32,
        /// Reason for failure.
        reason: String,
        failed_at: DateTime<Utc>,
    },

    // ========================================================================
    // Approval phase
    // ========================================================================

    /// Candidates found but confidence is below threshold - needs manual approval.
    NeedsApproval {
        /// Top candidates for review.
        candidates: Vec<ScoredCandidateSummary>,
        /// Index of the recommended candidate.
        recommended_idx: usize,
        /// Confidence score of the top candidate.
        confidence: f32,
        waiting_since: DateTime<Utc>,
    },

    /// Automatically approved (confidence >= threshold).
    AutoApproved {
        selected: SelectedCandidate,
        confidence: f32,
        approved_at: DateTime<Utc>,
    },

    /// Manually approved by user/admin.
    Approved {
        selected: SelectedCandidate,
        approved_by: String,
        approved_at: DateTime<Utc>,
    },

    /// Rejected by user/admin (terminal).
    Rejected {
        rejected_by: String,
        reason: Option<String>,
        rejected_at: DateTime<Utc>,
    },

    // ========================================================================
    // Processing phase
    // ========================================================================

    /// Torrent is being downloaded.
    Downloading {
        /// Info hash of the torrent being downloaded.
        info_hash: String,
        /// Download progress (0.0-100.0).
        progress_pct: f32,
        /// Current download speed in bytes per second.
        speed_bps: u64,
        /// Estimated time remaining in seconds.
        eta_secs: Option<u32>,
        started_at: DateTime<Utc>,
    },

    /// Converting downloaded files (transcoding, metadata embedding).
    Converting {
        /// Index of the current item being converted.
        current_idx: usize,
        /// Total items to convert.
        total: usize,
        /// Name of the current item.
        current_name: String,
        started_at: DateTime<Utc>,
    },

    /// Placing converted files to their final destinations.
    Placing {
        /// Number of files already placed.
        files_placed: usize,
        /// Total files to place.
        total_files: usize,
        started_at: DateTime<Utc>,
    },

    // ========================================================================
    // Terminal states
    // ========================================================================

    /// Ticket completed successfully (terminal).
    Completed {
        completed_at: DateTime<Utc>,
        stats: CompletionStats,
    },

    /// Ticket failed (terminal, may be retryable).
    Failed {
        /// Error message.
        error: String,
        /// Whether this failure can be retried.
        retryable: bool,
        /// Number of retry attempts so far.
        retry_count: u32,
        failed_at: DateTime<Utc>,
    },

    /// Ticket was cancelled by user/admin (terminal).
    Cancelled {
        cancelled_by: String,
        reason: Option<String>,
        cancelled_at: DateTime<Utc>,
    },
}

impl TicketState {
    /// Returns true if this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TicketState::Completed { .. }
                | TicketState::Failed { .. }
                | TicketState::Cancelled { .. }
                | TicketState::Rejected { .. }
        )
    }

    /// Returns true if the ticket can be cancelled from this state.
    pub fn can_cancel(&self) -> bool {
        !self.is_terminal()
    }

    /// Returns true if the ticket can be retried from this state.
    pub fn can_retry(&self) -> bool {
        match self {
            TicketState::Failed { retryable, .. } => *retryable,
            TicketState::AcquisitionFailed { .. } => true,
            _ => false,
        }
    }

    /// Returns true if the ticket is in an active processing state.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            TicketState::Acquiring { .. }
                | TicketState::Downloading { .. }
                | TicketState::Converting { .. }
                | TicketState::Placing { .. }
        )
    }

    /// Returns true if the ticket is waiting for user action.
    pub fn needs_attention(&self) -> bool {
        matches!(
            self,
            TicketState::NeedsApproval { .. } | TicketState::AcquisitionFailed { .. }
        )
    }

    /// Returns the state type as a string (for filtering).
    pub fn state_type(&self) -> &'static str {
        match self {
            TicketState::Pending => "pending",
            TicketState::Acquiring { .. } => "acquiring",
            TicketState::AcquisitionFailed { .. } => "acquisition_failed",
            TicketState::NeedsApproval { .. } => "needs_approval",
            TicketState::AutoApproved { .. } => "auto_approved",
            TicketState::Approved { .. } => "approved",
            TicketState::Rejected { .. } => "rejected",
            TicketState::Downloading { .. } => "downloading",
            TicketState::Converting { .. } => "converting",
            TicketState::Placing { .. } => "placing",
            TicketState::Completed { .. } => "completed",
            TicketState::Failed { .. } => "failed",
            TicketState::Cancelled { .. } => "cancelled",
        }
    }
}

/// A ticket representing a content acquisition request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ticket {
    /// Unique identifier (UUID).
    pub id: String,

    /// When the ticket was created.
    pub created_at: DateTime<Utc>,

    /// User who created the ticket (from auth identity).
    pub created_by: String,

    /// Current state.
    pub state: TicketState,

    /// Priority for queue ordering (higher = more urgent).
    pub priority: u16,

    /// Query context for search/matching.
    pub query_context: QueryContext,

    /// Destination path for final output.
    pub dest_path: String,

    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_state_is_not_terminal() {
        let state = TicketState::Pending;
        assert!(!state.is_terminal());
        assert!(state.can_cancel());
        assert!(!state.is_active());
        assert!(!state.needs_attention());
    }

    #[test]
    fn test_acquiring_state() {
        let state = TicketState::Acquiring {
            started_at: Utc::now(),
            queries_tried: vec!["test query".to_string()],
            candidates_found: 5,
            phase: AcquisitionPhase::Searching {
                query: "test query".to_string(),
            },
        };
        assert!(!state.is_terminal());
        assert!(state.can_cancel());
        assert!(state.is_active());
        assert!(!state.needs_attention());
        assert_eq!(state.state_type(), "acquiring");
    }

    #[test]
    fn test_acquisition_failed_state() {
        let state = TicketState::AcquisitionFailed {
            queries_tried: vec!["q1".to_string(), "q2".to_string()],
            candidates_seen: 10,
            reason: "No suitable match found".to_string(),
            failed_at: Utc::now(),
        };
        assert!(!state.is_terminal());
        assert!(state.can_retry());
        assert!(state.needs_attention());
        assert_eq!(state.state_type(), "acquisition_failed");
    }

    #[test]
    fn test_needs_approval_state() {
        let state = TicketState::NeedsApproval {
            candidates: vec![],
            recommended_idx: 0,
            confidence: 0.75,
            waiting_since: Utc::now(),
        };
        assert!(!state.is_terminal());
        assert!(state.needs_attention());
        assert_eq!(state.state_type(), "needs_approval");
    }

    #[test]
    fn test_downloading_state() {
        let state = TicketState::Downloading {
            info_hash: "abc123".to_string(),
            progress_pct: 45.5,
            speed_bps: 1_000_000,
            eta_secs: Some(120),
            started_at: Utc::now(),
        };
        assert!(!state.is_terminal());
        assert!(state.is_active());
        assert_eq!(state.state_type(), "downloading");
    }

    #[test]
    fn test_completed_state_is_terminal() {
        let state = TicketState::Completed {
            completed_at: Utc::now(),
            stats: CompletionStats {
                total_download_bytes: 1_000_000,
                download_duration_secs: 60,
                conversion_duration_secs: 30,
                final_size_bytes: 500_000,
                files_placed: 10,
            },
        };
        assert!(state.is_terminal());
        assert!(!state.can_cancel());
        assert_eq!(state.state_type(), "completed");
    }

    #[test]
    fn test_failed_state_retryable() {
        let state = TicketState::Failed {
            error: "Connection timeout".to_string(),
            retryable: true,
            retry_count: 1,
            failed_at: Utc::now(),
        };
        assert!(state.is_terminal());
        assert!(state.can_retry());
        assert_eq!(state.state_type(), "failed");
    }

    #[test]
    fn test_failed_state_not_retryable() {
        let state = TicketState::Failed {
            error: "Invalid torrent".to_string(),
            retryable: false,
            retry_count: 0,
            failed_at: Utc::now(),
        };
        assert!(state.is_terminal());
        assert!(!state.can_retry());
    }

    #[test]
    fn test_rejected_state_is_terminal() {
        let state = TicketState::Rejected {
            rejected_by: "admin".to_string(),
            reason: Some("Wrong content".to_string()),
            rejected_at: Utc::now(),
        };
        assert!(state.is_terminal());
        assert!(!state.can_cancel());
        assert_eq!(state.state_type(), "rejected");
    }

    #[test]
    fn test_cancelled_state_is_terminal() {
        let state = TicketState::Cancelled {
            cancelled_by: "user".to_string(),
            reason: Some("test".to_string()),
            cancelled_at: Utc::now(),
        };
        assert!(state.is_terminal());
        assert!(!state.can_cancel());
        assert_eq!(state.state_type(), "cancelled");
    }

    #[test]
    fn test_state_type_strings() {
        assert_eq!(TicketState::Pending.state_type(), "pending");

        let acquiring = TicketState::Acquiring {
            started_at: Utc::now(),
            queries_tried: vec![],
            candidates_found: 0,
            phase: AcquisitionPhase::QueryBuilding,
        };
        assert_eq!(acquiring.state_type(), "acquiring");
    }

    #[test]
    fn test_query_context_creation() {
        let ctx = QueryContext::new(vec!["music".to_string(), "flac".to_string()], "test query");
        assert_eq!(ctx.tags, vec!["music", "flac"]);
        assert_eq!(ctx.description, "test query");
    }

    #[test]
    fn test_ticket_state_serialization() {
        let state = TicketState::Pending;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#"{"type":"pending"}"#);

        let deserialized: TicketState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
    }

    #[test]
    fn test_acquiring_state_serialization() {
        let state = TicketState::Acquiring {
            started_at: Utc::now(),
            queries_tried: vec!["test".to_string()],
            candidates_found: 3,
            phase: AcquisitionPhase::Scoring { candidates_count: 3 },
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: TicketState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
    }

    #[test]
    fn test_cancelled_state_serialization() {
        let cancelled_at = Utc::now();
        let state = TicketState::Cancelled {
            cancelled_by: "admin".to_string(),
            reason: Some("no longer needed".to_string()),
            cancelled_at,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: TicketState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
    }

    #[test]
    fn test_acquisition_phase_serialization() {
        let phase = AcquisitionPhase::Searching {
            query: "test query".to_string(),
        };
        let json = serde_json::to_string(&phase).unwrap();
        assert!(json.contains("searching"));
        assert!(json.contains("test query"));

        let deserialized: AcquisitionPhase = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, phase);
    }
}
