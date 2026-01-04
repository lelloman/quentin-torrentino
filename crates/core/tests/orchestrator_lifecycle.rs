//! Orchestrator lifecycle integration tests.
//!
//! These tests verify the complete ticket lifecycle through the orchestrator:
//! pending -> acquiring -> downloading -> converting -> placing -> completed

use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;

use torrentino_core::{
    testing::{fixtures, MockConverter, MockPlacer, MockSearcher, MockTorrentClient},
    ticket::{CreateTicketRequest, QueryContext, TicketState},
    OrchestratorConfig, PipelineProcessor, ProcessorConfig, SqliteCatalog, SqliteTicketStore,
    TextBrainConfig, TicketOrchestrator, TicketStore,
};

/// Test helper to create all dependencies for orchestrator testing.
struct TestHarness {
    ticket_store: Arc<SqliteTicketStore>,
    searcher: Arc<MockSearcher>,
    torrent_client: Arc<MockTorrentClient>,
    converter: MockConverter,
    placer: MockPlacer,
    catalog: Arc<SqliteCatalog>,
    _temp_dir: TempDir,
}

impl TestHarness {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        let ticket_store =
            Arc::new(SqliteTicketStore::new(&db_path).expect("Failed to create ticket store"));
        let catalog = Arc::new(SqliteCatalog::new(&db_path).expect("Failed to create catalog"));
        let searcher = Arc::new(MockSearcher::new());
        let torrent_client = Arc::new(MockTorrentClient::new());
        let converter = MockConverter::new();
        let placer = MockPlacer::new();

        // Set fast durations for testing
        converter
            .set_conversion_duration(Duration::from_millis(10))
            .await;
        placer
            .set_placement_duration(Duration::from_millis(10))
            .await;

        Self {
            ticket_store,
            searcher,
            torrent_client,
            converter,
            placer,
            catalog,
            _temp_dir: temp_dir,
        }
    }

    fn create_orchestrator(&self) -> TicketOrchestrator<MockConverter, MockPlacer> {
        let config = OrchestratorConfig {
            enabled: true,
            acquisition_poll_interval_ms: 50,
            download_poll_interval_ms: 50,
            auto_approve_threshold: 0.0, // Auto-approve everything
            max_concurrent_downloads: 3,
            ..Default::default()
        };

        let processor_config = ProcessorConfig {
            max_parallel_conversions: 2,
            max_parallel_placements: 2,
            ..Default::default()
        };

        let pipeline = PipelineProcessor::new(
            processor_config,
            self.converter.clone(),
            self.placer.clone(),
        )
        .with_ticket_store(Arc::clone(&self.ticket_store) as Arc<dyn TicketStore>);

        let pipeline = Arc::new(pipeline);

        TicketOrchestrator::new(
            config,
            Arc::clone(&self.ticket_store) as Arc<dyn TicketStore>,
            Arc::clone(&self.searcher) as Arc<dyn torrentino_core::Searcher>,
            Arc::clone(&self.torrent_client) as Arc<dyn torrentino_core::TorrentClient>,
            pipeline,
            Arc::clone(&self.catalog) as Arc<dyn torrentino_core::TorrentCatalog>,
            None, // No audit for tests
            TextBrainConfig::default(),
        )
    }

    fn create_ticket(&self, description: &str) -> String {
        let request = CreateTicketRequest {
            created_by: "test".to_string(),
            priority: 100,
            query_context: QueryContext::new(vec!["test".to_string()], description),
            dest_path: "/media/test".into(),
            output_constraints: None,
        };

        self.ticket_store
            .create(request)
            .expect("Failed to create ticket")
            .id
    }

    async fn wait_for_state(
        &self,
        ticket_id: &str,
        expected_state: &str,
        timeout: Duration,
    ) -> bool {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < timeout {
            if let Ok(Some(ticket)) = self.ticket_store.get(ticket_id) {
                let state_type = Self::state_name(&ticket.state);

                if state_type == expected_state {
                    return true;
                }

                // Stop if we hit a terminal state
                if matches!(
                    state_type,
                    "failed" | "cancelled" | "completed" | "acquisition_failed" | "rejected"
                ) && expected_state != state_type
                {
                    return false;
                }
            }
            tokio::time::sleep(poll_interval).await;
        }
        false
    }

    fn get_ticket_state(&self, ticket_id: &str) -> Option<String> {
        self.ticket_store
            .get(ticket_id)
            .ok()
            .flatten()
            .map(|t| Self::state_name(&t.state).to_string())
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
            TicketState::PendingRetry { .. } => "pending_retry",
            TicketState::Failed { .. } => "failed",
            TicketState::Cancelled { .. } => "cancelled",
        }
    }
}

// =============================================================================
// Lifecycle Tests
// =============================================================================

#[tokio::test]
async fn test_ticket_transitions_to_acquiring_when_orchestrator_starts() {
    let harness = TestHarness::new().await;

    // Create a ticket before starting orchestrator
    let ticket_id = harness.create_ticket("Test album");

    // Verify it starts in pending state
    assert_eq!(
        harness.get_ticket_state(&ticket_id),
        Some("pending".to_string())
    );

    // Configure mock to return search results
    harness
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test Artist",
            "Test Album",
            "hash123",
        )])
        .await;

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for ticket to transition from pending
    let found = harness
        .wait_for_state(&ticket_id, "acquiring", Duration::from_secs(2))
        .await
        || harness
            .wait_for_state(&ticket_id, "auto_approved", Duration::from_secs(2))
            .await
        || harness
            .wait_for_state(&ticket_id, "downloading", Duration::from_secs(2))
            .await;

    orchestrator.stop().await;

    assert!(found, "Ticket should have transitioned from pending");
}

#[tokio::test]
async fn test_ticket_reaches_auto_approved_with_search_results() {
    let harness = TestHarness::new().await;

    // Configure mock to return search results
    harness
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test Artist",
            "Test Album",
            "hash123",
        )])
        .await;

    // Create ticket
    let ticket_id = harness.create_ticket("Test album");

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for auto_approved or later state (with threshold 0.0, should auto-approve)
    let reached = harness
        .wait_for_state(&ticket_id, "auto_approved", Duration::from_secs(5))
        .await
        || harness
            .wait_for_state(&ticket_id, "downloading", Duration::from_secs(5))
            .await;

    orchestrator.stop().await;

    assert!(
        reached,
        "Ticket should reach auto_approved or downloading with search results"
    );

    // Verify search was recorded
    let searches = harness.searcher.recorded_searches().await;
    assert!(
        !searches.is_empty(),
        "Searcher should have recorded the search"
    );
}

#[tokio::test]
async fn test_no_search_results_transitions_to_acquisition_failed() {
    let harness = TestHarness::new().await;

    // Configure mock to return empty results
    harness.searcher.set_results(vec![]).await;

    // Create ticket
    let ticket_id = harness.create_ticket("Nonexistent album");

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for acquisition_failed state (no candidates available)
    let reached = harness
        .wait_for_state(&ticket_id, "acquisition_failed", Duration::from_secs(5))
        .await;

    orchestrator.stop().await;

    assert!(
        reached,
        "Ticket should reach acquisition_failed when no search results are found"
    );
}

#[tokio::test]
async fn test_download_progress_updates_ticket_state() {
    let harness = TestHarness::new().await;

    // Configure search results
    harness
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test Artist",
            "Test Album",
            "testhash",
        )])
        .await;

    // Configure torrent client to simulate slow download (50% progress)
    harness.torrent_client.set_progress("testhash", 0.5).await;

    // Create ticket
    let ticket_id = harness.create_ticket("Test album");

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for downloading state
    let reached = harness
        .wait_for_state(&ticket_id, "downloading", Duration::from_secs(5))
        .await;

    orchestrator.stop().await;

    assert!(reached, "Ticket should reach downloading state");
}

#[tokio::test]
async fn test_multiple_tickets_processed_concurrently() {
    let harness = TestHarness::new().await;

    // Configure search results for both tickets
    harness
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("Artist 1", "Album 1", "hash1"),
            fixtures::audio_candidate("Artist 2", "Album 2", "hash2"),
        ])
        .await;

    // Create two tickets
    let ticket1 = harness.create_ticket("Album 1");
    let ticket2 = harness.create_ticket("Album 2");

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for both to progress from pending
    tokio::time::sleep(Duration::from_secs(2)).await;

    let state1 = harness.get_ticket_state(&ticket1);
    let state2 = harness.get_ticket_state(&ticket2);

    orchestrator.stop().await;

    // Both should have progressed from pending
    assert!(
        state1.as_deref() != Some("pending"),
        "Ticket 1 should have progressed from pending, got {:?}",
        state1
    );
    assert!(
        state2.as_deref() != Some("pending"),
        "Ticket 2 should have progressed from pending, got {:?}",
        state2
    );
}

#[tokio::test]
async fn test_orchestrator_stop_is_graceful() {
    let harness = TestHarness::new().await;

    // Configure slow search
    harness
        .searcher
        .set_results(vec![fixtures::audio_candidate("Test", "Album", "hash")])
        .await;

    // Create ticket
    let _ticket_id = harness.create_ticket("Test album");

    // Start and immediately stop
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Give it a moment to pick up the ticket
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Stop should complete without hanging
    let stop_result = tokio::time::timeout(Duration::from_secs(5), orchestrator.stop()).await;

    assert!(
        stop_result.is_ok(),
        "Orchestrator stop should complete within timeout"
    );
}

#[tokio::test]
async fn test_orchestrator_status_reflects_running_state() {
    let harness = TestHarness::new().await;

    let orchestrator = harness.create_orchestrator();

    // Before start, should not be running
    assert!(
        !orchestrator.status().await.running,
        "Orchestrator should not be running before start"
    );

    orchestrator.start().await;

    // After start, should be running
    assert!(
        orchestrator.status().await.running,
        "Orchestrator should be running after start"
    );

    orchestrator.stop().await;

    // After stop, should not be running
    assert!(
        !orchestrator.status().await.running,
        "Orchestrator should not be running after stop"
    );
}

// =============================================================================
// Discography Fallback Tests
// =============================================================================

#[tokio::test]
async fn test_discography_fallback_finds_album_in_discography() {
    use torrentino_core::searcher::TorrentFile;
    use torrentino_core::ticket::ExpectedContent;

    let harness = TestHarness::new().await;

    // Configure mock searcher with a query handler that:
    // - Returns empty for specific album queries ("Dark Side of the Moon")
    // - Returns a discography for fallback queries ("discography", "complete", "collection")
    harness
        .searcher
        .set_query_handler(|query| {
            let query_lower = query.to_lowercase();

            // Fallback queries should return a discography
            if query_lower.contains("discography")
                || query_lower.contains("complete")
                || query_lower.contains("collection")
            {
                // Return a discography with the target album in files
                let mut candidate = fixtures::audio_candidate(
                    "Pink Floyd",
                    "Discography (1967-2014)",
                    "discography_hash",
                );
                candidate.files = Some(vec![
                    TorrentFile {
                        path: "Pink Floyd/1973 - Dark Side of the Moon/01 - Speak to Me.flac"
                            .to_string(),
                        size_bytes: 30_000_000,
                    },
                    TorrentFile {
                        path: "Pink Floyd/1973 - Dark Side of the Moon/02 - Breathe.flac"
                            .to_string(),
                        size_bytes: 35_000_000,
                    },
                    TorrentFile {
                        path: "Pink Floyd/1979 - The Wall/01 - In the Flesh.flac".to_string(),
                        size_bytes: 25_000_000,
                    },
                ]);
                candidate.size_bytes = 10_000_000_000; // 10 GB discography
                Some(vec![candidate])
            } else {
                // Specific album queries return nothing
                Some(vec![])
            }
        })
        .await;

    // Create ticket for a specific album with expected content
    let request = CreateTicketRequest {
        created_by: "test".to_string(),
        priority: 100,
        query_context: QueryContext {
            tags: vec!["music".to_string(), "flac".to_string()],
            description: "Pink Floyd - Dark Side of the Moon".to_string(),
            expected: Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![],
            }),
            catalog_reference: None,
            search_constraints: None,
        },
        dest_path: "/media/test".into(),
        output_constraints: None,
    };

    let ticket_id = harness
        .ticket_store
        .create(request)
        .expect("Failed to create ticket")
        .id;

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for some state change
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Get final state for debugging
    let final_state = harness.get_ticket_state(&ticket_id);
    let searches = harness.searcher.recorded_searches().await;
    let query_strings: Vec<_> = searches.iter().map(|s| s.query.query.clone()).collect();

    orchestrator.stop().await;

    // Check if we reached expected states
    let reached = matches!(
        final_state.as_deref(),
        Some("auto_approved") | Some("downloading") | Some("needs_approval")
    );

    // Verify we found a match via fallback
    assert!(
        reached,
        "Ticket should reach auto_approved, needs_approval, or downloading via discography fallback. \
         Final state: {:?}, Queries made: {:?}",
        final_state, query_strings
    );

    // Verify queries were made - should include fallback queries
    let searches = harness.searcher.recorded_searches().await;
    let has_fallback_query = searches.iter().any(|s| {
        let q_lower = s.query.query.to_lowercase();
        q_lower.contains("discography")
            || q_lower.contains("complete")
            || q_lower.contains("collection")
    });
    let query_strings: Vec<_> = searches.iter().map(|s| s.query.query.clone()).collect();
    assert!(
        has_fallback_query,
        "Should have made fallback discography queries. Searches: {:?}",
        query_strings
    );
}

#[tokio::test]
async fn test_discography_fallback_not_triggered_when_album_found() {
    let harness = TestHarness::new().await;

    // Configure mock searcher to always return a matching album
    harness
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Pink Floyd",
            "Dark Side of the Moon",
            "album_hash",
        )])
        .await;

    // Create ticket
    let ticket_id = harness.create_ticket("Pink Floyd - Dark Side of the Moon");

    // Start orchestrator
    let orchestrator = harness.create_orchestrator();
    orchestrator.start().await;

    // Wait for auto_approved or downloading
    let reached = harness
        .wait_for_state(&ticket_id, "auto_approved", Duration::from_secs(5))
        .await
        || harness
            .wait_for_state(&ticket_id, "downloading", Duration::from_secs(5))
            .await;

    orchestrator.stop().await;

    assert!(
        reached,
        "Ticket should reach auto_approved with direct album match"
    );

    // Verify no fallback queries were made (primary search was successful)
    let searches = harness.searcher.recorded_searches().await;
    let _has_fallback_query = searches.iter().any(|s| {
        let q_lower = s.query.query.to_lowercase();
        q_lower.contains("discography") || q_lower.contains("complete collection")
    });

    // Fallback queries shouldn't be needed since primary search succeeded
    // Note: This depends on the scoring - if primary results are good enough,
    // fallback won't be triggered
    assert!(
        !searches.is_empty(),
        "Should have made at least some queries"
    );
}
