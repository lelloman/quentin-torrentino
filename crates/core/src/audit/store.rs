use chrono::{DateTime, Utc};
use thiserror::Error;

use super::AuditRecord;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Filter for querying audit events
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub ticket_id: Option<String>,
    pub event_type: Option<String>,
    pub user_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}

impl AuditFilter {
    pub fn new() -> Self {
        Self {
            limit: 100,
            offset: 0,
            ..Default::default()
        }
    }

    pub fn with_ticket_id(mut self, ticket_id: impl Into<String>) -> Self {
        self.ticket_id = Some(ticket_id.into());
        self
    }

    pub fn with_event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_type = Some(event_type.into());
        self
    }

    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_time_range(
        mut self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Self {
        self.from = from;
        self.to = to;
        self
    }

    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }
}

/// Trait for audit event storage
pub trait AuditStore: Send + Sync {
    /// Insert an audit record, returns the assigned ID
    fn insert(&self, record: &AuditRecord) -> Result<i64, AuditError>;

    /// Query audit records with optional filters
    fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditRecord>, AuditError>;

    /// Count matching audit records
    fn count(&self, filter: &AuditFilter) -> Result<i64, AuditError>;
}
