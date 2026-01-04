use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Candidate info for training data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingCandidate {
    /// Torrent title
    pub title: String,
    /// Info hash
    pub hash: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Seeder count
    pub seeders: u32,
    /// Category (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// File info for training data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingFile {
    /// File path within torrent
    pub path: String,
    /// Size in bytes
    pub size_bytes: u64,
}

/// File mapping for training data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingFileMapping {
    /// Torrent file path
    pub file_path: String,
    /// Matched item ID
    pub item_id: String,
    /// Confidence score
    pub confidence: f32,
}

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
    /// Ticket was permanently deleted (hard delete).
    TicketDeleted {
        ticket_id: String,
        deleted_by: String,
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
    /// Acquisition process started for a ticket.
    AcquisitionStarted {
        /// Associated ticket
        ticket_id: String,
        /// TextBrain mode being used
        mode: String,
        /// Description from query context
        description: String,
    },

    /// Query building phase started.
    QueryBuildingStarted {
        /// Associated ticket
        ticket_id: String,
        /// Method being attempted: "dumb", "llm", or hybrid
        method: String,
    },

    /// Query building phase completed.
    QueryBuildingCompleted {
        /// Associated ticket
        ticket_id: String,
        /// Generated queries
        queries: Vec<String>,
        /// Method that succeeded
        method: String,
        /// Duration in milliseconds
        duration_ms: u64,
    },

    /// Search operation started.
    SearchStarted {
        /// Associated ticket
        ticket_id: String,
        /// The query being searched
        query: String,
        /// Query index (1-based)
        query_index: u32,
        /// Total queries to try
        total_queries: u32,
    },

    /// Search operation completed.
    SearchCompleted {
        /// Associated ticket
        ticket_id: String,
        /// The query that was searched
        query: String,
        /// Number of candidates found
        candidates_found: u32,
        /// Duration in milliseconds
        duration_ms: u64,
    },

    /// Scoring phase started.
    ScoringStarted {
        /// Associated ticket
        ticket_id: String,
        /// Number of candidates to score
        candidates_count: u32,
        /// Method being attempted: "dumb", "llm", or hybrid
        method: String,
    },

    /// Scoring phase completed.
    ScoringCompleted {
        /// Associated ticket
        ticket_id: String,
        /// Number of candidates scored
        candidates_count: u32,
        /// Top candidate info hash (if any)
        top_candidate_hash: Option<String>,
        /// Top candidate score (0.0-1.0)
        top_candidate_score: Option<f32>,
        /// Method that succeeded
        method: String,
        /// Duration in milliseconds
        duration_ms: u64,
    },

    /// LLM call started.
    LlmCallStarted {
        /// Associated ticket (if any)
        ticket_id: Option<String>,
        /// Purpose of the call: "query_building" or "scoring"
        purpose: String,
        /// LLM provider being used
        provider: String,
        /// Model being used
        model: String,
    },

    /// LLM call completed successfully.
    LlmCallCompleted {
        /// Associated ticket (if any)
        ticket_id: Option<String>,
        /// Purpose of the call
        purpose: String,
        /// Input tokens used
        input_tokens: u32,
        /// Output tokens used
        output_tokens: u32,
        /// Duration in milliseconds
        duration_ms: u64,
    },

    /// LLM call failed.
    LlmCallFailed {
        /// Associated ticket (if any)
        ticket_id: Option<String>,
        /// Purpose of the call
        purpose: String,
        /// Error message
        error: String,
        /// Duration before failure in milliseconds
        duration_ms: u64,
        /// Whether it was a timeout
        is_timeout: bool,
    },

    /// Acquisition completed (summary event).
    AcquisitionCompleted {
        /// Associated ticket
        ticket_id: String,
        /// Whether a suitable candidate was found
        success: bool,
        /// Number of queries tried
        queries_tried: u32,
        /// Number of candidates evaluated
        candidates_evaluated: u32,
        /// Best candidate score (if any)
        best_score: Option<f32>,
        /// Total duration in milliseconds
        duration_ms: u64,
        /// Outcome: "auto_approved", "needs_approval", "no_candidates", "failed"
        outcome: String,
    },

    /// Fallback search started (e.g., discography search after album search fails).
    FallbackSearchStarted {
        /// Associated ticket
        ticket_id: String,
        /// Reason for fallback
        reason: String,
        /// Number of fallback queries to try
        fallback_queries: u32,
    },

    /// Fallback search completed.
    FallbackSearchCompleted {
        /// Associated ticket
        ticket_id: String,
        /// Number of candidates found in fallback
        candidates_found: u32,
        /// Best score from fallback candidates
        best_score: Option<f32>,
        /// Whether a discography was used
        used_discography: bool,
    },

    /// A discography/collection candidate was identified and scored.
    DiscographyCandidateScored {
        /// Associated ticket
        ticket_id: String,
        /// Candidate title
        title: String,
        /// Candidate info hash
        info_hash: String,
        /// Whether the target album was found in the file listing
        album_found: bool,
        /// Score assigned to this candidate
        score: f32,
        /// Number of files in the discography (if known)
        file_count: Option<u32>,
    },

    /// Target album found within a discography's file listing.
    AlbumFoundInDiscography {
        /// Associated ticket
        ticket_id: String,
        /// Discography title
        discography_title: String,
        /// Target album title being searched
        album_title: String,
        /// Number of matching tracks found
        matching_tracks: u32,
        /// Expected track count (if known)
        expected_tracks: Option<u32>,
    },

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

    // ==========================================================================
    // Training data events (for LLM fine-tuning)
    // ==========================================================================
    /// Full context for query building - used for training query generation models.
    TrainingQueryContext {
        /// Unique training sample ID
        sample_id: String,
        /// Associated ticket
        ticket_id: String,
        /// Input: structured tags
        input_tags: Vec<String>,
        /// Input: freeform description
        input_description: String,
        /// Input: expected content (serialized JSON)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_expected: Option<String>,
        /// Output: generated queries
        output_queries: Vec<String>,
        /// Method that generated these queries
        method: String,
        /// Confidence score
        confidence: f32,
        /// Whether these queries led to a successful match (filled in later)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        success: Option<bool>,
    },

    /// Full context for candidate scoring - used for training ranking models.
    TrainingScoringContext {
        /// Unique training sample ID
        sample_id: String,
        /// Associated ticket
        ticket_id: String,
        /// Input: query context description
        input_description: String,
        /// Input: expected content (serialized JSON)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_expected: Option<String>,
        /// Input: candidate titles
        input_candidates: Vec<TrainingCandidate>,
        /// Output: recommended index
        output_recommended_idx: usize,
        /// Output: scores for each candidate
        output_scores: Vec<f32>,
        /// Method used for scoring
        method: String,
    },

    /// File mapping result - used for training file matching models.
    TrainingFileMappingContext {
        /// Unique training sample ID
        sample_id: String,
        /// Associated ticket
        ticket_id: String,
        /// Input: expected content (serialized JSON)
        input_expected: String,
        /// Input: torrent files
        input_files: Vec<TrainingFile>,
        /// Output: file mappings
        output_mappings: Vec<TrainingFileMapping>,
        /// Mapping quality score
        quality: f32,
    },

    /// User correction - when user selects different candidate than recommended.
    /// This is valuable training data for improving ranking.
    UserCorrection {
        /// Associated ticket
        ticket_id: String,
        /// Original recommended index
        recommended_idx: usize,
        /// User-selected index
        selected_idx: usize,
        /// Query context description
        context_description: String,
        /// Expected content (serialized JSON)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_content: Option<String>,
        /// Candidates that were presented
        candidates: Vec<TrainingCandidate>,
        /// User ID who made the correction
        user_id: String,
    },

    // ==========================================================================
    // Phase 4: Conversion events
    // ==========================================================================
    /// Conversion job started.
    ConversionStarted {
        /// Associated ticket
        ticket_id: String,
        /// Conversion job ID
        job_id: String,
        /// Input file path
        input_path: String,
        /// Output file path
        output_path: String,
        /// Target format
        target_format: String,
        /// Number of files to convert in this job
        total_files: usize,
    },

    /// Conversion progress update (periodic).
    ConversionProgress {
        /// Associated ticket
        ticket_id: String,
        /// Conversion job ID
        job_id: String,
        /// Current file index (0-based)
        current_idx: usize,
        /// Total files
        total_files: usize,
        /// Current file name
        current_file: String,
        /// Progress percentage (0-100)
        percent: u8,
    },

    /// Conversion completed successfully.
    ConversionCompleted {
        /// Associated ticket
        ticket_id: String,
        /// Conversion job ID
        job_id: String,
        /// Number of files converted
        files_converted: usize,
        /// Total output size in bytes
        output_bytes: u64,
        /// Duration in milliseconds
        duration_ms: u64,
        /// Input format
        input_format: String,
        /// Output format
        output_format: String,
    },

    /// Conversion failed.
    ConversionFailed {
        /// Associated ticket
        ticket_id: String,
        /// Conversion job ID
        job_id: String,
        /// File that failed (if applicable)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failed_file: Option<String>,
        /// Error message
        error: String,
        /// Files completed before failure
        files_completed: usize,
        /// Whether the error is retryable
        retryable: bool,
    },

    // ==========================================================================
    // Phase 4: Placement events
    // ==========================================================================
    /// Placement job started.
    PlacementStarted {
        /// Associated ticket
        ticket_id: String,
        /// Placement job ID
        job_id: String,
        /// Number of files to place
        total_files: usize,
        /// Total bytes to place
        total_bytes: u64,
    },

    /// Placement progress update (periodic).
    PlacementProgress {
        /// Associated ticket
        ticket_id: String,
        /// Placement job ID
        job_id: String,
        /// Files placed so far
        files_placed: usize,
        /// Total files
        total_files: usize,
        /// Bytes copied so far
        bytes_placed: u64,
        /// Current file being placed
        current_file: String,
    },

    /// Placement completed successfully.
    PlacementCompleted {
        /// Associated ticket
        ticket_id: String,
        /// Placement job ID
        job_id: String,
        /// Number of files placed
        files_placed: usize,
        /// Total bytes placed
        total_bytes: u64,
        /// Duration in milliseconds
        duration_ms: u64,
        /// Destination directory
        dest_dir: String,
    },

    /// Placement failed.
    PlacementFailed {
        /// Associated ticket
        ticket_id: String,
        /// Placement job ID
        job_id: String,
        /// File that failed (if applicable)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failed_file: Option<String>,
        /// Error message
        error: String,
        /// Files completed before failure
        files_completed: usize,
    },

    /// Placement was rolled back after a failure.
    PlacementRolledBack {
        /// Associated ticket
        ticket_id: String,
        /// Placement job ID
        job_id: String,
        /// Files that were rolled back (removed)
        files_removed: usize,
        /// Directories that were rolled back (removed)
        directories_removed: usize,
        /// Whether rollback was successful
        success: bool,
        /// Any errors during rollback
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        errors: Vec<String>,
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
            Self::TicketDeleted { .. } => "ticket_deleted",
            Self::SearchExecuted { .. } => "search_executed",
            Self::IndexerRateLimitUpdated { .. } => "indexer_rate_limit_updated",
            Self::IndexerEnabledChanged { .. } => "indexer_enabled_changed",
            Self::TorrentAdded { .. } => "torrent_added",
            Self::TorrentRemoved { .. } => "torrent_removed",
            Self::TorrentPaused { .. } => "torrent_paused",
            Self::TorrentResumed { .. } => "torrent_resumed",
            Self::TorrentLimitChanged { .. } => "torrent_limit_changed",
            Self::TorrentRechecked { .. } => "torrent_rechecked",
            // Acquisition events
            Self::AcquisitionStarted { .. } => "acquisition_started",
            Self::QueryBuildingStarted { .. } => "query_building_started",
            Self::QueryBuildingCompleted { .. } => "query_building_completed",
            Self::SearchStarted { .. } => "search_started",
            Self::SearchCompleted { .. } => "search_completed",
            Self::ScoringStarted { .. } => "scoring_started",
            Self::ScoringCompleted { .. } => "scoring_completed",
            Self::LlmCallStarted { .. } => "llm_call_started",
            Self::LlmCallCompleted { .. } => "llm_call_completed",
            Self::LlmCallFailed { .. } => "llm_call_failed",
            Self::AcquisitionCompleted { .. } => "acquisition_completed",
            Self::FallbackSearchStarted { .. } => "fallback_search_started",
            Self::FallbackSearchCompleted { .. } => "fallback_search_completed",
            Self::DiscographyCandidateScored { .. } => "discography_candidate_scored",
            Self::AlbumFoundInDiscography { .. } => "album_found_in_discography",
            Self::QueriesGenerated { .. } => "queries_generated",
            Self::CandidatesScored { .. } => "candidates_scored",
            Self::CandidateSelected { .. } => "candidate_selected",
            Self::TrainingQueryContext { .. } => "training_query_context",
            Self::TrainingScoringContext { .. } => "training_scoring_context",
            Self::TrainingFileMappingContext { .. } => "training_file_mapping_context",
            Self::UserCorrection { .. } => "user_correction",
            // Phase 4 events
            Self::ConversionStarted { .. } => "conversion_started",
            Self::ConversionProgress { .. } => "conversion_progress",
            Self::ConversionCompleted { .. } => "conversion_completed",
            Self::ConversionFailed { .. } => "conversion_failed",
            Self::PlacementStarted { .. } => "placement_started",
            Self::PlacementProgress { .. } => "placement_progress",
            Self::PlacementCompleted { .. } => "placement_completed",
            Self::PlacementFailed { .. } => "placement_failed",
            Self::PlacementRolledBack { .. } => "placement_rolled_back",
        }
    }

    /// Extract ticket_id if this event is ticket-related
    pub fn ticket_id(&self) -> Option<&str> {
        match self {
            Self::TicketCreated { ticket_id, .. }
            | Self::TicketStateChanged { ticket_id, .. }
            | Self::TicketCancelled { ticket_id, .. }
            | Self::TicketDeleted { ticket_id, .. }
            // Acquisition events
            | Self::AcquisitionStarted { ticket_id, .. }
            | Self::QueryBuildingStarted { ticket_id, .. }
            | Self::QueryBuildingCompleted { ticket_id, .. }
            | Self::SearchStarted { ticket_id, .. }
            | Self::SearchCompleted { ticket_id, .. }
            | Self::ScoringStarted { ticket_id, .. }
            | Self::ScoringCompleted { ticket_id, .. }
            | Self::AcquisitionCompleted { ticket_id, .. }
            | Self::FallbackSearchStarted { ticket_id, .. }
            | Self::FallbackSearchCompleted { ticket_id, .. }
            | Self::DiscographyCandidateScored { ticket_id, .. }
            | Self::AlbumFoundInDiscography { ticket_id, .. }
            | Self::QueriesGenerated { ticket_id, .. }
            | Self::CandidatesScored { ticket_id, .. }
            | Self::CandidateSelected { ticket_id, .. }
            | Self::TrainingQueryContext { ticket_id, .. }
            | Self::TrainingScoringContext { ticket_id, .. }
            | Self::TrainingFileMappingContext { ticket_id, .. }
            | Self::UserCorrection { ticket_id, .. }
            // Phase 4 events
            | Self::ConversionStarted { ticket_id, .. }
            | Self::ConversionProgress { ticket_id, .. }
            | Self::ConversionCompleted { ticket_id, .. }
            | Self::ConversionFailed { ticket_id, .. }
            | Self::PlacementStarted { ticket_id, .. }
            | Self::PlacementProgress { ticket_id, .. }
            | Self::PlacementCompleted { ticket_id, .. }
            | Self::PlacementFailed { ticket_id, .. }
            | Self::PlacementRolledBack { ticket_id, .. } => Some(ticket_id),
            Self::TorrentAdded { ticket_id, .. } => ticket_id.as_deref(),
            // LLM events have optional ticket_id
            Self::LlmCallStarted { ticket_id, .. }
            | Self::LlmCallCompleted { ticket_id, .. }
            | Self::LlmCallFailed { ticket_id, .. } => ticket_id.as_deref(),
            _ => None,
        }
    }

    /// Extract user_id if this event was triggered by a user action
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::TicketCreated { requested_by, .. } => Some(requested_by),
            Self::TicketCancelled { cancelled_by, .. } => Some(cancelled_by),
            Self::TicketDeleted { deleted_by, .. } => Some(deleted_by),
            Self::CandidateSelected { selected_by, .. } => Some(selected_by),
            Self::SearchExecuted { user_id, .. }
            | Self::IndexerRateLimitUpdated { user_id, .. }
            | Self::IndexerEnabledChanged { user_id, .. }
            | Self::TorrentAdded { user_id, .. }
            | Self::TorrentRemoved { user_id, .. }
            | Self::TorrentPaused { user_id, .. }
            | Self::TorrentResumed { user_id, .. }
            | Self::TorrentLimitChanged { user_id, .. }
            | Self::TorrentRechecked { user_id, .. }
            | Self::UserCorrection { user_id, .. } => Some(user_id),
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
