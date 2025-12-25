use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEvent {
    // System events
    ServiceStarted {
        version: String,
        config_hash: String,
    },
    ServiceStopped {
        reason: String,
    },

    // Ticket lifecycle
    TicketCreated {
        ticket_id: String,
        requested_by: String,
        priority: u16,
        tags: Vec<String>,
        description: String,
        dest_path: String,
    },
    TicketStateChanged {
        ticket_id: String,
        from_state: String,
        to_state: String,
        reason: Option<String>,
    },
    TicketCancelled {
        ticket_id: String,
        cancelled_by: String,
        reason: Option<String>,
        previous_state: String,
    },
}

impl AuditEvent {
    /// Returns the event type as a string for storage
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ServiceStarted { .. } => "service_started",
            Self::ServiceStopped { .. } => "service_stopped",
            Self::TicketCreated { .. } => "ticket_created",
            Self::TicketStateChanged { .. } => "ticket_state_changed",
            Self::TicketCancelled { .. } => "ticket_cancelled",
        }
    }

    /// Extract ticket_id if this event is ticket-related
    pub fn ticket_id(&self) -> Option<&str> {
        match self {
            Self::ServiceStarted { .. } | Self::ServiceStopped { .. } => None,
            Self::TicketCreated { ticket_id, .. }
            | Self::TicketStateChanged { ticket_id, .. }
            | Self::TicketCancelled { ticket_id, .. } => Some(ticket_id),
        }
    }

    /// Extract user_id if this event was triggered by a user action
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::TicketCreated { requested_by, .. } => Some(requested_by),
            Self::TicketCancelled { cancelled_by, .. } => Some(cancelled_by),
            _ => None,
        }
    }
}

/// A stored audit record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub ticket_id: Option<String>,
    pub user_id: Option<String>,
    pub data: AuditEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_service_started() {
        let event = AuditEvent::ServiceStarted {
            version: "0.1.0".to_string(),
            config_hash: "abc123".to_string(),
        };
        assert_eq!(event.event_type(), "service_started");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), None);
    }

    #[test]
    fn test_event_type_service_stopped() {
        let event = AuditEvent::ServiceStopped {
            reason: "shutdown".to_string(),
        };
        assert_eq!(event.event_type(), "service_stopped");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), None);
    }

    #[test]
    fn test_event_type_ticket_created() {
        let event = AuditEvent::TicketCreated {
            ticket_id: "ticket-123".to_string(),
            requested_by: "user-456".to_string(),
            priority: 100,
            tags: vec!["music".to_string(), "flac".to_string()],
            description: "test description".to_string(),
            dest_path: "/media/test".to_string(),
        };
        assert_eq!(event.event_type(), "ticket_created");
        assert_eq!(event.ticket_id(), Some("ticket-123"));
        assert_eq!(event.user_id(), Some("user-456"));
    }

    #[test]
    fn test_event_type_ticket_state_changed() {
        let event = AuditEvent::TicketStateChanged {
            ticket_id: "ticket-123".to_string(),
            from_state: "pending".to_string(),
            to_state: "searching".to_string(),
            reason: Some("auto-transition".to_string()),
        };
        assert_eq!(event.event_type(), "ticket_state_changed");
        assert_eq!(event.ticket_id(), Some("ticket-123"));
        assert_eq!(event.user_id(), None);
    }

    #[test]
    fn test_event_type_ticket_cancelled() {
        let event = AuditEvent::TicketCancelled {
            ticket_id: "ticket-123".to_string(),
            cancelled_by: "admin".to_string(),
            reason: Some("duplicate request".to_string()),
            previous_state: "pending".to_string(),
        };
        assert_eq!(event.event_type(), "ticket_cancelled");
        assert_eq!(event.ticket_id(), Some("ticket-123"));
        assert_eq!(event.user_id(), Some("admin"));
    }

    #[test]
    fn test_serialize_deserialize_service_started() {
        let event = AuditEvent::ServiceStarted {
            version: "0.1.0".to_string(),
            config_hash: "abc123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"service_started\""));
        assert!(json.contains("\"version\":\"0.1.0\""));

        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), "service_started");
    }

    #[test]
    fn test_serialize_deserialize_ticket_created() {
        let event = AuditEvent::TicketCreated {
            ticket_id: "t-001".to_string(),
            requested_by: "user-1".to_string(),
            priority: 50,
            tags: vec!["movie".to_string()],
            description: "Test movie".to_string(),
            dest_path: "/media/movies".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.event_type(), "ticket_created");
        assert_eq!(deserialized.ticket_id(), Some("t-001"));
        assert_eq!(deserialized.user_id(), Some("user-1"));
    }

    #[test]
    fn test_audit_record_serialize() {
        let record = AuditRecord {
            id: 1,
            timestamp: Utc::now(),
            event_type: "service_started".to_string(),
            ticket_id: None,
            user_id: None,
            data: AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc123".to_string(),
            },
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"event_type\":\"service_started\""));
    }
}
