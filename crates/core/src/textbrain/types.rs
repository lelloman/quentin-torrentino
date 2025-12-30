//! Shared types for TextBrain operations.

use serde::{Deserialize, Serialize};

use crate::searcher::TorrentCandidate;
use crate::textbrain::LlmUsage;

/// Result of query building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryBuildResult {
    /// Generated search queries, ordered by priority.
    pub queries: Vec<String>,
    /// Method used: "dumb", "llm", or "dumb_then_llm".
    pub method: String,
    /// Confidence in the queries (0.0-1.0).
    /// Higher means more likely to find good results.
    pub confidence: f32,
    /// LLM token usage (if LLM was used).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_usage: Option<LlmUsage>,
}

/// A scored torrent candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredCandidate {
    /// The torrent candidate.
    pub candidate: TorrentCandidate,
    /// Match score (0.0-1.0).
    /// 1.0 = perfect match, 0.0 = no match.
    pub score: f32,
    /// Human-readable reasoning for the score.
    pub reasoning: String,
    /// File mappings (which torrent files match which ticket items).
    /// Empty until file mapping is implemented.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_mappings: Vec<FileMapping>,
}

/// Mapping of a torrent file to a ticket item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileMapping {
    /// Path of the file within the torrent.
    pub torrent_file_path: String,
    /// ID of the ticket item this file matches.
    pub ticket_item_id: String,
    /// Confidence in this mapping (0.0-1.0).
    pub confidence: f32,
}

/// Result of candidate matching/scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    /// Scored candidates, sorted by score descending.
    pub candidates: Vec<ScoredCandidate>,
    /// Method used: "dumb", "llm", or "dumb_then_llm".
    pub method: String,
    /// LLM token usage (if LLM was used).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_usage: Option<LlmUsage>,
}

/// Result of the full acquisition process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionResult {
    /// Best matching candidate (if any found above threshold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_candidate: Option<ScoredCandidate>,
    /// All scored candidates from all queries.
    pub all_candidates: Vec<ScoredCandidate>,
    /// Queries that were tried.
    pub queries_tried: Vec<String>,
    /// Total candidates evaluated across all queries.
    pub candidates_evaluated: u32,
    /// Method used for query building.
    pub query_method: String,
    /// Method used for scoring.
    pub score_method: String,
    /// Whether the best candidate was auto-approved (score >= threshold).
    pub auto_approved: bool,
    /// Total LLM token usage across all operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_usage: Option<LlmUsage>,
    /// Total duration of the acquisition process in milliseconds.
    pub duration_ms: u64,
}

/// Summary of a scored candidate for storage in ticket state.
/// Lighter than full ScoredCandidate - doesn't include full TorrentCandidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoredCandidateSummary {
    /// Torrent title.
    pub title: String,
    /// Info hash for identification.
    pub info_hash: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Seeder count.
    pub seeders: u32,
    /// Match score (0.0-1.0).
    pub score: f32,
    /// Brief reasoning.
    pub reasoning: String,
}

impl From<&ScoredCandidate> for ScoredCandidateSummary {
    fn from(sc: &ScoredCandidate) -> Self {
        Self {
            title: sc.candidate.title.clone(),
            info_hash: sc.candidate.info_hash.clone(),
            size_bytes: sc.candidate.size_bytes,
            seeders: sc.candidate.seeders,
            score: sc.score,
            reasoning: sc.reasoning.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::{TorrentCandidate, TorrentSource};

    fn make_candidate(title: &str, score: f32) -> ScoredCandidate {
        ScoredCandidate {
            candidate: TorrentCandidate {
                title: title.to_string(),
                info_hash: "abc123".to_string(),
                size_bytes: 1024,
                seeders: 10,
                leechers: 5,
                category: None,
                publish_date: None,
                files: None,
                sources: vec![TorrentSource {
                    indexer: "test".to_string(),
                    magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
                    torrent_url: None,
                    seeders: 10,
                    leechers: 5,
                    details_url: None,
                }],
                from_cache: false,
            },
            score,
            reasoning: "Test reasoning".to_string(),
            file_mappings: vec![],
        }
    }

    #[test]
    fn test_query_build_result_serialization() {
        let result = QueryBuildResult {
            queries: vec!["query1".to_string(), "query2".to_string()],
            method: "dumb".to_string(),
            confidence: 0.8,
            llm_usage: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("llm_usage")); // None should be skipped

        let parsed: QueryBuildResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.queries.len(), 2);
        assert_eq!(parsed.method, "dumb");
        assert_eq!(parsed.confidence, 0.8);
    }

    #[test]
    fn test_scored_candidate_summary_from() {
        let candidate = make_candidate("Test Torrent", 0.95);
        let summary = ScoredCandidateSummary::from(&candidate);

        assert_eq!(summary.title, "Test Torrent");
        assert_eq!(summary.info_hash, "abc123");
        assert_eq!(summary.score, 0.95);
    }

    #[test]
    fn test_acquisition_result_serialization() {
        let result = AcquisitionResult {
            best_candidate: None,
            all_candidates: vec![],
            queries_tried: vec!["test query".to_string()],
            candidates_evaluated: 5,
            query_method: "dumb".to_string(),
            score_method: "dumb".to_string(),
            auto_approved: false,
            llm_usage: None,
            duration_ms: 1500,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: AcquisitionResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.queries_tried, vec!["test query"]);
        assert_eq!(parsed.candidates_evaluated, 5);
        assert!(!parsed.auto_approved);
    }

    #[test]
    fn test_file_mapping_serialization() {
        let mapping = FileMapping {
            torrent_file_path: "Album/01 - Track.flac".to_string(),
            ticket_item_id: "track-001".to_string(),
            confidence: 0.92,
        };

        let json = serde_json::to_string(&mapping).unwrap();
        let parsed: FileMapping = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.torrent_file_path, "Album/01 - Track.flac");
        assert_eq!(parsed.ticket_item_id, "track-001");
        assert_eq!(parsed.confidence, 0.92);
    }
}
