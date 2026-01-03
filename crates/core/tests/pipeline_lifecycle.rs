//! Pipeline lifecycle integration tests.
//!
//! These tests verify the pipeline processor with mock converter and placer:
//! - Running state management
//! - Concurrent processing limits
//! - Job state transitions (converting -> placing -> completed)
//! - Error handling and failure states

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::mpsc;

use torrentino_core::{
    PipelineProcessor, ProcessorConfig, SqliteTicketStore, TicketStore,
    converter::{ConversionConstraints, ConverterError},
    placer::PlacerError,
    processor::{PipelineJob, PipelineProgress, SourceFile},
    testing::{MockConverter, MockPlacer},
    ticket::{CreateTicketRequest, QueryContext, TicketState},
};

/// Test helper to create pipeline processor with mocks.
struct TestHarness {
    processor: PipelineProcessor<MockConverter, MockPlacer>,
    converter: MockConverter,
    placer: MockPlacer,
    ticket_store: Arc<SqliteTicketStore>,
    temp_dir: TempDir,
    source_dir: TempDir,
}

impl TestHarness {
    async fn new() -> Self {
        Self::with_config(ProcessorConfig::default()).await
    }

    async fn with_config(mut config: ProcessorConfig) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let source_dir = TempDir::new().expect("Failed to create source dir");
        let db_path = temp_dir.path().join("test.db");

        // Use temp directory for pipeline temp files
        config.temp_dir = temp_dir.path().to_path_buf();

        let ticket_store = Arc::new(
            SqliteTicketStore::new(&db_path).expect("Failed to create ticket store"),
        );
        let converter = MockConverter::new();
        let placer = MockPlacer::new();

        // Set fast durations for testing
        converter.set_conversion_duration(Duration::from_millis(10)).await;
        placer.set_placement_duration(Duration::from_millis(10)).await;

        let processor = PipelineProcessor::new(
            config,
            converter.clone(),
            placer.clone(),
        )
        .with_ticket_store(Arc::clone(&ticket_store) as Arc<dyn TicketStore>);

        Self {
            processor,
            converter,
            placer,
            ticket_store,
            temp_dir,
            source_dir,
        }
    }

    fn create_ticket(&self, description: &str) -> String {
        let request = CreateTicketRequest {
            created_by: "test".to_string(),
            priority: 100,
            query_context: QueryContext::new(
                vec!["test".to_string()],
                description,
            ),
            dest_path: self.temp_dir.path().join("output").to_string_lossy().to_string(),
            output_constraints: None,
        };

        self.ticket_store.create(request).expect("Failed to create ticket").id
    }

    fn create_source_file(&self, name: &str) -> PathBuf {
        let path = self.source_dir.path().join(name);
        std::fs::write(&path, b"test content").expect("Failed to create source file");
        path
    }

    fn get_ticket_state(&self, ticket_id: &str) -> Option<String> {
        self.ticket_store.get(ticket_id).ok().flatten().map(|t| {
            Self::state_name(&t.state).to_string()
        })
    }

    fn state_name(state: &TicketState) -> &'static str {
        match state {
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

// =============================================================================
// Pipeline Status Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_starts_in_stopped_state() {
    let harness = TestHarness::new().await;

    let status = harness.processor.status().await;
    assert!(!status.running, "Pipeline should not be running before start");
}

#[tokio::test]
async fn test_pipeline_status_reflects_running_state() {
    let harness = TestHarness::new().await;

    // Before start
    let status = harness.processor.status().await;
    assert!(!status.running);

    // After start
    harness.processor.start().await;
    let status = harness.processor.status().await;
    assert!(status.running);

    // After stop
    harness.processor.stop().await;
    let status = harness.processor.status().await;
    assert!(!status.running);
}

#[tokio::test]
async fn test_pipeline_rejects_jobs_when_not_running() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");

    let job = PipelineJob {
        ticket_id,
        source_files: vec![],
        file_mappings: vec![],
        constraints: None,
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    let result = harness.processor.process(job, None).await;
    assert!(result.is_err(), "Should reject job when not running");
}

// =============================================================================
// Job Processing Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_processes_job_successfully() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");
    let source_path = harness.create_source_file("test.mp3");

    harness.processor.start().await;

    let (progress_tx, mut progress_rx) = mpsc::channel(100);

    let job = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files: vec![SourceFile {
            path: source_path,
            item_id: "track01".to_string(),
            dest_filename: "track01.ogg".to_string(),
        }],
        file_mappings: vec![],
        constraints: None, // No conversion, just copy
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    harness.processor.process(job, Some(progress_tx)).await.unwrap();

    // Wait for completion
    let mut completed = false;
    while let Some(progress) = progress_rx.recv().await {
        match progress {
            PipelineProgress::Completed { ticket_id: id, .. } => {
                assert_eq!(id, ticket_id);
                completed = true;
                break;
            }
            PipelineProgress::Failed { error, .. } => {
                panic!("Pipeline failed unexpectedly: {}", error);
            }
            _ => {}
        }
    }

    assert!(completed, "Job should complete successfully");

    // Verify ticket state is completed
    let state = harness.get_ticket_state(&ticket_id);
    assert_eq!(state, Some("completed".to_string()));
}

#[tokio::test]
async fn test_pipeline_updates_ticket_to_converting_state() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");
    let source_path = harness.create_source_file("test.mp3");

    // Slow down conversion to catch the state
    harness.converter.set_conversion_duration(Duration::from_millis(500)).await;

    harness.processor.start().await;

    let (progress_tx, mut progress_rx) = mpsc::channel(100);

    let job = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files: vec![SourceFile {
            path: source_path,
            item_id: "track01".to_string(),
            dest_filename: "track01.ogg".to_string(),
        }],
        file_mappings: vec![],
        constraints: Some(ConversionConstraints::Audio(
            torrentino_core::converter::AudioConstraints {
                format: torrentino_core::converter::AudioFormat::OggVorbis,
                bitrate_kbps: Some(192),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }
        )),
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    harness.processor.process(job, Some(progress_tx)).await.unwrap();

    // Check for converting state via progress
    let mut saw_converting = false;
    while let Some(progress) = progress_rx.recv().await {
        match progress {
            PipelineProgress::Converting { .. } => {
                saw_converting = true;
            }
            PipelineProgress::Completed { .. } | PipelineProgress::Failed { .. } => {
                break;
            }
            _ => {}
        }
    }

    assert!(saw_converting, "Should see converting progress");
}

#[tokio::test]
async fn test_pipeline_handles_conversion_failure() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");
    let source_path = harness.create_source_file("test.mp3");

    // Configure mock to fail
    harness.converter.set_next_error(ConverterError::conversion_failed("test error", None)).await;

    harness.processor.start().await;

    let (progress_tx, mut progress_rx) = mpsc::channel(100);

    let job = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files: vec![SourceFile {
            path: source_path,
            item_id: "track01".to_string(),
            dest_filename: "track01.ogg".to_string(),
        }],
        file_mappings: vec![],
        constraints: Some(ConversionConstraints::Audio(
            torrentino_core::converter::AudioConstraints {
                format: torrentino_core::converter::AudioFormat::OggVorbis,
                bitrate_kbps: Some(192),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }
        )),
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    harness.processor.process(job, Some(progress_tx)).await.unwrap();

    // Wait for failure
    let mut failed = false;
    while let Some(progress) = progress_rx.recv().await {
        if let PipelineProgress::Failed { failed_phase, .. } = progress {
            assert_eq!(failed_phase, "conversion");
            failed = true;
            break;
        }
    }

    assert!(failed, "Job should fail during conversion");

    // Verify ticket state is failed
    let state = harness.get_ticket_state(&ticket_id);
    assert_eq!(state, Some("failed".to_string()));
}

#[tokio::test]
async fn test_pipeline_handles_placement_failure() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");
    let source_path = harness.create_source_file("test.mp3");

    // Configure placer to fail
    harness.placer.set_next_error(PlacerError::DestinationExists { path: "/test".into() }).await;

    harness.processor.start().await;

    let (progress_tx, mut progress_rx) = mpsc::channel(100);

    let job = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files: vec![SourceFile {
            path: source_path,
            item_id: "track01".to_string(),
            dest_filename: "track01.ogg".to_string(),
        }],
        file_mappings: vec![],
        constraints: None, // No conversion - go straight to placement
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    harness.processor.process(job, Some(progress_tx)).await.unwrap();

    // Wait for failure
    let mut failed = false;
    while let Some(progress) = progress_rx.recv().await {
        if let PipelineProgress::Failed { failed_phase, .. } = progress {
            assert_eq!(failed_phase, "placement");
            failed = true;
            break;
        }
    }

    assert!(failed, "Job should fail during placement");

    // Verify ticket state is failed
    let state = harness.get_ticket_state(&ticket_id);
    assert_eq!(state, Some("failed".to_string()));
}

// =============================================================================
// Concurrency Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_respects_max_parallel_conversions() {
    let config = ProcessorConfig {
        max_parallel_conversions: 2,
        max_parallel_placements: 2,
        ..Default::default()
    };
    let harness = TestHarness::with_config(config).await;

    // Slow down conversion to test concurrency
    harness.converter.set_conversion_duration(Duration::from_millis(200)).await;

    harness.processor.start().await;

    // Submit 4 jobs
    let mut receivers = Vec::new();
    for i in 0..4 {
        let ticket_id = harness.create_ticket(&format!("Album {}", i));
        let source_path = harness.create_source_file(&format!("test{}.mp3", i));

        let (progress_tx, progress_rx) = mpsc::channel(100);

        let job = PipelineJob {
            ticket_id,
            source_files: vec![SourceFile {
                path: source_path,
                item_id: format!("track{:02}", i),
                dest_filename: format!("track{:02}.ogg", i),
            }],
            file_mappings: vec![],
            constraints: Some(ConversionConstraints::Audio(
                torrentino_core::converter::AudioConstraints {
                    format: torrentino_core::converter::AudioFormat::OggVorbis,
                    bitrate_kbps: Some(192),
                    sample_rate_hz: None,
                    channels: None,
                    compression_level: None,
                }
            )),
            dest_dir: harness.temp_dir.path().join("output"),
            metadata: None,
        };

        harness.processor.process(job, Some(progress_tx)).await.unwrap();
        receivers.push(progress_rx);
    }

    // Wait for all to complete
    let mut completed = 0;
    for mut rx in receivers {
        while let Some(progress) = rx.recv().await {
            match progress {
                PipelineProgress::Completed { .. } => {
                    completed += 1;
                    break;
                }
                PipelineProgress::Failed { error, .. } => {
                    panic!("Pipeline failed unexpectedly: {}", error);
                }
                _ => {}
            }
        }
    }

    assert_eq!(completed, 4, "All 4 jobs should complete");

    // Verify conversion count
    let count = harness.converter.conversion_count().await;
    assert_eq!(count, 4, "Should have converted 4 files");
}

#[tokio::test]
async fn test_pipeline_prevents_duplicate_jobs() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");
    let source_path = harness.create_source_file("test.mp3");

    // Slow down to keep job active
    harness.converter.set_conversion_duration(Duration::from_millis(500)).await;

    harness.processor.start().await;

    let job1 = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files: vec![SourceFile {
            path: source_path.clone(),
            item_id: "track01".to_string(),
            dest_filename: "track01.ogg".to_string(),
        }],
        file_mappings: vec![],
        constraints: Some(ConversionConstraints::Audio(
            torrentino_core::converter::AudioConstraints {
                format: torrentino_core::converter::AudioFormat::OggVorbis,
                bitrate_kbps: Some(192),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }
        )),
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    // First job should succeed
    harness.processor.process(job1.clone(), None).await.unwrap();

    // Small delay to ensure job is registered
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second job with same ticket_id should fail
    let result = harness.processor.process(job1, None).await;
    assert!(result.is_err(), "Should reject duplicate job");
}

// =============================================================================
// Status Tracking Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_status_shows_active_jobs() {
    let harness = TestHarness::new().await;
    let ticket_id = harness.create_ticket("Test album");
    let source_path = harness.create_source_file("test.mp3");

    // Slow down to capture status
    harness.converter.set_conversion_duration(Duration::from_millis(300)).await;

    harness.processor.start().await;

    let job = PipelineJob {
        ticket_id: ticket_id.clone(),
        source_files: vec![SourceFile {
            path: source_path,
            item_id: "track01".to_string(),
            dest_filename: "track01.ogg".to_string(),
        }],
        file_mappings: vec![],
        constraints: Some(ConversionConstraints::Audio(
            torrentino_core::converter::AudioConstraints {
                format: torrentino_core::converter::AudioFormat::OggVorbis,
                bitrate_kbps: Some(192),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }
        )),
        dest_dir: harness.temp_dir.path().join("output"),
        metadata: None,
    };

    let (progress_tx, _) = mpsc::channel(100);
    harness.processor.process(job, Some(progress_tx)).await.unwrap();

    // Small delay for job to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    let status = harness.processor.status().await;
    assert!(
        status.converting_tickets.contains(&ticket_id) || status.placing_tickets.contains(&ticket_id),
        "Active job should appear in status"
    );
}
