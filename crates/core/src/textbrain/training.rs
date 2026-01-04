//! Training data collection for LLM fine-tuning.
//!
//! This module provides utilities for collecting training data from TextBrain
//! operations. The data can be used to fine-tune smaller models for local inference.
//!
//! # Data Types Collected
//!
//! 1. **Query Generation**: Input context → Generated queries
//! 2. **Candidate Scoring**: Candidates → Scores and rankings
//! 3. **File Mapping**: Expected content + Files → Mappings
//! 4. **User Corrections**: When users select different candidates than recommended

use uuid::Uuid;

use crate::audit::{AuditEvent, TrainingCandidate, TrainingFile, TrainingFileMapping};
use crate::searcher::TorrentCandidate;
use crate::textbrain::types::{
    AcquisitionResult, FileMapping, MatchResult, QueryBuildResult, ScoredCandidate,
};
use crate::ticket::{ExpectedContent, QueryContext};

/// Generate a unique sample ID for training data.
fn generate_sample_id() -> String {
    Uuid::new_v4().to_string()
}

/// Create a training event for query generation.
pub fn create_query_training_event(
    ticket_id: &str,
    context: &QueryContext,
    result: &QueryBuildResult,
) -> AuditEvent {
    AuditEvent::TrainingQueryContext {
        sample_id: generate_sample_id(),
        ticket_id: ticket_id.to_string(),
        input_tags: context.tags.clone(),
        input_description: context.description.clone(),
        input_expected: context
            .expected
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok()),
        output_queries: result.queries.clone(),
        method: result.method.clone(),
        confidence: result.confidence,
        success: None, // Will be filled in later when we know the outcome
    }
}

/// Create a training event for candidate scoring.
pub fn create_scoring_training_event(
    ticket_id: &str,
    context: &QueryContext,
    result: &MatchResult,
) -> AuditEvent {
    let candidates: Vec<TrainingCandidate> = result
        .candidates
        .iter()
        .map(|sc| TrainingCandidate {
            title: sc.candidate.title.clone(),
            hash: sc.candidate.info_hash.clone(),
            size_bytes: sc.candidate.size_bytes,
            seeders: sc.candidate.seeders,
            category: sc.candidate.category.clone(),
        })
        .collect();

    let scores: Vec<f32> = result.candidates.iter().map(|sc| sc.score).collect();

    AuditEvent::TrainingScoringContext {
        sample_id: generate_sample_id(),
        ticket_id: ticket_id.to_string(),
        input_description: context.description.clone(),
        input_expected: context
            .expected
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok()),
        input_candidates: candidates,
        output_recommended_idx: 0, // First candidate is always recommended
        output_scores: scores,
        method: result.method.clone(),
    }
}

/// Create a training event for file mapping.
pub fn create_file_mapping_training_event(
    ticket_id: &str,
    expected: &ExpectedContent,
    candidate: &TorrentCandidate,
    mappings: &[FileMapping],
    quality: f32,
) -> Option<AuditEvent> {
    // Only create if we have files and mappings
    let files = candidate.files.as_ref()?;

    if files.is_empty() {
        return None;
    }

    let training_files: Vec<TrainingFile> = files
        .iter()
        .map(|f| TrainingFile {
            path: f.path.clone(),
            size_bytes: f.size_bytes,
        })
        .collect();

    let training_mappings: Vec<TrainingFileMapping> = mappings
        .iter()
        .map(|m| TrainingFileMapping {
            file_path: m.torrent_file_path.clone(),
            item_id: m.ticket_item_id.clone(),
            confidence: m.confidence,
        })
        .collect();

    Some(AuditEvent::TrainingFileMappingContext {
        sample_id: generate_sample_id(),
        ticket_id: ticket_id.to_string(),
        input_expected: serde_json::to_string(expected).ok()?,
        input_files: training_files,
        output_mappings: training_mappings,
        quality,
    })
}

/// Create a training event for user correction.
///
/// This is called when a user selects a different candidate than the one
/// that was automatically recommended.
pub fn create_user_correction_event(
    ticket_id: &str,
    user_id: &str,
    context: &QueryContext,
    candidates: &[ScoredCandidate],
    recommended_idx: usize,
    selected_idx: usize,
) -> AuditEvent {
    let training_candidates: Vec<TrainingCandidate> = candidates
        .iter()
        .map(|sc| TrainingCandidate {
            title: sc.candidate.title.clone(),
            hash: sc.candidate.info_hash.clone(),
            size_bytes: sc.candidate.size_bytes,
            seeders: sc.candidate.seeders,
            category: sc.candidate.category.clone(),
        })
        .collect();

    AuditEvent::UserCorrection {
        ticket_id: ticket_id.to_string(),
        recommended_idx,
        selected_idx,
        context_description: context.description.clone(),
        expected_content: context
            .expected
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok()),
        candidates: training_candidates,
        user_id: user_id.to_string(),
    }
}

/// Create training events from a full acquisition result.
///
/// Returns multiple events: query context, scoring context, and optionally
/// file mapping contexts for top candidates.
pub fn create_acquisition_training_events(
    ticket_id: &str,
    context: &QueryContext,
    result: &AcquisitionResult,
) -> Vec<AuditEvent> {
    let mut events = Vec::new();

    // Query context event
    events.push(AuditEvent::TrainingQueryContext {
        sample_id: generate_sample_id(),
        ticket_id: ticket_id.to_string(),
        input_tags: context.tags.clone(),
        input_description: context.description.clone(),
        input_expected: context
            .expected
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok()),
        output_queries: result.queries_tried.clone(),
        method: result.query_method.clone(),
        confidence: if result.best_candidate.is_some() {
            result.best_candidate.as_ref().unwrap().score
        } else {
            0.0
        },
        success: Some(result.best_candidate.is_some() && result.auto_approved),
    });

    // Scoring context event (if we have candidates)
    if !result.all_candidates.is_empty() {
        let candidates: Vec<TrainingCandidate> = result
            .all_candidates
            .iter()
            .take(10) // Limit to top 10 for training data size
            .map(|sc| TrainingCandidate {
                title: sc.candidate.title.clone(),
                hash: sc.candidate.info_hash.clone(),
                size_bytes: sc.candidate.size_bytes,
                seeders: sc.candidate.seeders,
                category: sc.candidate.category.clone(),
            })
            .collect();

        let scores: Vec<f32> = result
            .all_candidates
            .iter()
            .take(10)
            .map(|sc| sc.score)
            .collect();

        events.push(AuditEvent::TrainingScoringContext {
            sample_id: generate_sample_id(),
            ticket_id: ticket_id.to_string(),
            input_description: context.description.clone(),
            input_expected: context
                .expected
                .as_ref()
                .and_then(|e| serde_json::to_string(e).ok()),
            input_candidates: candidates,
            output_recommended_idx: 0,
            output_scores: scores,
            method: result.score_method.clone(),
        });
    }

    // File mapping events for top candidates with files
    if let Some(expected) = &context.expected {
        for candidate in result.all_candidates.iter().take(3) {
            if !candidate.file_mappings.is_empty() {
                let quality =
                    crate::textbrain::calculate_mapping_quality(&candidate.file_mappings, expected);

                if let Some(event) = create_file_mapping_training_event(
                    ticket_id,
                    expected,
                    &candidate.candidate,
                    &candidate.file_mappings,
                    quality,
                ) {
                    events.push(event);
                }
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::{TorrentFile, TorrentSource};
    use crate::ticket::ExpectedTrack;

    fn make_context() -> QueryContext {
        QueryContext::new(
            vec!["music".to_string(), "flac".to_string()],
            "Abbey Road by The Beatles",
        )
        .with_expected(ExpectedContent::album_by(
            "The Beatles",
            "Abbey Road",
            vec![
                ExpectedTrack::new(1, "Come Together"),
                ExpectedTrack::new(2, "Something"),
            ],
        ))
    }

    fn make_candidate(title: &str, score: f32) -> ScoredCandidate {
        ScoredCandidate {
            candidate: TorrentCandidate {
                title: title.to_string(),
                info_hash: format!("hash_{}", title.replace(' ', "_")),
                size_bytes: 500_000_000,
                seeders: 50,
                leechers: 5,
                category: Some("Music".to_string()),
                publish_date: None,
                files: Some(vec![
                    TorrentFile {
                        path: "01 - Come Together.flac".to_string(),
                        size_bytes: 30_000_000,
                    },
                    TorrentFile {
                        path: "02 - Something.flac".to_string(),
                        size_bytes: 25_000_000,
                    },
                ]),
                sources: vec![TorrentSource {
                    indexer: "test".to_string(),
                    magnet_uri: Some("magnet:?xt=urn:btih:abc".to_string()),
                    torrent_url: None,
                    seeders: 50,
                    leechers: 5,
                    details_url: None,
                }],
                from_cache: false,
            },
            score,
            reasoning: "Test match".to_string(),
            file_mappings: vec![
                FileMapping {
                    torrent_file_path: "01 - Come Together.flac".to_string(),
                    ticket_item_id: "track-1".to_string(),
                    confidence: 0.95,
                },
                FileMapping {
                    torrent_file_path: "02 - Something.flac".to_string(),
                    ticket_item_id: "track-2".to_string(),
                    confidence: 0.90,
                },
            ],
        }
    }

    #[test]
    fn test_create_query_training_event() {
        let context = make_context();
        let result = QueryBuildResult {
            queries: vec!["Beatles Abbey Road".to_string()],
            method: "dumb".to_string(),
            confidence: 0.85,
            llm_usage: None,
        };

        let event = create_query_training_event("ticket-123", &context, &result);

        if let AuditEvent::TrainingQueryContext {
            ticket_id,
            input_description,
            output_queries,
            method,
            confidence,
            ..
        } = event
        {
            assert_eq!(ticket_id, "ticket-123");
            assert!(input_description.contains("Beatles"));
            assert_eq!(output_queries.len(), 1);
            assert_eq!(method, "dumb");
            assert_eq!(confidence, 0.85);
        } else {
            panic!("Expected TrainingQueryContext event");
        }
    }

    #[test]
    fn test_create_scoring_training_event() {
        let context = make_context();
        let result = MatchResult {
            candidates: vec![
                make_candidate("Abbey Road - Beatles FLAC", 0.95),
                make_candidate("Beatles Greatest Hits", 0.65),
            ],
            method: "dumb".to_string(),
            llm_usage: None,
        };

        let event = create_scoring_training_event("ticket-123", &context, &result);

        if let AuditEvent::TrainingScoringContext {
            input_candidates,
            output_scores,
            output_recommended_idx,
            ..
        } = event
        {
            assert_eq!(input_candidates.len(), 2);
            assert_eq!(output_scores, vec![0.95, 0.65]);
            assert_eq!(output_recommended_idx, 0);
        } else {
            panic!("Expected TrainingScoringContext event");
        }
    }

    #[test]
    fn test_create_user_correction_event() {
        let context = make_context();
        let candidates = vec![
            make_candidate("Wrong Album", 0.90),
            make_candidate("Correct Album", 0.85),
        ];

        let event = create_user_correction_event(
            "ticket-123",
            "user-456",
            &context,
            &candidates,
            0, // recommended
            1, // user selected
        );

        if let AuditEvent::UserCorrection {
            ticket_id,
            user_id,
            recommended_idx,
            selected_idx,
            candidates: training_candidates,
            ..
        } = event
        {
            assert_eq!(ticket_id, "ticket-123");
            assert_eq!(user_id, "user-456");
            assert_eq!(recommended_idx, 0);
            assert_eq!(selected_idx, 1);
            assert_eq!(training_candidates.len(), 2);
        } else {
            panic!("Expected UserCorrection event");
        }
    }

    #[test]
    fn test_create_file_mapping_training_event() {
        let expected = ExpectedContent::album_by(
            "The Beatles",
            "Abbey Road",
            vec![ExpectedTrack::new(1, "Come Together")],
        );

        let candidate = make_candidate("Abbey Road", 0.9);
        let mappings = &candidate.file_mappings;

        let event = create_file_mapping_training_event(
            "ticket-123",
            &expected,
            &candidate.candidate,
            mappings,
            0.95,
        );

        assert!(event.is_some());

        if let Some(AuditEvent::TrainingFileMappingContext {
            input_files,
            output_mappings,
            quality,
            ..
        }) = event
        {
            assert_eq!(input_files.len(), 2);
            assert_eq!(output_mappings.len(), 2);
            assert_eq!(quality, 0.95);
        } else {
            panic!("Expected TrainingFileMappingContext event");
        }
    }

    #[test]
    fn test_create_acquisition_training_events() {
        let context = make_context();
        let result = AcquisitionResult {
            best_candidate: Some(make_candidate("Abbey Road FLAC", 0.95)),
            all_candidates: vec![
                make_candidate("Abbey Road FLAC", 0.95),
                make_candidate("Beatles Hits", 0.70),
            ],
            queries_tried: vec!["Beatles Abbey Road".to_string()],
            candidates_evaluated: 2,
            query_method: "dumb".to_string(),
            score_method: "dumb".to_string(),
            auto_approved: true,
            llm_usage: None,
            duration_ms: 150,
        };

        let events = create_acquisition_training_events("ticket-123", &context, &result);

        // Should have query context, scoring context, and file mapping events
        assert!(events.len() >= 2);

        let has_query_event = events
            .iter()
            .any(|e| matches!(e, AuditEvent::TrainingQueryContext { .. }));
        let has_scoring_event = events
            .iter()
            .any(|e| matches!(e, AuditEvent::TrainingScoringContext { .. }));

        assert!(has_query_event, "Should have query training event");
        assert!(has_scoring_event, "Should have scoring training event");
    }
}
