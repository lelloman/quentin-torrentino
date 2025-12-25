//! Core ticket data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

/// Current state of a ticket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TicketState {
    /// Ticket created, waiting to be processed.
    Pending,

    /// Ticket was cancelled by user/admin.
    Cancelled {
        cancelled_by: String,
        reason: Option<String>,
        cancelled_at: DateTime<Utc>,
    },

    /// Ticket completed successfully.
    Completed { completed_at: DateTime<Utc> },

    /// Ticket failed.
    Failed {
        error: String,
        failed_at: DateTime<Utc>,
    },
}

impl TicketState {
    /// Returns true if this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TicketState::Cancelled { .. } | TicketState::Completed { .. } | TicketState::Failed { .. }
        )
    }

    /// Returns true if the ticket can be cancelled from this state.
    pub fn can_cancel(&self) -> bool {
        !self.is_terminal()
    }

    /// Returns the state type as a string (for filtering).
    pub fn state_type(&self) -> &'static str {
        match self {
            TicketState::Pending => "pending",
            TicketState::Cancelled { .. } => "cancelled",
            TicketState::Completed { .. } => "completed",
            TicketState::Failed { .. } => "failed",
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
    }

    #[test]
    fn test_completed_state_is_terminal() {
        let state = TicketState::Completed {
            completed_at: Utc::now(),
        };
        assert!(state.is_terminal());
        assert!(!state.can_cancel());
    }

    #[test]
    fn test_failed_state_is_terminal() {
        let state = TicketState::Failed {
            error: "test error".to_string(),
            failed_at: Utc::now(),
        };
        assert!(state.is_terminal());
        assert!(!state.can_cancel());
    }

    #[test]
    fn test_state_type_strings() {
        assert_eq!(TicketState::Pending.state_type(), "pending");
        assert_eq!(
            TicketState::Cancelled {
                cancelled_by: "u".to_string(),
                reason: None,
                cancelled_at: Utc::now()
            }
            .state_type(),
            "cancelled"
        );
        assert_eq!(
            TicketState::Completed {
                completed_at: Utc::now()
            }
            .state_type(),
            "completed"
        );
        assert_eq!(
            TicketState::Failed {
                error: "e".to_string(),
                failed_at: Utc::now()
            }
            .state_type(),
            "failed"
        );
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
}
