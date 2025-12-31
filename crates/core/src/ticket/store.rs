//! Ticket storage trait and types.
//!
//! This module will be fully implemented in Task 2.

use std::fmt;

use crate::ticket::{OutputConstraints, QueryContext, Ticket, TicketState};

/// Error type for ticket operations.
#[derive(Debug)]
pub enum TicketError {
    /// Ticket not found.
    NotFound(String),
    /// Cannot perform operation due to current state.
    InvalidState {
        ticket_id: String,
        current_state: String,
        operation: String,
    },
    /// Database error.
    Database(String),
}

impl fmt::Display for TicketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TicketError::NotFound(id) => write!(f, "Ticket not found: {}", id),
            TicketError::InvalidState {
                ticket_id,
                current_state,
                operation,
            } => write!(
                f,
                "Cannot {} ticket {}: current state is {}",
                operation, ticket_id, current_state
            ),
            TicketError::Database(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for TicketError {}

/// Request to create a new ticket.
#[derive(Debug, Clone)]
pub struct CreateTicketRequest {
    /// User creating the ticket.
    pub created_by: String,
    /// Priority (higher = more urgent).
    pub priority: u16,
    /// Query context for search/matching.
    pub query_context: QueryContext,
    /// Destination path for output.
    pub dest_path: String,
    /// Output format constraints (None = keep original, no conversion).
    pub output_constraints: Option<OutputConstraints>,
}

/// Filter for querying tickets.
#[derive(Debug, Clone, Default)]
pub struct TicketFilter {
    /// Filter by state type.
    pub state: Option<String>,
    /// Filter by creator.
    pub created_by: Option<String>,
    /// Maximum number of results.
    pub limit: i64,
    /// Offset for pagination.
    pub offset: i64,
}

impl TicketFilter {
    /// Create a new filter with defaults.
    pub fn new() -> Self {
        Self {
            state: None,
            created_by: None,
            limit: 100,
            offset: 0,
        }
    }

    /// Filter by state type.
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Filter by creator.
    pub fn with_created_by(mut self, created_by: impl Into<String>) -> Self {
        self.created_by = Some(created_by.into());
        self
    }

    /// Set limit.
    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    /// Set offset.
    pub fn with_offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }
}

/// Trait for ticket storage backends.
pub trait TicketStore: Send + Sync {
    /// Create a new ticket.
    fn create(&self, request: CreateTicketRequest) -> Result<Ticket, TicketError>;

    /// Get a ticket by ID.
    fn get(&self, id: &str) -> Result<Option<Ticket>, TicketError>;

    /// List tickets matching the filter.
    fn list(&self, filter: &TicketFilter) -> Result<Vec<Ticket>, TicketError>;

    /// Count tickets matching the filter.
    fn count(&self, filter: &TicketFilter) -> Result<i64, TicketError>;

    /// Update a ticket's state.
    fn update_state(&self, id: &str, new_state: TicketState) -> Result<Ticket, TicketError>;

    /// Permanently delete a ticket and all associated data.
    /// Returns the deleted ticket if found.
    fn delete(&self, id: &str) -> Result<Ticket, TicketError>;
}
