//! End-to-end tests with mocked external dependencies.
//!
//! These tests run the full server stack in-process with mock implementations
//! for external services (Jackett, qBittorrent, MusicBrainz, TMDB).

mod common;

use axum::http::StatusCode;
use serde_json::json;
use torrentino_core::TorrentClient;

use common::{fixtures, TestConfig, TestFixture};

// =============================================================================
// Basic API Tests
// =============================================================================

#[tokio::test]
async fn test_health_endpoint() {
    let fixture = TestFixture::new().await;
    let response = fixture.get("/api/v1/health").await;
    assert_eq!(response.status, StatusCode::OK);
}

#[tokio::test]
async fn test_create_ticket() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "priority": 100,
                "query_context": {
                    "tags": ["music", "flac"],
                    "description": "Abbey Road by The Beatles"
                },
                "dest_path": "/media/music/beatles"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
    assert!(response.body["id"].is_string());
    assert_eq!(response.body["priority"], 100);
    assert_eq!(response.body["state"]["type"], "pending");
    assert_eq!(response.body["query_context"]["tags"][0], "music");
}

#[tokio::test]
async fn test_get_ticket() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": ["movie"],
                    "description": "Test movie"
                },
                "dest_path": "/media/movies/test"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Get the ticket
    let get_response = fixture.get(&format!("/api/v1/tickets/{}", ticket_id)).await;

    assert_eq!(get_response.status, StatusCode::OK);
    assert_eq!(get_response.body["id"], ticket_id);
    assert_eq!(get_response.body["dest_path"], "/media/movies/test");
}

#[tokio::test]
async fn test_list_tickets() {
    let fixture = TestFixture::new().await;

    // Create 3 tickets
    for i in 0..3 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "priority": i * 10,
                    "query_context": {
                        "tags": ["test"],
                        "description": format!("Test ticket {}", i)
                    },
                    "dest_path": format!("/media/test/{}", i)
                }),
            )
            .await;
    }

    // List all tickets
    let response = fixture.get("/api/v1/tickets").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["total"], 3);

    // Verify priority ordering (highest first)
    let tickets = response.body["tickets"].as_array().unwrap();
    assert_eq!(tickets[0]["priority"], 20);
    assert_eq!(tickets[1]["priority"], 10);
    assert_eq!(tickets[2]["priority"], 0);
}

#[tokio::test]
async fn test_cancel_ticket() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "to be cancelled" },
                "dest_path": "/test/cancel"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Cancel the ticket
    let cancel_response = fixture
        .delete_with_body(
            &format!("/api/v1/tickets/{}", ticket_id),
            json!({ "reason": "Testing cancellation" }),
        )
        .await;

    assert_eq!(cancel_response.status, StatusCode::OK);
    assert_eq!(cancel_response.body["state"]["type"], "cancelled");
    assert_eq!(cancel_response.body["state"]["reason"], "Testing cancellation");
}

// =============================================================================
// Search Tests with Mock
// =============================================================================

#[tokio::test]
async fn test_search_with_mock_results() {
    let fixture = TestFixture::new().await;

    // Configure mock searcher with results
    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("The Beatles", "Abbey Road", "hash1"),
            fixtures::audio_candidate("The Beatles", "Let It Be", "hash2"),
        ])
        .await;

    // Perform search
    let response = fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "beatles"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["candidates"].as_array().unwrap().len(), 2);

    // Verify search was recorded
    let searches = fixture.searcher.recorded_searches().await;
    assert_eq!(searches.len(), 1);
    assert!(searches[0].query.query.contains("beatles"));
}

#[tokio::test]
async fn test_search_empty_results() {
    let fixture = TestFixture::new().await;

    // No results configured
    let response = fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "nonexistent artist"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["candidates"].as_array().unwrap().len(), 0);
}

// =============================================================================
// Torrent Client Tests with Mock
// =============================================================================

#[tokio::test]
async fn test_list_torrents() {
    let fixture = TestFixture::new().await;

    // Add a mock torrent
    use torrentino_core::torrent_client::AddTorrentRequest;
    fixture
        .torrent_client
        .add_torrent(AddTorrentRequest::magnet("magnet:?xt=urn:btih:testhash123"))
        .await
        .unwrap();

    // List torrents via API
    let response = fixture.get("/api/v1/torrents").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["torrents"].as_array().unwrap().len(), 1);
}

// =============================================================================
// External Catalog Tests with Mock
// =============================================================================

#[tokio::test]
async fn test_musicbrainz_search() {
    let fixture = TestFixture::new().await;

    // Configure mock external catalog
    fixture
        .external_catalog
        .add_release(fixtures::musicbrainz_release("The Beatles", "Abbey Road", 17))
        .await;

    // Search MusicBrainz
    let response = fixture
        .get("/api/v1/external-catalog/musicbrainz/search?query=beatles&limit=10")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // API returns array directly, not wrapped in {"results": ...}
    let results = response.body.as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["artist_credit"], "The Beatles");
    assert_eq!(results[0]["title"], "Abbey Road");
}

#[tokio::test]
async fn test_tmdb_movie_search() {
    let fixture = TestFixture::new().await;

    // Configure mock external catalog
    fixture
        .external_catalog
        .add_movie(fixtures::tmdb_movie("The Matrix", 1999))
        .await;

    // Search TMDB
    let response = fixture
        .get("/api/v1/external-catalog/tmdb/movies/search?query=matrix")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // API returns array directly, not wrapped in {"results": ...}
    let results = response.body.as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"], "The Matrix");
}

#[tokio::test]
async fn test_tmdb_tv_search() {
    let fixture = TestFixture::new().await;

    // Configure mock external catalog
    fixture
        .external_catalog
        .add_series(fixtures::tmdb_series("Breaking Bad", 5))
        .await;

    // Search TMDB TV
    let response = fixture
        .get("/api/v1/external-catalog/tmdb/tv/search?query=breaking")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // API returns array directly, not wrapped in {"results": ...}
    let results = response.body.as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "Breaking Bad");
    assert_eq!(results[0]["number_of_seasons"], 5);
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_search_error_handling() {
    let fixture = TestFixture::new().await;

    // Configure mock to fail
    fixture
        .searcher
        .set_next_error(torrentino_core::searcher::SearchError::ConnectionFailed(
            "Mock connection error".into(),
        ))
        .await;

    let response = fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "test"
            }),
        )
        .await;

    // Should return internal server error status
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_get_nonexistent_ticket() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/tickets/nonexistent-id").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(response.body["error"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

// =============================================================================
// Catalog Caching Tests
// =============================================================================

#[tokio::test]
async fn test_search_results_cached_in_catalog() {
    let fixture = TestFixture::new().await;

    // Configure mock searcher
    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Pink Floyd",
            "The Wall",
            "wallhash123",
        )])
        .await;

    // Perform search
    fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "pink floyd"
            }),
        )
        .await;

    // Check catalog contains the result
    // Catalog API returns { entries: [...], total: N }
    let catalog_response = fixture.get("/api/v1/catalog?query=floyd").await;

    assert_eq!(catalog_response.status, StatusCode::OK);
    let entries = catalog_response.body["entries"].as_array().unwrap();
    assert!(!entries.is_empty(), "Search results should be cached in catalog");
}

// =============================================================================
// Orchestrator API Tests
// =============================================================================

#[tokio::test]
async fn test_orchestrator_status() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/orchestrator/status").await;

    assert_eq!(response.status, StatusCode::OK);
    // Orchestrator is not available in default test config (no orchestrator injected)
    assert_eq!(response.body["available"], false);
    assert_eq!(response.body["running"], false);
}

#[tokio::test]
async fn test_orchestrator_start_when_unavailable() {
    let fixture = TestFixture::new().await;

    let response = fixture.post("/api/v1/orchestrator/start", json!({})).await;

    // Should return error when orchestrator is not available
    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(response.body["error"].as_str().is_some());
}

// =============================================================================
// Ticket Retry Tests
// =============================================================================

#[tokio::test]
async fn test_retry_cancelled_ticket() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "retry test" },
                "dest_path": "/test/retry"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Cancel the ticket
    fixture
        .delete_with_body(
            &format!("/api/v1/tickets/{}", ticket_id),
            json!({ "reason": "Testing retry" }),
        )
        .await;

    // Retry the ticket
    let retry_response = fixture
        .post(&format!("/api/v1/tickets/{}/retry", ticket_id), json!({}))
        .await;

    assert_eq!(retry_response.status, StatusCode::OK);
    assert_eq!(retry_response.body["state"]["type"], "pending");
}

#[tokio::test]
async fn test_retry_pending_ticket_fails() {
    let fixture = TestFixture::new().await;

    // Create a ticket (it's in pending state)
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "retry test" },
                "dest_path": "/test/retry"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Try to retry a pending ticket (should fail)
    let retry_response = fixture
        .post(&format!("/api/v1/tickets/{}/retry", ticket_id), json!({}))
        .await;

    assert_eq!(retry_response.status, StatusCode::CONFLICT);
    assert!(retry_response.body["error"]
        .as_str()
        .unwrap()
        .contains("pending"));
}

#[tokio::test]
async fn test_retry_nonexistent_ticket() {
    let fixture = TestFixture::new().await;

    let retry_response = fixture
        .post("/api/v1/tickets/nonexistent-id/retry", json!({}))
        .await;

    assert_eq!(retry_response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Ticket Hard Delete Tests
// =============================================================================

#[tokio::test]
async fn test_hard_delete_requires_confirmation() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "delete test" },
                "dest_path": "/test/delete"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Try to hard delete without confirmation
    let delete_response = fixture
        .post(&format!("/api/v1/tickets/{}/delete", ticket_id), json!({}))
        .await;

    assert_eq!(delete_response.status, StatusCode::BAD_REQUEST);
    assert!(delete_response.body["error"]
        .as_str()
        .unwrap()
        .contains("confirmation"));
}

#[tokio::test]
async fn test_hard_delete_with_confirmation() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "delete test" },
                "dest_path": "/test/delete"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Hard delete with confirmation
    let delete_response = fixture
        .post(
            &format!("/api/v1/tickets/{}/delete?confirm=true", ticket_id),
            json!({}),
        )
        .await;

    assert_eq!(delete_response.status, StatusCode::OK);

    // Verify ticket is gone
    let get_response = fixture.get(&format!("/api/v1/tickets/{}", ticket_id)).await;
    assert_eq!(get_response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// TextBrain API Tests
// =============================================================================

#[tokio::test]
async fn test_textbrain_config() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/textbrain/config").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["mode"].is_string());
    assert!(response.body["auto_approve_threshold"].is_number());
}

#[tokio::test]
async fn test_textbrain_build_queries() {
    let fixture = TestFixture::new().await;

    // The API expects a "context" object with tags and description
    let response = fixture
        .post(
            "/api/v1/textbrain/queries",
            json!({
                "context": {
                    "tags": ["music", "rock"],
                    "description": "The Beatles Abbey Road"
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // Response has "result" containing "queries"
    assert!(response.body["result"]["queries"].is_array());
    let queries = response.body["result"]["queries"].as_array().unwrap();
    assert!(!queries.is_empty(), "Should generate at least one query");
}

#[tokio::test]
async fn test_textbrain_score_candidates() {
    let fixture = TestFixture::new().await;

    // Configure mock searcher with results
    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("The Beatles", "Abbey Road", "hash1"),
            fixtures::audio_candidate("The Beatles", "Let It Be", "hash2"),
        ])
        .await;

    // First search to populate catalog
    fixture
        .post("/api/v1/search", json!({ "query": "beatles" }))
        .await;

    // Score candidates - API expects full candidate objects, not just hashes
    let response = fixture
        .post(
            "/api/v1/textbrain/score",
            json!({
                "context": {
                    "tags": ["music"],
                    "description": "Abbey Road by The Beatles"
                },
                "candidates": [
                    {
                        "title": "The Beatles - Abbey Road [FLAC]",
                        "info_hash": "hash1",
                        "size_bytes": 104857600,
                        "seeders": 50,
                        "leechers": 10
                    },
                    {
                        "title": "The Beatles - Let It Be [FLAC]",
                        "info_hash": "hash2",
                        "size_bytes": 104857600,
                        "seeders": 50,
                        "leechers": 10
                    }
                ]
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // Response has "result" containing "candidates"
    assert!(response.body["result"]["candidates"].is_array());
}

// =============================================================================
// Pipeline API Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_status() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/pipeline/status").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["available"].is_boolean());
    // Pipeline is not available in default test config (no pipeline injected)
    assert_eq!(response.body["available"], false);
}

#[tokio::test]
async fn test_pipeline_converter_info() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/pipeline/converter").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["available"], true);
    assert_eq!(response.body["name"], "ffmpeg");
}

#[tokio::test]
async fn test_pipeline_placer_info() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/pipeline/placer").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["available"], true);
    assert_eq!(response.body["name"], "fs");
}

#[tokio::test]
async fn test_pipeline_encoder_capabilities() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/pipeline/encoders").await;

    assert_eq!(response.status, StatusCode::OK);
    // EncoderCapabilitiesResponse has nested "capabilities" with hardware encoder boolean flags
    assert!(response.body["capabilities"].is_object());
    // Check for hardware encoder boolean flags (h264_nvenc, hevc_nvenc, etc.)
    assert!(response.body["capabilities"]["h264_nvenc"].is_boolean());
    assert!(response.body["available_video_formats"].is_array());
    assert!(response.body["has_hardware_encoder"].is_boolean());
}

// =============================================================================
// Catalog Management Tests
// =============================================================================

#[tokio::test]
async fn test_catalog_stats() {
    let fixture = TestFixture::new().await;

    // Add some entries to catalog via search
    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("Artist 1", "Album 1", "cstats_hash1"),
            fixtures::audio_candidate("Artist 2", "Album 2", "cstats_hash2"),
        ])
        .await;

    fixture
        .post("/api/v1/search", json!({ "query": "artist" }))
        .await;

    let response = fixture.get("/api/v1/catalog/stats").await;

    assert_eq!(response.status, StatusCode::OK);
    // CatalogStats has "total_torrents" field for total entries
    assert!(response.body["total_torrents"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn test_catalog_clear() {
    let fixture = TestFixture::new().await;

    // Add entries to catalog via search
    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test",
            "Album",
            "clearhash",
        )])
        .await;

    fixture
        .post("/api/v1/search", json!({ "query": "test" }))
        .await;

    // Clear catalog
    let clear_response = fixture.delete("/api/v1/catalog").await;
    assert_eq!(clear_response.status, StatusCode::OK);

    // Verify catalog is empty
    let stats_response = fixture.get("/api/v1/catalog/stats").await;
    assert_eq!(stats_response.body["total_torrents"], 0);
}

// =============================================================================
// Searcher Status Tests
// =============================================================================

#[tokio::test]
async fn test_searcher_status() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/searcher/status").await;

    assert_eq!(response.status, StatusCode::OK);
    // SearcherStatusResponse has "configured" and "backend" fields
    assert!(response.body["configured"].is_boolean());
    assert!(response.body["backend"].is_string());
}

// =============================================================================
// Torrent Client Extended Tests
// =============================================================================

#[tokio::test]
async fn test_torrent_client_status() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/torrents/status").await;

    assert_eq!(response.status, StatusCode::OK);
    // TorrentClientStatusResponse has "configured" and "backend" fields
    assert!(response.body["configured"].is_boolean());
    assert!(response.body["backend"].is_string());
}

#[tokio::test]
async fn test_add_magnet_torrent() {
    let fixture = TestFixture::new().await;

    // AddMagnetRequest uses "uri" not "magnet_uri"
    let response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({
                "uri": "magnet:?xt=urn:btih:testmagnet123"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // AddTorrentResponse has "hash" not "info_hash"
    assert!(response.body["hash"].is_string());

    // Verify it was added
    let list_response = fixture.get("/api/v1/torrents").await;
    assert!(list_response.body["torrents"].as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn test_get_specific_torrent() {
    let fixture = TestFixture::new().await;

    // Add a torrent first
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({
                "uri": "magnet:?xt=urn:btih:specifichash456"
            }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Get specific torrent by the returned hash
    let response = fixture.get(&format!("/api/v1/torrents/{}", hash)).await;

    assert_eq!(response.status, StatusCode::OK);
    // TorrentInfo uses "hash" for the info hash
    assert!(response.body["hash"].is_string());
}

#[tokio::test]
async fn test_remove_torrent() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({
                "uri": "magnet:?xt=urn:btih:removehash789"
            }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Remove it
    let remove_response = fixture.delete(&format!("/api/v1/torrents/{}", hash)).await;
    assert_eq!(remove_response.status, StatusCode::OK);

    // Verify it's gone
    let get_response = fixture.get(&format!("/api/v1/torrents/{}", hash)).await;
    assert_eq!(get_response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// External Catalog Extended Tests
// =============================================================================

#[tokio::test]
async fn test_external_catalog_status() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/external-catalog/status").await;

    assert_eq!(response.status, StatusCode::OK);
    // ExternalCatalogStatus has boolean availability flags
    assert!(response.body["musicbrainz_available"].is_boolean());
    assert!(response.body["tmdb_available"].is_boolean());
}

#[tokio::test]
async fn test_musicbrainz_get_release() {
    let fixture = TestFixture::new().await;

    // Configure mock with specific release
    let release = fixtures::musicbrainz_release("The Beatles", "Abbey Road", 17);
    fixture.external_catalog.add_release(release).await;

    // Get release by MBID
    let response = fixture
        .get("/api/v1/external-catalog/musicbrainz/release/mb-abbey-road")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["title"], "Abbey Road");
    assert_eq!(response.body["artist_credit"], "The Beatles");
    assert_eq!(response.body["tracks"].as_array().unwrap().len(), 17);
}

#[tokio::test]
async fn test_tmdb_get_movie() {
    let fixture = TestFixture::new().await;

    // Configure mock
    let movie = fixtures::tmdb_movie("Inception", 2010);
    fixture.external_catalog.add_movie(movie.clone()).await;

    // Get movie by ID
    let response = fixture
        .get(&format!("/api/v1/external-catalog/tmdb/movies/{}", movie.id))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["title"], "Inception");
}

#[tokio::test]
async fn test_tmdb_get_series() {
    let fixture = TestFixture::new().await;

    // Configure mock
    let series = fixtures::tmdb_series("Breaking Bad", 5);
    fixture.external_catalog.add_series(series.clone()).await;

    // Get series by ID
    let response = fixture
        .get(&format!("/api/v1/external-catalog/tmdb/tv/{}", series.id))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["name"], "Breaking Bad");
    assert_eq!(response.body["number_of_seasons"], 5);
}

#[tokio::test]
async fn test_tmdb_get_season() {
    let fixture = TestFixture::new().await;

    // Configure mock
    let series = fixtures::tmdb_series("Breaking Bad", 5);
    let season = fixtures::tmdb_season(1, 7);
    fixture.external_catalog.add_series(series.clone()).await;
    fixture
        .external_catalog
        .add_season(series.id, season)
        .await;

    // Get season
    let response = fixture
        .get(&format!(
            "/api/v1/external-catalog/tmdb/tv/{}/season/1",
            series.id
        ))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["season_number"], 1);
    assert_eq!(response.body["episodes"].as_array().unwrap().len(), 7);
}

// =============================================================================
// Audit API Tests
// =============================================================================

#[tokio::test]
async fn test_audit_query_by_event_type() {
    let fixture = TestFixture::new().await;

    // Create multiple tickets to generate audit events
    for i in 0..3 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("audit test {}", i) },
                    "dest_path": format!("/test/audit/{}", i)
                }),
            )
            .await;
    }

    // Give audit writer time to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Query by event type
    let response = fixture
        .get("/api/v1/audit?event_type=ticket_created")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let events = response.body["events"].as_array().unwrap();
    assert!(events.len() >= 3, "Should have at least 3 ticket_created events");
}

#[tokio::test]
async fn test_audit_query_with_limit() {
    let fixture = TestFixture::new().await;

    // Create tickets to generate audit events
    for i in 0..5 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("limit test {}", i) },
                    "dest_path": format!("/test/limit/{}", i)
                }),
            )
            .await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Query with limit
    let response = fixture.get("/api/v1/audit?limit=2").await;

    assert_eq!(response.status, StatusCode::OK);
    let events = response.body["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
}

// =============================================================================
// Ticket Approval/Rejection Tests
// =============================================================================

#[tokio::test]
async fn test_approve_ticket_wrong_state() {
    let fixture = TestFixture::new().await;

    // Create a ticket (will be in pending state)
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "approval test" },
                "dest_path": "/test/approve"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Try to approve a pending ticket (should fail - needs to be in NeedsApproval state)
    let approve_response = fixture
        .post(&format!("/api/v1/tickets/{}/approve", ticket_id), json!({}))
        .await;

    assert_eq!(approve_response.status, StatusCode::CONFLICT);
    assert!(approve_response.body["error"]
        .as_str()
        .unwrap()
        .contains("pending"));
}

#[tokio::test]
async fn test_reject_ticket_wrong_state() {
    let fixture = TestFixture::new().await;

    // Create a ticket (will be in pending state)
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "reject test" },
                "dest_path": "/test/reject"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Try to reject a pending ticket (should fail - needs to be in NeedsApproval state)
    let reject_response = fixture
        .post(&format!("/api/v1/tickets/{}/reject", ticket_id), json!({}))
        .await;

    assert_eq!(reject_response.status, StatusCode::CONFLICT);
    assert!(reject_response.body["error"]
        .as_str()
        .unwrap()
        .contains("pending"));
}

#[tokio::test]
async fn test_approve_nonexistent_ticket() {
    let fixture = TestFixture::new().await;

    let approve_response = fixture
        .post("/api/v1/tickets/nonexistent-id/approve", json!({}))
        .await;

    assert_eq!(approve_response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_reject_nonexistent_ticket() {
    let fixture = TestFixture::new().await;

    let reject_response = fixture
        .post("/api/v1/tickets/nonexistent-id/reject", json!({}))
        .await;

    assert_eq!(reject_response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Config Endpoint Tests
// =============================================================================

#[tokio::test]
async fn test_config_endpoint() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/config").await;

    assert_eq!(response.status, StatusCode::OK);
    // SanitizedConfig response should have server, database, auth sections
    assert!(response.body["server"].is_object());
    assert!(response.body["database"].is_object());
    assert!(response.body["auth"].is_object());
}

// =============================================================================
// Additional Ticket State Tests
// =============================================================================

#[tokio::test]
async fn test_ticket_list_with_state_filter() {
    let fixture = TestFixture::new().await;

    // Create multiple tickets
    for i in 0..3 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("state filter test {}", i) },
                    "dest_path": format!("/test/state/{}", i)
                }),
            )
            .await;
    }

    // List only pending tickets
    let response = fixture.get("/api/v1/tickets?state=pending").await;

    assert_eq!(response.status, StatusCode::OK);
    let tickets = response.body["tickets"].as_array().unwrap();
    // All created tickets should be in pending state
    for ticket in tickets {
        assert_eq!(ticket["state"]["type"], "pending");
    }
}

#[tokio::test]
async fn test_ticket_list_pagination() {
    let fixture = TestFixture::new().await;

    // Create 5 tickets
    for i in 0..5 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("pagination test {}", i) },
                    "dest_path": format!("/test/page/{}", i)
                }),
            )
            .await;
    }

    // Get first page with limit 2
    let page1 = fixture.get("/api/v1/tickets?limit=2&offset=0").await;
    assert_eq!(page1.status, StatusCode::OK);
    assert_eq!(page1.body["tickets"].as_array().unwrap().len(), 2);
    assert!(page1.body["total"].as_i64().unwrap() >= 5);

    // Get second page
    let page2 = fixture.get("/api/v1/tickets?limit=2&offset=2").await;
    assert_eq!(page2.status, StatusCode::OK);
    assert_eq!(page2.body["tickets"].as_array().unwrap().len(), 2);

    // Ensure we got different tickets
    let page1_ids: Vec<&str> = page1.body["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    let page2_ids: Vec<&str> = page2.body["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();

    for id in &page1_ids {
        assert!(!page2_ids.contains(id), "Pages should not overlap");
    }
}

#[tokio::test]
async fn test_ticket_with_priority() {
    let fixture = TestFixture::new().await;

    // Create ticket with priority
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "priority": 100,
                "query_context": { "tags": ["music"], "description": "high priority" },
                "dest_path": "/test/priority"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(response.body["priority"], 100);
}

#[tokio::test]
async fn test_ticket_with_output_constraints() {
    let fixture = TestFixture::new().await;

    // Create ticket with output constraints
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": ["music"],
                    "description": "with constraints"
                },
                "dest_path": "/test/constraints",
                "output_constraints": {
                    "type": "audio",
                    "format": "ogg_vorbis",
                    "bitrate_kbps": 320
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
    assert!(response.body["output_constraints"].is_object());
}

// =============================================================================
// Pipeline with Fixture Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_status_with_pipeline_enabled() {
    let fixture = TestFixture::with_config(TestConfig::with_pipeline()).await;

    let response = fixture.get("/api/v1/pipeline/status").await;

    assert_eq!(response.status, StatusCode::OK);
    // Pipeline should be available when enabled
    assert_eq!(response.body["available"], true);
}

#[tokio::test]
async fn test_pipeline_progress_nonexistent_ticket() {
    let fixture = TestFixture::with_config(TestConfig::with_pipeline()).await;

    let response = fixture.get("/api/v1/pipeline/progress/nonexistent").await;

    // Should return 404 for nonexistent ticket
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pipeline_process_ticket_wrong_state() {
    let fixture = TestFixture::with_config(TestConfig::with_pipeline()).await;

    // Create a ticket (pending state)
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "pipeline test" },
                "dest_path": "/test/pipeline"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Try to process a pending ticket (should fail - needs to be Downloaded)
    let process_response = fixture
        .post(&format!("/api/v1/pipeline/process/{}", ticket_id), json!({}))
        .await;

    // Should fail because ticket is not in Downloaded state
    // Returns 422 Unprocessable Entity for invalid state transition
    assert_eq!(process_response.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_pipeline_validate_ffmpeg() {
    let fixture = TestFixture::with_config(TestConfig::with_pipeline()).await;

    let response = fixture.get("/api/v1/pipeline/validate").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["valid"].is_boolean());
}

// =============================================================================
// Additional Torrent Operations Tests
// =============================================================================

#[tokio::test]
async fn test_pause_torrent() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:pausehash123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Pause the torrent
    let pause_response = fixture
        .post(&format!("/api/v1/torrents/{}/pause", hash), json!({}))
        .await;

    assert_eq!(pause_response.status, StatusCode::OK);
}

#[tokio::test]
async fn test_resume_torrent() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:resumehash123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Resume the torrent
    let resume_response = fixture
        .post(&format!("/api/v1/torrents/{}/resume", hash), json!({}))
        .await;

    assert_eq!(resume_response.status, StatusCode::OK);
}

#[tokio::test]
async fn test_recheck_torrent() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:recheckhash123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Recheck the torrent
    let recheck_response = fixture
        .post(&format!("/api/v1/torrents/{}/recheck", hash), json!({}))
        .await;

    assert_eq!(recheck_response.status, StatusCode::OK);
}

#[tokio::test]
async fn test_set_torrent_upload_limit() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:limithash123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Set upload limit
    let limit_response = fixture
        .post(
            &format!("/api/v1/torrents/{}/upload-limit", hash),
            json!({ "limit": 100000 }),
        )
        .await;

    assert_eq!(limit_response.status, StatusCode::OK);
}

#[tokio::test]
async fn test_set_torrent_download_limit() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:dlimithash123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Set download limit
    let limit_response = fixture
        .post(
            &format!("/api/v1/torrents/{}/download-limit", hash),
            json!({ "limit": 200000 }),
        )
        .await;

    assert_eq!(limit_response.status, StatusCode::OK);
}

// =============================================================================
// Catalog Entry Tests
// =============================================================================

#[tokio::test]
async fn test_get_catalog_entry() {
    let fixture = TestFixture::new().await;

    // Add entry to catalog via search
    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test Artist",
            "Test Album",
            "catalogentry123",
        )])
        .await;

    fixture
        .post("/api/v1/search", json!({ "query": "test" }))
        .await;

    // Get specific entry
    let response = fixture.get("/api/v1/catalog/catalogentry123").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["info_hash"].is_string());
}

#[tokio::test]
async fn test_get_nonexistent_catalog_entry() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/catalog/nonexistent").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_catalog_entry() {
    let fixture = TestFixture::new().await;

    // Add entry to catalog via search
    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Delete Artist",
            "Delete Album",
            "deleteentry456",
        )])
        .await;

    fixture
        .post("/api/v1/search", json!({ "query": "delete" }))
        .await;

    // Delete specific entry
    let delete_response = fixture.delete("/api/v1/catalog/deleteentry456").await;
    assert_eq!(delete_response.status, StatusCode::OK);

    // Verify it's gone
    let get_response = fixture.get("/api/v1/catalog/deleteentry456").await;
    assert_eq!(get_response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Searcher Indexers Test
// =============================================================================

#[tokio::test]
async fn test_list_indexers() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/searcher/indexers").await;

    assert_eq!(response.status, StatusCode::OK);
    // Response has "indexers" array
    assert!(response.body["indexers"].is_array());
}

// =============================================================================
// Orchestrator Stop Test
// =============================================================================

#[tokio::test]
async fn test_orchestrator_stop_when_unavailable() {
    let fixture = TestFixture::new().await;

    let response = fixture.post("/api/v1/orchestrator/stop", json!({})).await;

    // Should return error when orchestrator is not available
    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
}
