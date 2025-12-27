use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    // Search events
    SearchExecuted {
        /// Who initiated the search
        user_id: String,
        /// Search backend used (e.g., "jackett")
        searcher: String,
        /// The query that was searched
        query: String,
        /// Which indexers were queried
        indexers_queried: Vec<String>,
        /// Number of results returned
        results_count: u32,
        /// How long the search took in milliseconds
        duration_ms: u64,
        /// Any indexers that failed (name -> error message)
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        indexer_errors: HashMap<String, String>,
    },
    IndexerRateLimitUpdated {
        /// Who updated the rate limit
        user_id: String,
        /// Which indexer was updated
        indexer: String,
        /// Previous rate limit (requests per minute)
        old_rpm: u32,
        /// New rate limit (requests per minute)
        new_rpm: u32,
    },
    IndexerEnabledChanged {
        /// Who changed the enabled state
        user_id: String,
        /// Which indexer was updated
        indexer: String,
        /// New enabled state
        enabled: bool,
    },

    // Torrent client events
    TorrentAdded {
        /// Who added the torrent
        user_id: String,
        /// Info hash of the added torrent
        hash: String,
        /// Torrent name (if available)
        name: Option<String>,
        /// Source: "magnet" or "file"
        source: String,
        /// Associated ticket (if any)
        ticket_id: Option<String>,
    },
    TorrentRemoved {
        /// Who removed the torrent
        user_id: String,
        /// Info hash of the removed torrent
        hash: String,
        /// Torrent name
        name: String,
        /// Whether files were also deleted
        delete_files: bool,
    },
    TorrentPaused {
        /// Who paused the torrent
        user_id: String,
        /// Info hash
        hash: String,
        /// Torrent name
        name: String,
    },
    TorrentResumed {
        /// Who resumed the torrent
        user_id: String,
        /// Info hash
        hash: String,
        /// Torrent name
        name: String,
    },
    TorrentLimitChanged {
        /// Who changed the limit
        user_id: String,
        /// Info hash
        hash: String,
        /// Torrent name
        name: String,
        /// Type of limit: "upload" or "download"
        limit_type: String,
        /// Previous limit (bytes/sec, 0 = unlimited)
        old_limit: u64,
        /// New limit (bytes/sec, 0 = unlimited)
        new_limit: u64,
    },
    TorrentRechecked {
        /// Who initiated the recheck
        user_id: String,
        /// Info hash
        hash: String,
        /// Torrent name
        name: String,
    },

    // TextBrain events
    QueriesGenerated {
        /// Associated ticket
        ticket_id: String,
        /// Generated search queries
        queries: Vec<String>,
        /// Method used: "dumb", "llm", or "dumb_then_llm"
        method: String,
        /// LLM tokens used (if any)
        llm_input_tokens: Option<u32>,
        llm_output_tokens: Option<u32>,
        /// How long query generation took
        duration_ms: u64,
    },
    CandidatesScored {
        /// Associated ticket
        ticket_id: String,
        /// Number of candidates scored
        candidates_count: u32,
        /// Top candidate info hash (if any)
        top_candidate_hash: Option<String>,
        /// Top candidate score (0-100)
        top_candidate_score: Option<u32>,
        /// Method used: "dumb", "llm", or "dumb_then_llm"
        method: String,
        /// LLM tokens used (if any)
        llm_input_tokens: Option<u32>,
        llm_output_tokens: Option<u32>,
        /// How long scoring took
        duration_ms: u64,
    },
    CandidateSelected {
        /// Associated ticket
        ticket_id: String,
        /// Who selected (user or "auto")
        selected_by: String,
        /// Selected torrent info hash
        hash: String,
        /// Selected torrent title
        title: String,
        /// Final score
        score: u32,
        /// Whether this was auto-selected (high confidence) or manual
        auto_selected: bool,
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
            Self::SearchExecuted { .. } => "search_executed",
            Self::IndexerRateLimitUpdated { .. } => "indexer_rate_limit_updated",
            Self::IndexerEnabledChanged { .. } => "indexer_enabled_changed",
            Self::TorrentAdded { .. } => "torrent_added",
            Self::TorrentRemoved { .. } => "torrent_removed",
            Self::TorrentPaused { .. } => "torrent_paused",
            Self::TorrentResumed { .. } => "torrent_resumed",
            Self::TorrentLimitChanged { .. } => "torrent_limit_changed",
            Self::TorrentRechecked { .. } => "torrent_rechecked",
            Self::QueriesGenerated { .. } => "queries_generated",
            Self::CandidatesScored { .. } => "candidates_scored",
            Self::CandidateSelected { .. } => "candidate_selected",
        }
    }

    /// Extract ticket_id if this event is ticket-related
    pub fn ticket_id(&self) -> Option<&str> {
        match self {
            Self::TicketCreated { ticket_id, .. }
            | Self::TicketStateChanged { ticket_id, .. }
            | Self::TicketCancelled { ticket_id, .. }
            | Self::QueriesGenerated { ticket_id, .. }
            | Self::CandidatesScored { ticket_id, .. }
            | Self::CandidateSelected { ticket_id, .. } => Some(ticket_id),
            Self::TorrentAdded { ticket_id, .. } => ticket_id.as_deref(),
            _ => None,
        }
    }

    /// Extract user_id if this event was triggered by a user action
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::TicketCreated { requested_by, .. } => Some(requested_by),
            Self::TicketCancelled { cancelled_by, .. } => Some(cancelled_by),
            Self::CandidateSelected { selected_by, .. } => Some(selected_by),
            Self::SearchExecuted { user_id, .. }
            | Self::IndexerRateLimitUpdated { user_id, .. }
            | Self::IndexerEnabledChanged { user_id, .. }
            | Self::TorrentAdded { user_id, .. }
            | Self::TorrentRemoved { user_id, .. }
            | Self::TorrentPaused { user_id, .. }
            | Self::TorrentResumed { user_id, .. }
            | Self::TorrentLimitChanged { user_id, .. }
            | Self::TorrentRechecked { user_id, .. } => Some(user_id),
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

    #[test]
    fn test_event_type_search_executed() {
        let event = AuditEvent::SearchExecuted {
            user_id: "user-123".to_string(),
            searcher: "jackett".to_string(),
            query: "test query".to_string(),
            indexers_queried: vec!["rutracker".to_string(), "redacted".to_string()],
            results_count: 42,
            duration_ms: 1500,
            indexer_errors: HashMap::new(),
        };
        assert_eq!(event.event_type(), "search_executed");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), Some("user-123"));
    }

    #[test]
    fn test_event_type_indexer_rate_limit_updated() {
        let event = AuditEvent::IndexerRateLimitUpdated {
            user_id: "admin".to_string(),
            indexer: "rutracker".to_string(),
            old_rpm: 10,
            new_rpm: 20,
        };
        assert_eq!(event.event_type(), "indexer_rate_limit_updated");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), Some("admin"));
    }

    #[test]
    fn test_event_type_indexer_enabled_changed() {
        let event = AuditEvent::IndexerEnabledChanged {
            user_id: "admin".to_string(),
            indexer: "redacted".to_string(),
            enabled: false,
        };
        assert_eq!(event.event_type(), "indexer_enabled_changed");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), Some("admin"));
    }

    #[test]
    fn test_serialize_deserialize_search_executed() {
        let mut errors = HashMap::new();
        errors.insert("failed_indexer".to_string(), "timeout".to_string());

        let event = AuditEvent::SearchExecuted {
            user_id: "user-1".to_string(),
            searcher: "jackett".to_string(),
            query: "Radiohead".to_string(),
            indexers_queried: vec!["indexer1".to_string()],
            results_count: 10,
            duration_ms: 500,
            indexer_errors: errors,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"search_executed\""));
        assert!(json.contains("\"query\":\"Radiohead\""));
        assert!(json.contains("\"indexer_errors\""));

        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), "search_executed");
    }

    #[test]
    fn test_serialize_search_executed_empty_errors() {
        let event = AuditEvent::SearchExecuted {
            user_id: "user-1".to_string(),
            searcher: "jackett".to_string(),
            query: "test".to_string(),
            indexers_queried: vec![],
            results_count: 0,
            duration_ms: 100,
            indexer_errors: HashMap::new(),
        };

        let json = serde_json::to_string(&event).unwrap();
        // Empty hashmap should be skipped
        assert!(!json.contains("indexer_errors"));
    }

    #[test]
    fn test_event_type_torrent_added() {
        let event = AuditEvent::TorrentAdded {
            user_id: "user-123".to_string(),
            hash: "abc123def456".to_string(),
            name: Some("Test Torrent".to_string()),
            source: "magnet".to_string(),
            ticket_id: Some("ticket-789".to_string()),
        };
        assert_eq!(event.event_type(), "torrent_added");
        assert_eq!(event.ticket_id(), Some("ticket-789"));
        assert_eq!(event.user_id(), Some("user-123"));
    }

    #[test]
    fn test_event_type_torrent_added_no_ticket() {
        let event = AuditEvent::TorrentAdded {
            user_id: "user-123".to_string(),
            hash: "abc123def456".to_string(),
            name: None,
            source: "file".to_string(),
            ticket_id: None,
        };
        assert_eq!(event.event_type(), "torrent_added");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), Some("user-123"));
    }

    #[test]
    fn test_event_type_torrent_removed() {
        let event = AuditEvent::TorrentRemoved {
            user_id: "admin".to_string(),
            hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
            delete_files: true,
        };
        assert_eq!(event.event_type(), "torrent_removed");
        assert_eq!(event.ticket_id(), None);
        assert_eq!(event.user_id(), Some("admin"));
    }

    #[test]
    fn test_event_type_torrent_paused() {
        let event = AuditEvent::TorrentPaused {
            user_id: "user-1".to_string(),
            hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
        };
        assert_eq!(event.event_type(), "torrent_paused");
        assert_eq!(event.user_id(), Some("user-1"));
    }

    #[test]
    fn test_event_type_torrent_resumed() {
        let event = AuditEvent::TorrentResumed {
            user_id: "user-1".to_string(),
            hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
        };
        assert_eq!(event.event_type(), "torrent_resumed");
        assert_eq!(event.user_id(), Some("user-1"));
    }

    #[test]
    fn test_event_type_torrent_limit_changed() {
        let event = AuditEvent::TorrentLimitChanged {
            user_id: "admin".to_string(),
            hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
            limit_type: "upload".to_string(),
            old_limit: 0,
            new_limit: 1048576,
        };
        assert_eq!(event.event_type(), "torrent_limit_changed");
        assert_eq!(event.user_id(), Some("admin"));
    }

    #[test]
    fn test_event_type_torrent_rechecked() {
        let event = AuditEvent::TorrentRechecked {
            user_id: "user-1".to_string(),
            hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
        };
        assert_eq!(event.event_type(), "torrent_rechecked");
        assert_eq!(event.user_id(), Some("user-1"));
    }

    #[test]
    fn test_serialize_deserialize_torrent_added() {
        let event = AuditEvent::TorrentAdded {
            user_id: "user-1".to_string(),
            hash: "abcdef123456".to_string(),
            name: Some("My Movie".to_string()),
            source: "magnet".to_string(),
            ticket_id: Some("t-001".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"torrent_added\""));
        assert!(json.contains("\"hash\":\"abcdef123456\""));
        assert!(json.contains("\"source\":\"magnet\""));

        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), "torrent_added");
        assert_eq!(deserialized.ticket_id(), Some("t-001"));
    }
}
