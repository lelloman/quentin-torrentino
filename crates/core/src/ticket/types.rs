//! Core ticket data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::converter::{AudioConstraints, VideoConstraints};
use crate::textbrain::{FileMapping, ScoredCandidateSummary};

/// Output format constraints for a ticket.
///
/// Specifies how downloaded files should be processed before placement.
/// If not specified, files are placed as-is without conversion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputConstraints {
    /// Keep original format - no conversion, just copy files.
    Original,

    /// Convert audio files to specified format.
    Audio(AudioConstraints),

    /// Convert video files to specified format.
    Video(VideoConstraints),
}

impl OutputConstraints {
    /// Create constraints for keeping original format.
    pub fn original() -> Self {
        Self::Original
    }

    /// Create constraints for audio conversion.
    pub fn audio(constraints: AudioConstraints) -> Self {
        Self::Audio(constraints)
    }

    /// Create constraints for video conversion.
    pub fn video(constraints: VideoConstraints) -> Self {
        Self::Video(constraints)
    }

    /// Returns true if conversion is needed.
    pub fn needs_conversion(&self) -> bool {
        !matches!(self, Self::Original)
    }

    /// Convert to ConversionConstraints for the pipeline.
    /// Returns None if Original (no conversion needed).
    pub fn to_conversion_constraints(&self) -> Option<crate::converter::ConversionConstraints> {
        match self {
            Self::Original => None,
            Self::Audio(a) => Some(crate::converter::ConversionConstraints::Audio(a.clone())),
            Self::Video(v) => Some(crate::converter::ConversionConstraints::Video(v.clone())),
        }
    }
}

/// Default failover round for backward compatibility.
fn default_failover_round() -> u8 {
    1
}

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

    /// Expected content structure for file validation.
    /// Used to verify torrent files match what we're looking for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<ExpectedContent>,
}

impl QueryContext {
    /// Create a new query context.
    pub fn new(tags: Vec<String>, description: impl Into<String>) -> Self {
        Self {
            tags,
            description: description.into(),
            expected: None,
        }
    }

    /// Create a query context with expected content.
    pub fn with_expected(mut self, expected: ExpectedContent) -> Self {
        self.expected = Some(expected);
        self
    }
}

/// Expected content structure for file validation.
///
/// Defines what files we expect to find in a torrent.
/// Used during scoring to validate the torrent matches our requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectedContent {
    /// Music album with expected tracks.
    Album {
        /// Artist name (optional, for matching).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artist: Option<String>,
        /// Album title.
        title: String,
        /// Expected tracks in order.
        tracks: Vec<ExpectedTrack>,
    },

    /// Single music track.
    Track {
        /// Artist name (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artist: Option<String>,
        /// Track title.
        title: String,
    },

    /// Movie file.
    Movie {
        /// Movie title.
        title: String,
        /// Release year (optional, for disambiguation).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        year: Option<u32>,
    },

    /// TV episode(s).
    TvEpisode {
        /// Series name.
        series: String,
        /// Season number.
        season: u32,
        /// Episode numbers (e.g., [1, 2, 3] for S01E01-03).
        episodes: Vec<u32>,
    },
}

/// Expected track in an album.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpectedTrack {
    /// Track number (1-indexed).
    pub number: u32,
    /// Track title.
    pub title: String,
    /// Expected duration in seconds (optional, for validation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u32>,
}

impl ExpectedTrack {
    /// Create a new expected track.
    pub fn new(number: u32, title: impl Into<String>) -> Self {
        Self {
            number,
            title: title.into(),
            duration_secs: None,
        }
    }

    /// Set expected duration.
    pub fn with_duration(mut self, secs: u32) -> Self {
        self.duration_secs = Some(secs);
        self
    }
}

impl ExpectedContent {
    /// Create an album expectation.
    pub fn album(title: impl Into<String>, tracks: Vec<ExpectedTrack>) -> Self {
        Self::Album {
            artist: None,
            title: title.into(),
            tracks,
        }
    }

    /// Create an album expectation with artist.
    pub fn album_by(artist: impl Into<String>, title: impl Into<String>, tracks: Vec<ExpectedTrack>) -> Self {
        Self::Album {
            artist: Some(artist.into()),
            title: title.into(),
            tracks,
        }
    }

    /// Create a single track expectation.
    pub fn track(title: impl Into<String>) -> Self {
        Self::Track {
            artist: None,
            title: title.into(),
        }
    }

    /// Create a single track expectation with artist.
    pub fn track_by(artist: impl Into<String>, title: impl Into<String>) -> Self {
        Self::Track {
            artist: Some(artist.into()),
            title: title.into(),
        }
    }

    /// Create a movie expectation.
    pub fn movie(title: impl Into<String>) -> Self {
        Self::Movie {
            title: title.into(),
            year: None,
        }
    }

    /// Create a movie expectation with year.
    pub fn movie_year(title: impl Into<String>, year: u32) -> Self {
        Self::Movie {
            title: title.into(),
            year: Some(year),
        }
    }

    /// Create a TV episode expectation.
    pub fn tv_episode(series: impl Into<String>, season: u32, episode: u32) -> Self {
        Self::TvEpisode {
            series: series.into(),
            season,
            episodes: vec![episode],
        }
    }

    /// Create a TV episode range expectation.
    pub fn tv_episodes(series: impl Into<String>, season: u32, episodes: Vec<u32>) -> Self {
        Self::TvEpisode {
            series: series.into(),
            season,
            episodes,
        }
    }

    /// Get the expected file count.
    pub fn expected_file_count(&self) -> usize {
        match self {
            ExpectedContent::Album { tracks, .. } => tracks.len(),
            ExpectedContent::Track { .. } => 1,
            ExpectedContent::Movie { .. } => 1,
            ExpectedContent::TvEpisode { episodes, .. } => episodes.len(),
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
    /// File mappings from acquisition (which torrent files match which ticket items).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_mappings: Vec<FileMapping>,
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
        /// All candidates for failover (selected is candidates[0]).
        #[serde(default)]
        candidates: Vec<SelectedCandidate>,
        confidence: f32,
        approved_at: DateTime<Utc>,
    },

    /// Manually approved by user/admin.
    Approved {
        selected: SelectedCandidate,
        /// All candidates for failover (selected is candidates[0]).
        #[serde(default)]
        candidates: Vec<SelectedCandidate>,
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
        /// Index of current candidate being tried (0-based).
        #[serde(default)]
        candidate_idx: usize,
        /// Current failover round (1, 2, or 3).
        #[serde(default = "default_failover_round")]
        failover_round: u8,
        /// Progress at last check (for stall detection).
        #[serde(default)]
        last_progress_pct: f32,
        /// When progress last changed (for stall detection).
        #[serde(default = "Utc::now")]
        last_progress_at: DateTime<Utc>,
        /// All candidates for failover.
        #[serde(default)]
        candidates: Vec<SelectedCandidate>,
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

    /// Output format constraints (None = keep original, no conversion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_constraints: Option<OutputConstraints>,

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
        let now = Utc::now();
        let state = TicketState::Downloading {
            info_hash: "abc123".to_string(),
            progress_pct: 45.5,
            speed_bps: 1_000_000,
            eta_secs: Some(120),
            started_at: now,
            candidate_idx: 0,
            failover_round: 1,
            last_progress_pct: 45.5,
            last_progress_at: now,
            candidates: vec![],
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

    // ========================================================================
    // ExpectedContent tests
    // ========================================================================

    #[test]
    fn test_expected_track_creation() {
        let track = ExpectedTrack::new(1, "Come Together");
        assert_eq!(track.number, 1);
        assert_eq!(track.title, "Come Together");
        assert!(track.duration_secs.is_none());

        let track_with_duration = ExpectedTrack::new(2, "Something").with_duration(180);
        assert_eq!(track_with_duration.duration_secs, Some(180));
    }

    #[test]
    fn test_expected_content_album() {
        let tracks = vec![
            ExpectedTrack::new(1, "Come Together"),
            ExpectedTrack::new(2, "Something"),
        ];
        let album = ExpectedContent::album("Abbey Road", tracks);

        if let ExpectedContent::Album { artist, title, tracks } = album {
            assert!(artist.is_none());
            assert_eq!(title, "Abbey Road");
            assert_eq!(tracks.len(), 2);
        } else {
            panic!("Expected Album variant");
        }
    }

    #[test]
    fn test_expected_content_album_with_artist() {
        let tracks = vec![ExpectedTrack::new(1, "Track 1")];
        let album = ExpectedContent::album_by("The Beatles", "Abbey Road", tracks);

        if let ExpectedContent::Album { artist, title, .. } = album {
            assert_eq!(artist, Some("The Beatles".to_string()));
            assert_eq!(title, "Abbey Road");
        } else {
            panic!("Expected Album variant");
        }
    }

    #[test]
    fn test_expected_content_track() {
        let track = ExpectedContent::track("Yesterday");
        if let ExpectedContent::Track { artist, title } = track {
            assert!(artist.is_none());
            assert_eq!(title, "Yesterday");
        } else {
            panic!("Expected Track variant");
        }
    }

    #[test]
    fn test_expected_content_movie() {
        let movie = ExpectedContent::movie_year("The Matrix", 1999);
        if let ExpectedContent::Movie { title, year } = movie {
            assert_eq!(title, "The Matrix");
            assert_eq!(year, Some(1999));
        } else {
            panic!("Expected Movie variant");
        }
    }

    #[test]
    fn test_expected_content_tv_episode() {
        let ep = ExpectedContent::tv_episode("Breaking Bad", 1, 1);
        if let ExpectedContent::TvEpisode { series, season, episodes } = ep {
            assert_eq!(series, "Breaking Bad");
            assert_eq!(season, 1);
            assert_eq!(episodes, vec![1]);
        } else {
            panic!("Expected TvEpisode variant");
        }
    }

    #[test]
    fn test_expected_content_tv_episodes_range() {
        let eps = ExpectedContent::tv_episodes("Breaking Bad", 1, vec![1, 2, 3]);
        if let ExpectedContent::TvEpisode { episodes, .. } = eps {
            assert_eq!(episodes, vec![1, 2, 3]);
        } else {
            panic!("Expected TvEpisode variant");
        }
    }

    #[test]
    fn test_expected_file_count() {
        let album = ExpectedContent::album("Test", vec![
            ExpectedTrack::new(1, "T1"),
            ExpectedTrack::new(2, "T2"),
            ExpectedTrack::new(3, "T3"),
        ]);
        assert_eq!(album.expected_file_count(), 3);

        let track = ExpectedContent::track("Single");
        assert_eq!(track.expected_file_count(), 1);

        let movie = ExpectedContent::movie("Film");
        assert_eq!(movie.expected_file_count(), 1);

        let episodes = ExpectedContent::tv_episodes("Show", 1, vec![1, 2, 3, 4]);
        assert_eq!(episodes.expected_file_count(), 4);
    }

    #[test]
    fn test_expected_content_serialization() {
        let album = ExpectedContent::album_by("The Beatles", "Abbey Road", vec![
            ExpectedTrack::new(1, "Come Together"),
            ExpectedTrack::new(2, "Something"),
        ]);

        let json = serde_json::to_string(&album).unwrap();
        assert!(json.contains("\"type\":\"album\""));
        assert!(json.contains("\"artist\":\"The Beatles\""));
        assert!(json.contains("Abbey Road"));

        let deserialized: ExpectedContent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, album);
    }

    #[test]
    fn test_query_context_with_expected() {
        let ctx = QueryContext::new(
            vec!["music".to_string(), "album".to_string(), "flac".to_string()],
            "Abbey Road by The Beatles",
        ).with_expected(ExpectedContent::album("Abbey Road", vec![
            ExpectedTrack::new(1, "Come Together"),
        ]));

        assert!(ctx.expected.is_some());
        assert_eq!(ctx.expected.as_ref().unwrap().expected_file_count(), 1);
    }

    #[test]
    fn test_query_context_expected_serialization() {
        let ctx = QueryContext::new(vec!["movie".to_string()], "The Matrix")
            .with_expected(ExpectedContent::movie_year("The Matrix", 1999));

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"expected\""));
        assert!(json.contains("\"type\":\"movie\""));

        let deserialized: QueryContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ctx);
    }

    #[test]
    fn test_query_context_no_expected_skips_serialization() {
        let ctx = QueryContext::new(vec!["test".to_string()], "description");
        let json = serde_json::to_string(&ctx).unwrap();
        // expected should be skipped when None
        assert!(!json.contains("expected"));
    }
}
