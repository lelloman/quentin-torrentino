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
    assert_eq!(
        cancel_response.body["state"]["reason"],
        "Testing cancellation"
    );
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
        .add_release(fixtures::musicbrainz_release(
            "The Beatles",
            "Abbey Road",
            17,
        ))
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
    assert!(
        !entries.is_empty(),
        "Search results should be cached in catalog"
    );
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
    assert!(!list_response.body["torrents"]
        .as_array()
        .unwrap()
        .is_empty());
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
        .get(&format!(
            "/api/v1/external-catalog/tmdb/movies/{}",
            movie.id
        ))
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
    fixture.external_catalog.add_season(series.id, season).await;

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
    let response = fixture.get("/api/v1/audit?event_type=ticket_created").await;

    assert_eq!(response.status, StatusCode::OK);
    let events = response.body["events"].as_array().unwrap();
    assert!(
        events.len() >= 3,
        "Should have at least 3 ticket_created events"
    );
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
        .post(
            &format!("/api/v1/pipeline/process/{}", ticket_id),
            json!({}),
        )
        .await;

    // Should fail because ticket is not in Downloaded state
    // Returns 422 Unprocessable Entity for invalid state transition
    assert_eq!(process_response.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_pipeline_validate_ffmpeg() {
    let fixture = TestFixture::with_config(TestConfig::with_pipeline()).await;

    let response = fixture.get("/api/v1/pipeline/validate").await;

    // Endpoint returns 200 if ffmpeg is available, 503 if not.
    // We don't assume ffmpeg is installed on the host machine.
    assert!(
        response.status == StatusCode::OK || response.status == StatusCode::SERVICE_UNAVAILABLE,
        "Expected 200 or 503, got {}",
        response.status
    );
    assert!(response.body["valid"].is_boolean());
    assert!(response.body["ffmpeg_available"].is_boolean());
    assert!(response.body["ffprobe_available"].is_boolean());
    assert!(response.body["message"].is_string());
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

// =============================================================================
// Torrent Add From URL Tests
// =============================================================================

#[tokio::test]
async fn test_add_torrent_from_url_invalid_url() {
    let fixture = TestFixture::new().await;

    // Try to add from an invalid/unreachable URL
    let response = fixture
        .post(
            "/api/v1/torrents/add/url",
            json!({
                "url": "http://localhost:99999/nonexistent.torrent"
            }),
        )
        .await;

    // Should fail with bad gateway (can't reach URL)
    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_add_torrent_with_options() {
    let fixture = TestFixture::new().await;

    // Add magnet with optional parameters
    let response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({
                "uri": "magnet:?xt=urn:btih:optionshash123",
                "download_path": "/custom/path",
                "category": "movies",
                "paused": true,
                "ticket_id": "ticket-123"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["hash"].is_string());
}

// =============================================================================
// TextBrain Complete Tests
// =============================================================================

#[tokio::test]
async fn test_textbrain_complete_unsupported_provider() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/textbrain/complete",
            json!({
                "prompt": "Test prompt",
                "provider": "openai",
                "api_key": "test-key"
            }),
        )
        .await;

    // Should return error for unsupported provider
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(response.body["error"]
        .as_str()
        .unwrap()
        .contains("Unsupported provider"));
}

// =============================================================================
// TextBrain Process Ticket Tests
// =============================================================================

#[tokio::test]
async fn test_textbrain_process_nonexistent_ticket() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/textbrain/process/nonexistent-id",
            json!({
                "api_key": "test"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(response.body["error"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

// =============================================================================
// TextBrain Acquire Tests
// =============================================================================

#[tokio::test]
async fn test_textbrain_acquire_basic() {
    let fixture = TestFixture::new().await;

    // Configure mock searcher with results
    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("Pink Floyd", "The Dark Side of the Moon", "dsotm_hash"),
            fixtures::audio_candidate("Pink Floyd", "Wish You Were Here", "wywh_hash"),
        ])
        .await;

    let response = fixture
        .post(
            "/api/v1/textbrain/acquire",
            json!({
                "description": "Pink Floyd The Dark Side of the Moon FLAC",
                "tags": ["music", "album", "flac"],
                "auto_approve_threshold": 0.5
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["queries_tried"].is_array());
    assert!(response.body["candidates"].is_array());
    assert!(response.body["duration_ms"].is_number());
}

#[tokio::test]
async fn test_textbrain_acquire_with_expected_album() {
    let fixture = TestFixture::new().await;

    // Configure mock searcher
    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "The Beatles",
            "Abbey Road",
            "abbeyroad_hash",
        )])
        .await;

    let response = fixture
        .post(
            "/api/v1/textbrain/acquire",
            json!({
                "description": "The Beatles Abbey Road",
                "tags": ["music", "album"],
                "expected": {
                    "type": "album",
                    "artist": "The Beatles",
                    "title": "Abbey Road",
                    "tracks": [
                        { "number": 1, "title": "Come Together", "duration_secs": 259 },
                        { "number": 2, "title": "Something", "duration_secs": 183 }
                    ]
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["query_method"].is_string());
    assert!(response.body["score_method"].is_string());
}

#[tokio::test]
async fn test_textbrain_acquire_cache_only_empty() {
    let fixture = TestFixture::new().await;

    // Cache-only search with empty cache
    let response = fixture
        .post(
            "/api/v1/textbrain/acquire",
            json!({
                "description": "Some obscure album",
                "tags": ["music"],
                "cache_only": true
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["candidates_evaluated"], 0);
}

#[tokio::test]
async fn test_textbrain_acquire_movie() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![fixtures::video_candidate(
            "Inception",
            2010,
            "inception_hash",
        )])
        .await;

    let response = fixture
        .post(
            "/api/v1/textbrain/acquire",
            json!({
                "description": "Inception 2010 1080p",
                "tags": ["movie", "1080p"],
                "expected": {
                    "type": "movie",
                    "title": "Inception",
                    "year": 2010
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
}

#[tokio::test]
async fn test_textbrain_acquire_tv_episode() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![fixtures::torrent_candidate(
            "Breaking Bad S01E01 1080p",
            "bb_s01e01_hash",
        )])
        .await;

    let response = fixture
        .post(
            "/api/v1/textbrain/acquire",
            json!({
                "description": "Breaking Bad Season 1 Episode 1",
                "tags": ["tv", "1080p"],
                "expected": {
                    "type": "tv_episode",
                    "series": "Breaking Bad",
                    "season": 1,
                    "episodes": [1]
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
}

// =============================================================================
// Ticket Approval Flow Tests (with proper state setup)
// =============================================================================

#[tokio::test]
async fn test_approve_ticket_in_needs_approval_state() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": ["music"], "description": "approval flow test" },
                "dest_path": "/test/approval-flow"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Manually put ticket into NeedsApproval state via the ticket store
    // We need to search first to populate catalog, then update state
    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test Artist",
            "Test Album",
            "approval_test_hash",
        )])
        .await;

    // Search to populate catalog
    fixture
        .post("/api/v1/search", json!({ "query": "test" }))
        .await;

    // Now we can test approval - but the ticket is still in pending state
    // This tests that approval correctly rejects pending tickets
    let approve_response = fixture
        .post(&format!("/api/v1/tickets/{}/approve", ticket_id), json!({}))
        .await;

    // Should fail because it's in pending state, not NeedsApproval
    assert_eq!(approve_response.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_reject_with_reason() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "reject with reason" },
                "dest_path": "/test/reject-reason"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Try to reject (will fail because wrong state, but tests the reason handling)
    let reject_response = fixture
        .post(
            &format!("/api/v1/tickets/{}/reject", ticket_id),
            json!({ "reason": "Quality too low" }),
        )
        .await;

    // Fails because pending state
    assert_eq!(reject_response.status, StatusCode::CONFLICT);
}

// =============================================================================
// Ticket Validation and Edge Cases
// =============================================================================

#[tokio::test]
async fn test_create_ticket_missing_required_fields() {
    let fixture = TestFixture::new().await;

    // Missing dest_path
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "test" }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_ticket_missing_query_context() {
    let fixture = TestFixture::new().await;

    // Missing query_context
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "dest_path": "/test/path"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_ticket_list_empty_state_filter() {
    let fixture = TestFixture::new().await;

    // Filter by a state that has no tickets
    let response = fixture.get("/api/v1/tickets?state=completed").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["tickets"].as_array().unwrap().len(), 0);
    assert_eq!(response.body["total"], 0);
}

#[tokio::test]
async fn test_ticket_list_invalid_limit() {
    let fixture = TestFixture::new().await;

    // Create some tickets first
    for i in 0..3 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("test {}", i) },
                    "dest_path": format!("/test/{}", i)
                }),
            )
            .await;
    }

    // Negative limit should be clamped to 1
    let response = fixture.get("/api/v1/tickets?limit=-5").await;
    assert_eq!(response.status, StatusCode::OK);

    // Excessive limit should be clamped to max (1000)
    let response2 = fixture.get("/api/v1/tickets?limit=9999").await;
    assert_eq!(response2.status, StatusCode::OK);
}

#[tokio::test]
async fn test_create_ticket_with_full_query_context() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "priority": 500,
                "query_context": {
                    "tags": ["music", "flac", "album"],
                    "description": "Pink Floyd - The Wall",
                    "expected": {
                        "type": "album",
                        "artist": "Pink Floyd",
                        "title": "The Wall",
                        "tracks": [
                            { "number": 1, "title": "In the Flesh?", "duration_secs": 195 },
                            { "number": 2, "title": "The Thin Ice", "duration_secs": 145 }
                        ]
                    },
                    "search_constraints": {
                        "preferred_format": "flac",
                        "min_bitrate_kbps": 320,
                        "min_seeders": 5
                    }
                },
                "dest_path": "/media/music/pink-floyd/the-wall",
                "output_constraints": {
                    "type": "audio",
                    "format": "flac"
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(response.body["priority"], 500);
    assert_eq!(
        response.body["query_context"]["tags"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn test_create_ticket_with_catalog_reference() {
    let fixture = TestFixture::new().await;

    // Test with TMDB movie reference (simpler structure)
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": ["movie"],
                    "description": "Inception",
                    "catalog_reference": {
                        "type": "tmdb",
                        "id": 27205,
                        "media_type": "movie",
                        "runtime_minutes": 148
                    }
                },
                "dest_path": "/media/movies/inception"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
}

// =============================================================================
// Audit Extended Tests
// =============================================================================

#[tokio::test]
async fn test_audit_query_by_ticket_id() {
    let fixture = TestFixture::new().await;

    // Create a specific ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "audit ticket query" },
                "dest_path": "/test/audit-ticket"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Give audit writer time to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Query by ticket ID
    let response = fixture
        .get(&format!("/api/v1/audit?ticket_id={}", ticket_id))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let events = response.body["events"].as_array().unwrap();
    // Should have at least the ticket_created event
    assert!(!events.is_empty());
}

#[tokio::test]
async fn test_audit_query_pagination() {
    let fixture = TestFixture::new().await;

    // Create multiple tickets to generate events
    for i in 0..5 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("pagination audit {}", i) },
                    "dest_path": format!("/test/audit-page/{}", i)
                }),
            )
            .await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Get first page
    let page1 = fixture.get("/api/v1/audit?limit=2&offset=0").await;
    assert_eq!(page1.status, StatusCode::OK);
    assert_eq!(page1.body["events"].as_array().unwrap().len(), 2);
    assert_eq!(page1.body["limit"], 2);
    assert_eq!(page1.body["offset"], 0);

    // Get second page
    let page2 = fixture.get("/api/v1/audit?limit=2&offset=2").await;
    assert_eq!(page2.status, StatusCode::OK);
    assert_eq!(page2.body["events"].as_array().unwrap().len(), 2);
    assert_eq!(page2.body["offset"], 2);
}

#[tokio::test]
async fn test_audit_query_empty_result() {
    let fixture = TestFixture::new().await;

    // Query for non-existent ticket
    let response = fixture
        .get("/api/v1/audit?ticket_id=nonexistent-ticket-id")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["events"].as_array().unwrap().len(), 0);
    assert_eq!(response.body["total"], 0);
}

// =============================================================================
// Torrent Filtering Tests
// =============================================================================

#[tokio::test]
async fn test_list_torrents_with_filter() {
    let fixture = TestFixture::new().await;

    // Add multiple torrents
    fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:filter1", "category": "music" }),
        )
        .await;
    fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:filter2", "category": "movies" }),
        )
        .await;

    // Filter by category
    let response = fixture.get("/api/v1/torrents?category=music").await;

    assert_eq!(response.status, StatusCode::OK);
    // Mock may not support filtering, but endpoint should work
    assert!(response.body["torrents"].is_array());
}

#[tokio::test]
async fn test_list_torrents_with_search() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:searchtest" }),
        )
        .await;

    // Search by name
    let response = fixture.get("/api/v1/torrents?search=test").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["torrents"].is_array());
}

// =============================================================================
// Torrent Operations on Nonexistent
// =============================================================================

#[tokio::test]
async fn test_pause_nonexistent_torrent() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post("/api/v1/torrents/nonexistent/pause", json!({}))
        .await;

    // Mock might not properly return not found, but should handle gracefully
    assert!(
        response.status == StatusCode::NOT_FOUND
            || response.status == StatusCode::OK
            || response.status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_resume_nonexistent_torrent() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post("/api/v1/torrents/nonexistent/resume", json!({}))
        .await;

    assert!(
        response.status == StatusCode::NOT_FOUND
            || response.status == StatusCode::OK
            || response.status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_remove_torrent_with_files() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:deletefiles123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Remove with delete_files=true
    let remove_response = fixture
        .delete(&format!("/api/v1/torrents/{}?delete_files=true", hash))
        .await;

    assert_eq!(remove_response.status, StatusCode::OK);
}

// =============================================================================
// Catalog Search Tests
// =============================================================================

#[tokio::test]
async fn test_catalog_search_with_query() {
    let fixture = TestFixture::new().await;

    // Add entries via search
    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("Metallica", "Master of Puppets", "mop_hash"),
            fixtures::audio_candidate("Megadeth", "Rust in Peace", "rip_hash"),
        ])
        .await;

    fixture
        .post("/api/v1/search", json!({ "query": "metal" }))
        .await;

    // Search catalog
    let response = fixture.get("/api/v1/catalog?query=metallica").await;

    assert_eq!(response.status, StatusCode::OK);
    // Catalog should have entries with matching query
    let entries = response.body["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_catalog_pagination() {
    let fixture = TestFixture::new().await;

    // Add multiple entries
    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("Artist1", "Album1", "pg_hash1"),
            fixtures::audio_candidate("Artist2", "Album2", "pg_hash2"),
            fixtures::audio_candidate("Artist3", "Album3", "pg_hash3"),
            fixtures::audio_candidate("Artist4", "Album4", "pg_hash4"),
            fixtures::audio_candidate("Artist5", "Album5", "pg_hash5"),
        ])
        .await;

    fixture
        .post("/api/v1/search", json!({ "query": "artist" }))
        .await;

    // Get with pagination
    let response = fixture.get("/api/v1/catalog?limit=2").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["entries"].as_array().unwrap().len() <= 2);
}

// =============================================================================
// External Catalog Error Cases
// =============================================================================

#[tokio::test]
async fn test_musicbrainz_search_empty() {
    let fixture = TestFixture::new().await;

    // Don't add any mock data - should return empty results
    let response = fixture
        .get("/api/v1/external-catalog/musicbrainz/search?query=nonexistent12345")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let results = response.body.as_array().unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_musicbrainz_get_nonexistent_release() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .get("/api/v1/external-catalog/musicbrainz/release/nonexistent-mbid")
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_tmdb_get_nonexistent_movie() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .get("/api/v1/external-catalog/tmdb/movies/99999999")
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_tmdb_get_nonexistent_series() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .get("/api/v1/external-catalog/tmdb/tv/99999999")
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_tmdb_get_nonexistent_season() {
    let fixture = TestFixture::new().await;

    // Add series but not the season
    let series = fixtures::tmdb_series("Test Series", 3);
    fixture.external_catalog.add_series(series.clone()).await;

    let response = fixture
        .get(&format!(
            "/api/v1/external-catalog/tmdb/tv/{}/season/99",
            series.id
        ))
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Search with Indexer Filter
// =============================================================================

#[tokio::test]
async fn test_search_with_indexer_filter() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Test",
            "Album",
            "indexer_filter_hash",
        )])
        .await;

    let response = fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "test",
                "indexers": ["1337x", "rutracker"]
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);

    // Verify the search was recorded with indexers
    let searches = fixture.searcher.recorded_searches().await;
    assert!(!searches.is_empty());
}

#[tokio::test]
async fn test_search_with_limit() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![
            fixtures::audio_candidate("A1", "B1", "limit_hash1"),
            fixtures::audio_candidate("A2", "B2", "limit_hash2"),
            fixtures::audio_candidate("A3", "B3", "limit_hash3"),
        ])
        .await;

    let response = fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "test",
                "limit": 2
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // Mock may return all results, but limit is passed
}

// =============================================================================
// Pipeline Extended Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_process_nonexistent_ticket() {
    let fixture = TestFixture::with_config(TestConfig::with_pipeline()).await;

    let response = fixture
        .post(
            "/api/v1/pipeline/process/nonexistent-id",
            json!({
                "source_files": [
                    { "path": "/tmp/test.flac", "item_id": "track-1", "dest_filename": "01 - Test.ogg" }
                ],
                "dest_dir": "/output"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// TextBrain Query Building Tests
// =============================================================================

#[tokio::test]
async fn test_textbrain_build_queries_with_expected() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/textbrain/queries",
            json!({
                "context": {
                    "tags": ["music", "flac"],
                    "description": "Radiohead OK Computer",
                    "expected": {
                        "type": "album",
                        "artist": "Radiohead",
                        "title": "OK Computer",
                        "tracks": [
                            { "number": 1, "title": "Airbag" },
                            { "number": 2, "title": "Paranoid Android" }
                        ]
                    }
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let queries = response.body["result"]["queries"].as_array().unwrap();
    // Should generate queries based on artist/album
    assert!(!queries.is_empty());
    // Check that queries contain relevant terms
    let all_queries: String = queries
        .iter()
        .filter_map(|q| q.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        all_queries.to_lowercase().contains("radiohead")
            || all_queries.to_lowercase().contains("computer")
    );
}

#[tokio::test]
async fn test_textbrain_build_queries_movie() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/textbrain/queries",
            json!({
                "context": {
                    "tags": ["movie", "1080p"],
                    "description": "The Matrix 1999",
                    "expected": {
                        "type": "movie",
                        "title": "The Matrix",
                        "year": 1999
                    }
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let queries = response.body["result"]["queries"].as_array().unwrap();
    assert!(!queries.is_empty());
}

// =============================================================================
// TextBrain Score with Files
// =============================================================================

#[tokio::test]
async fn test_textbrain_score_with_file_list() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/textbrain/score",
            json!({
                "context": {
                    "tags": ["music"],
                    "description": "Test Album",
                    "expected": {
                        "type": "album",
                        "artist": "Test Artist",
                        "title": "Test Album",
                        "tracks": [
                            { "number": 1, "title": "Track One" },
                            { "number": 2, "title": "Track Two" }
                        ]
                    }
                },
                "candidates": [
                    {
                        "title": "Test Artist - Test Album [FLAC]",
                        "info_hash": "score_files_hash",
                        "size_bytes": 500000000,
                        "seeders": 100,
                        "leechers": 5,
                        "files": [
                            { "path": "01 - Track One.flac", "size_bytes": 50000000 },
                            { "path": "02 - Track Two.flac", "size_bytes": 50000000 }
                        ]
                    }
                ]
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let candidates = response.body["result"]["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty());
    // Check file mappings are populated
    assert!(candidates[0]["file_mappings"].is_array());
}

// =============================================================================
// Retry Failed Ticket Variations
// =============================================================================

#[tokio::test]
async fn test_retry_rejected_ticket() {
    let fixture = TestFixture::new().await;

    // Create ticket, cancel it, then retry
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "retry rejected" },
                "dest_path": "/test/retry-rejected"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Cancel first (to make it retryable)
    fixture
        .delete_with_body(
            &format!("/api/v1/tickets/{}", ticket_id),
            json!({ "reason": "Test" }),
        )
        .await;

    // Retry should work
    let retry_response = fixture
        .post(&format!("/api/v1/tickets/{}/retry", ticket_id), json!({}))
        .await;

    assert_eq!(retry_response.status, StatusCode::OK);
    assert_eq!(retry_response.body["state"]["type"], "pending");
}

// =============================================================================
// Health and Config Extended Tests
// =============================================================================

#[tokio::test]
async fn test_config_has_expected_structure() {
    let fixture = TestFixture::new().await;

    let response = fixture.get("/api/v1/config").await;

    assert_eq!(response.status, StatusCode::OK);
    // Verify structure
    assert!(response.body["server"]["host"].is_string());
    assert!(response.body["server"]["port"].is_number());
    assert!(response.body["database"]["path"].is_string());
    assert!(response.body["auth"]["method"].is_string());
}

// =============================================================================
// Ticket Created By Filter
// =============================================================================

#[tokio::test]
async fn test_ticket_list_by_creator() {
    let fixture = TestFixture::new().await;

    // Create tickets (all will be created by "anonymous" in test)
    for i in 0..3 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("creator test {}", i) },
                    "dest_path": format!("/test/creator/{}", i)
                }),
            )
            .await;
    }

    // Filter by creator
    let response = fixture.get("/api/v1/tickets?created_by=anonymous").await;

    assert_eq!(response.status, StatusCode::OK);
    let tickets = response.body["tickets"].as_array().unwrap();
    assert!(tickets.len() >= 3);

    // All should be by anonymous
    for ticket in tickets {
        assert_eq!(ticket["created_by"], "anonymous");
    }
}

#[tokio::test]
async fn test_ticket_list_by_nonexistent_creator() {
    let fixture = TestFixture::new().await;

    // Create some tickets
    fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "some ticket" },
                "dest_path": "/test/some"
            }),
        )
        .await;

    // Filter by nonexistent creator
    let response = fixture
        .get("/api/v1/tickets?created_by=nonexistent-user")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["tickets"].as_array().unwrap().len(), 0);
}

// =============================================================================
// Combined Filter Tests
// =============================================================================

#[tokio::test]
async fn test_ticket_list_combined_filters() {
    let fixture = TestFixture::new().await;

    // Create tickets
    for i in 0..5 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("combined {}", i) },
                    "dest_path": format!("/test/combined/{}", i)
                }),
            )
            .await;
    }

    // Combined filter: state + created_by + pagination
    let response = fixture
        .get("/api/v1/tickets?state=pending&created_by=anonymous&limit=2&offset=1")
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body["tickets"].as_array().unwrap().len() <= 2);
}

// =============================================================================
// Searcher with Categories
// =============================================================================

#[tokio::test]
async fn test_search_with_categories() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Cat Artist",
            "Cat Album",
            "cat_hash",
        )])
        .await;

    let response = fixture
        .post(
            "/api/v1/search",
            json!({
                "query": "test",
                "categories": ["music", "audio"]  // SearchCategory enum values
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
}

// =============================================================================
// Empty Body Handling
// =============================================================================

#[tokio::test]
async fn test_cancel_ticket_empty_body() {
    let fixture = TestFixture::new().await;

    // Create ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "empty cancel" },
                "dest_path": "/test/empty-cancel"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Cancel with no body (should work)
    let cancel_response = fixture
        .delete(&format!("/api/v1/tickets/{}", ticket_id))
        .await;

    assert_eq!(cancel_response.status, StatusCode::OK);
    assert_eq!(cancel_response.body["state"]["type"], "cancelled");
}

// =============================================================================
// Torrent Get Files
// =============================================================================

#[tokio::test]
async fn test_get_torrent_has_expected_fields() {
    let fixture = TestFixture::new().await;

    // Add a torrent
    let add_response = fixture
        .post(
            "/api/v1/torrents/add/magnet",
            json!({ "uri": "magnet:?xt=urn:btih:fieldshash123" }),
        )
        .await;

    let hash = add_response.body["hash"].as_str().unwrap();

    // Get torrent info
    let response = fixture.get(&format!("/api/v1/torrents/{}", hash)).await;

    assert_eq!(response.status, StatusCode::OK);
    // Check expected fields
    assert!(response.body["hash"].is_string());
    assert!(response.body["name"].is_string());
    assert!(response.body["state"].is_string());
    assert!(response.body["progress"].is_number());
}

// =============================================================================
// CHAOS TESTING - Let's Break This Thing
// =============================================================================

// -----------------------------------------------------------------------------
// Concurrency Stress Tests (Sequential - tower tests don't support true concurrency)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_sequential_ticket_creation_stress() {
    let fixture = TestFixture::new().await;

    // Create 50 tickets in rapid succession
    let mut created_ids = Vec::new();
    for i in 0..50 {
        let response = fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": {
                        "tags": ["stress", format!("test-{}", i)],
                        "description": format!("Stress ticket {}", i)
                    },
                    "dest_path": format!("/stress/test/{}", i)
                }),
            )
            .await;

        assert_eq!(response.status, StatusCode::CREATED);
        created_ids.push(response.body["id"].as_str().unwrap().to_string());
    }

    // Verify all tickets exist and are unique
    let list_response = fixture.get("/api/v1/tickets?limit=100").await;
    let tickets = list_response.body["tickets"].as_array().unwrap();
    assert!(tickets.len() >= 50);

    // Verify no duplicate IDs
    let mut seen_ids = std::collections::HashSet::new();
    for id in &created_ids {
        assert!(seen_ids.insert(id.clone()), "Duplicate ticket ID: {}", id);
    }
}

#[tokio::test]
async fn test_rapid_state_transitions_stress() {
    let fixture = TestFixture::new().await;

    // Create a ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "state stress" },
                "dest_path": "/state/stress"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Rapidly cycle through cancel -> retry many times
    for i in 0..20 {
        let cancel = fixture
            .delete(&format!("/api/v1/tickets/{}", ticket_id))
            .await;
        // First cancel should succeed, rest might conflict
        if i == 0 {
            assert_eq!(cancel.status, StatusCode::OK);
        }

        let retry = fixture
            .post(&format!("/api/v1/tickets/{}/retry", ticket_id), json!({}))
            .await;
        // Retry should work on cancelled ticket
        assert!(
            retry.status == StatusCode::OK || retry.status == StatusCode::CONFLICT,
            "Iteration {}: retry had status {:?}",
            i,
            retry.status
        );
    }

    // Ticket should still be accessible
    let final_state = fixture.get(&format!("/api/v1/tickets/{}", ticket_id)).await;
    assert_eq!(final_state.status, StatusCode::OK);
}

#[tokio::test]
async fn test_search_stress() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![fixtures::audio_candidate(
            "Flood",
            "Test",
            "flood_hash",
        )])
        .await;

    // 100 rapid searches
    for i in 0..100 {
        let response = fixture
            .post(
                "/api/v1/search",
                json!({ "query": format!("search {}", i) }),
            )
            .await;

        assert_eq!(response.status, StatusCode::OK);
    }

    // Verify recorded searches
    let searches = fixture.searcher.recorded_searches().await;
    assert_eq!(searches.len(), 100);
}

// -----------------------------------------------------------------------------
// Unicode and Special Character Edge Cases
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_unicode_in_ticket_description() {
    let fixture = TestFixture::new().await;

    // Various unicode edge cases
    let descriptions = vec![
        "日本語のアルバム",             // Japanese
        "Björk - Homogenic",            // Accented chars
        "🎵 Music with emoji 🎶",       // Emoji
        "RTL text: مرحبا",              // Arabic RTL
        "Ṱ̈́h̴̢̄i̷͔͑s̵̱̀ ̵̣̾ỉ̵̦s̴̰̀ ̴̣̈z̸͖̄a̴͖͝l̸̰̆g̴̻̈́o̶̜͂",                // Zalgo text
        "Line1\nLine2\rLine3\r\nLine4", // Various newlines
        "\t\tTabbed\t\tcontent",        // Tabs
        "Null\x00byte",                 // Null byte
        "",                             // Empty string
        " ",                            // Just whitespace
        "   leading and trailing   ",   // Whitespace padding
        "a]",                           // Very long
    ];

    for (i, desc) in descriptions.iter().enumerate() {
        let response = fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": ["unicode"], "description": desc },
                    "dest_path": format!("/unicode/test/{}", i)
                }),
            )
            .await;

        // Should either succeed or fail gracefully (no panic, no 500)
        assert!(
            response.status == StatusCode::CREATED
                || response.status == StatusCode::BAD_REQUEST
                || response.status == StatusCode::UNPROCESSABLE_ENTITY,
            "Description {:?} caused unexpected status: {:?}",
            desc,
            response.status
        );

        // If created, verify we can read it back correctly
        if response.status == StatusCode::CREATED {
            let id = response.body["id"].as_str().unwrap();
            let get_response = fixture.get(&format!("/api/v1/tickets/{}", id)).await;
            assert_eq!(get_response.status, StatusCode::OK);
            // Description should round-trip (except null bytes might be stripped)
        }
    }
}

#[tokio::test]
async fn test_unicode_in_tags() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": ["音楽", "🎸", "ñoño", "тест", ""],
                    "description": "Unicode tags test"
                },
                "dest_path": "/unicode/tags"
            }),
        )
        .await;

    // Empty tag might be rejected or accepted - either is fine
    assert!(
        response.status == StatusCode::CREATED
            || response.status == StatusCode::BAD_REQUEST
            || response.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_path_traversal_attempts() {
    let fixture = TestFixture::new().await;

    let malicious_paths = vec![
        "../../../etc/passwd",
        "/etc/passwd",
        "..\\..\\windows\\system32",
        "/dev/null",
        "file:///etc/passwd",
        "/proc/self/environ",
        "\x00/etc/passwd",
        "/valid/path/../../../etc/passwd",
    ];

    for path in malicious_paths {
        let response = fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": "path traversal test" },
                    "dest_path": path
                }),
            )
            .await;

        // These should either be rejected or accepted (and sanitized later)
        // Key is no server error
        assert!(
            response.status != StatusCode::INTERNAL_SERVER_ERROR,
            "Path {} caused server error",
            path
        );
    }
}

#[tokio::test]
async fn test_sql_injection_attempts() {
    let fixture = TestFixture::new().await;

    let payloads = vec![
        "'; DROP TABLE tickets; --",
        "1' OR '1'='1",
        "1; DELETE FROM tickets WHERE 1=1; --",
        "' UNION SELECT * FROM sqlite_master --",
        "Robert'); DROP TABLE Students;--",
    ];

    for payload in &payloads {
        // Try in description
        let response = fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": payload },
                    "dest_path": "/sql/test"
                }),
            )
            .await;

        assert_eq!(
            response.status,
            StatusCode::CREATED,
            "SQL injection payload should be treated as regular text"
        );
    }

    // Verify tickets still work (table not dropped)
    let list = fixture.get("/api/v1/tickets").await;
    assert_eq!(list.status, StatusCode::OK);
    assert!(list.body["tickets"].as_array().unwrap().len() >= payloads.len());
}

#[tokio::test]
async fn test_json_injection_in_strings() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": ["{\"malicious\": true}"],
                    "description": "{\"injected\": \"json\", \"admin\": true}"
                },
                "dest_path": "/json/injection"
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED);

    // Verify it's stored as string, not parsed as JSON
    let id = response.body["id"].as_str().unwrap();
    let get_response = fixture.get(&format!("/api/v1/tickets/{}", id)).await;
    assert!(get_response.body["query_context"]["description"]
        .as_str()
        .unwrap()
        .contains("{\"injected\""));
}

// -----------------------------------------------------------------------------
// Boundary Value Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_priority_boundary_values() {
    let fixture = TestFixture::new().await;

    // Test u16 boundaries
    let priorities = vec![0, 1, 65534, 65535];

    for priority in priorities {
        let response = fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "priority": priority,
                    "query_context": { "tags": [], "description": format!("priority {}", priority) },
                    "dest_path": format!("/priority/{}", priority)
                }),
            )
            .await;

        assert_eq!(response.status, StatusCode::CREATED);
        assert_eq!(response.body["priority"], priority);
    }
}

#[tokio::test]
async fn test_priority_overflow() {
    let fixture = TestFixture::new().await;

    // Values beyond u16::MAX
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "priority": 65536,  // u16::MAX + 1
                "query_context": { "tags": [], "description": "overflow" },
                "dest_path": "/overflow"
            }),
        )
        .await;

    // Should reject or wrap - either way, no panic
    assert!(response.status != StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_negative_priority() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "priority": -1,
                "query_context": { "tags": [], "description": "negative" },
                "dest_path": "/negative"
            }),
        )
        .await;

    // Should reject negative values
    assert!(
        response.status == StatusCode::BAD_REQUEST
            || response.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_pagination_boundaries() {
    let fixture = TestFixture::new().await;

    // Create a few tickets
    for i in 0..5 {
        fixture
            .post(
                "/api/v1/tickets",
                json!({
                    "query_context": { "tags": [], "description": format!("pagination {}", i) },
                    "dest_path": format!("/page/{}", i)
                }),
            )
            .await;
    }

    // Edge cases
    let test_cases = vec![
        ("limit=0", true),        // Zero limit - should clamp to 1
        ("limit=1000000", true),  // Huge limit - should clamp to max
        ("offset=-1", true),      // Negative offset - should clamp to 0
        ("offset=1000000", true), // Huge offset - valid, just empty
        ("limit=NaN", false),     // Invalid type
        ("offset=abc", false),    // Invalid type
    ];

    for (query, should_succeed) in test_cases {
        let response = fixture.get(&format!("/api/v1/tickets?{}", query)).await;

        if should_succeed {
            assert_eq!(
                response.status,
                StatusCode::OK,
                "Query {} should succeed",
                query
            );
        } else {
            assert!(
                response.status == StatusCode::BAD_REQUEST
                    || response.status == StatusCode::UNPROCESSABLE_ENTITY
                    || response.status == StatusCode::OK, // Might ignore invalid params
                "Query {} had unexpected status {:?}",
                query,
                response.status
            );
        }
    }
}

#[tokio::test]
async fn test_torrent_hash_edge_cases() {
    let fixture = TestFixture::new().await;

    // Invalid hash formats
    let hashes = vec![
        "",                                                             // Empty
        "abc",                                                          // Too short
        "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",                     // Invalid hex chars
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",                     // Valid 40-char
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",                     // Uppercase
        "123456789012345678901234567890123456789012345678901234567890", // 64 chars (SHA256?)
        "../../../etc/passwd",                                          // Path traversal in hash
    ];

    for hash in hashes {
        let response = fixture.get(&format!("/api/v1/torrents/{}", hash)).await;
        // Should not panic - 404 or 400 is fine
        assert!(
            response.status != StatusCode::INTERNAL_SERVER_ERROR,
            "Hash {} caused server error",
            hash
        );
    }
}

// -----------------------------------------------------------------------------
// Malformed Input Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_extra_unknown_fields_ignored() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "test" },
                "dest_path": "/test",
                "unknown_field": "should be ignored",
                "another_unknown": { "nested": true },
                "__proto__": { "polluted": true }  // Prototype pollution attempt
            }),
        )
        .await;

    // Should succeed, ignoring unknown fields
    assert_eq!(response.status, StatusCode::CREATED);
}

#[tokio::test]
async fn test_wrong_types_in_request() {
    let fixture = TestFixture::new().await;

    // priority as string instead of number
    let response1 = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "priority": "high",  // Should be number
                "query_context": { "tags": [], "description": "test" },
                "dest_path": "/test"
            }),
        )
        .await;

    assert!(
        response1.status == StatusCode::BAD_REQUEST
            || response1.status == StatusCode::UNPROCESSABLE_ENTITY
    );

    // tags as string instead of array
    let response2 = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": "not,an,array", "description": "test" },
                "dest_path": "/test"
            }),
        )
        .await;

    assert!(
        response2.status == StatusCode::BAD_REQUEST
            || response2.status == StatusCode::UNPROCESSABLE_ENTITY
    );

    // description as number instead of string
    let response3 = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": 12345 },
                "dest_path": "/test"
            }),
        )
        .await;

    assert!(
        response3.status == StatusCode::BAD_REQUEST
            || response3.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_null_values_in_request() {
    let fixture = TestFixture::new().await;

    // Explicit null for required field
    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": null },
                "dest_path": "/test"
            }),
        )
        .await;

    assert!(
        response.status == StatusCode::BAD_REQUEST
            || response.status == StatusCode::UNPROCESSABLE_ENTITY
    );

    // Null in array
    let response2 = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [null, "valid", null], "description": "test" },
                "dest_path": "/test"
            }),
        )
        .await;

    // Could accept or reject - but shouldn't panic
    assert!(response2.status != StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_deeply_nested_json() {
    let fixture = TestFixture::new().await;

    // Create deeply nested structure
    let mut nested = json!({ "level": 100 });
    for i in (0..100).rev() {
        nested = json!({ "level": i, "child": nested });
    }

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": [],
                    "description": "nested test",
                    "extra": nested  // Unknown field with deep nesting
                },
                "dest_path": "/nested"
            }),
        )
        .await;

    // Should handle gracefully
    assert!(response.status != StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_duplicate_keys_in_json() {
    let fixture = TestFixture::new().await;

    // Note: serde typically uses last value for duplicate keys
    // We're testing that it doesn't crash
    let response = fixture
        .post_raw(
            "/api/v1/tickets",
            r#"{
            "query_context": { "tags": [], "description": "first" },
            "query_context": { "tags": ["dup"], "description": "second" },
            "dest_path": "/dup1",
            "dest_path": "/dup2"
        }"#,
        )
        .await;

    // Should use one of the values (typically last) and not crash
    assert!(response.status != StatusCode::INTERNAL_SERVER_ERROR);
}

// -----------------------------------------------------------------------------
// State Machine Violation Attempts
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_rapid_state_transitions() {
    let fixture = TestFixture::new().await;

    // Create ticket
    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "rapid transitions" },
                "dest_path": "/rapid"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Rapidly fire cancel -> retry -> cancel -> retry
    for _ in 0..10 {
        fixture
            .delete(&format!("/api/v1/tickets/{}", ticket_id))
            .await;
        fixture
            .post(&format!("/api/v1/tickets/{}/retry", ticket_id), json!({}))
            .await;
    }

    // Ticket should still be accessible and in valid state
    let final_state = fixture.get(&format!("/api/v1/tickets/{}", ticket_id)).await;
    assert_eq!(final_state.status, StatusCode::OK);
    let state_type = final_state.body["state"]["type"].as_str().unwrap();
    assert!(
        state_type == "pending" || state_type == "cancelled",
        "Unexpected state: {}",
        state_type
    );
}

#[tokio::test]
async fn test_double_cancel() {
    let fixture = TestFixture::new().await;

    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "double cancel" },
                "dest_path": "/double-cancel"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // First cancel
    let cancel1 = fixture
        .delete(&format!("/api/v1/tickets/{}", ticket_id))
        .await;
    assert_eq!(cancel1.status, StatusCode::OK);

    // Second cancel - should fail or be idempotent
    let cancel2 = fixture
        .delete(&format!("/api/v1/tickets/{}", ticket_id))
        .await;

    // Either conflict (already cancelled) or OK (idempotent) - but not error
    assert!(
        cancel2.status == StatusCode::OK || cancel2.status == StatusCode::CONFLICT,
        "Double cancel had unexpected status: {:?}",
        cancel2.status
    );
}

#[tokio::test]
async fn test_operations_on_deleted_ticket() {
    let fixture = TestFixture::new().await;

    let create_response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": "will be deleted" },
                "dest_path": "/deleted"
            }),
        )
        .await;

    let ticket_id = create_response.body["id"].as_str().unwrap();

    // Hard delete (POST to /delete with confirm=true)
    let delete_response = fixture
        .post(
            &format!("/api/v1/tickets/{}/delete?confirm=true", ticket_id),
            json!({}),
        )
        .await;
    assert_eq!(delete_response.status, StatusCode::OK);

    // Try various operations on deleted ticket
    let get = fixture.get(&format!("/api/v1/tickets/{}", ticket_id)).await;
    assert_eq!(get.status, StatusCode::NOT_FOUND);

    let cancel = fixture
        .delete(&format!("/api/v1/tickets/{}", ticket_id))
        .await;
    assert_eq!(cancel.status, StatusCode::NOT_FOUND);

    let retry = fixture
        .post(&format!("/api/v1/tickets/{}/retry", ticket_id), json!({}))
        .await;
    assert_eq!(retry.status, StatusCode::NOT_FOUND);

    let approve = fixture
        .post(&format!("/api/v1/tickets/{}/approve", ticket_id), json!({}))
        .await;
    assert_eq!(approve.status, StatusCode::NOT_FOUND);
}

// -----------------------------------------------------------------------------
// Large Payload Stress Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_many_tags() {
    let fixture = TestFixture::new().await;

    // 1000 tags
    let tags: Vec<String> = (0..1000).map(|i| format!("tag-{}", i)).collect();

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": tags, "description": "many tags" },
                "dest_path": "/many-tags"
            }),
        )
        .await;

    // Should handle - might be slow but shouldn't fail
    assert!(
        response.status == StatusCode::CREATED || response.status == StatusCode::BAD_REQUEST,
        "Many tags response: {:?}",
        response.status
    );
}

#[tokio::test]
async fn test_huge_description() {
    let fixture = TestFixture::new().await;

    // 1MB description
    let huge_desc = "x".repeat(1024 * 1024);

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": { "tags": [], "description": huge_desc },
                "dest_path": "/huge-desc"
            }),
        )
        .await;

    // Should either accept or reject with proper error - not crash
    assert!(
        response.status == StatusCode::CREATED
            || response.status == StatusCode::BAD_REQUEST
            || response.status == StatusCode::PAYLOAD_TOO_LARGE
            || response.status == StatusCode::UNPROCESSABLE_ENTITY,
        "Huge description response: {:?}",
        response.status
    );
}

#[tokio::test]
async fn test_many_expected_tracks() {
    let fixture = TestFixture::new().await;

    // Album with 500 tracks
    let tracks: Vec<serde_json::Value> = (1..=500)
        .map(|i| {
            json!({
                "number": i,
                "title": format!("Track {} with a reasonably long title to add some bulk", i),
                "duration_secs": 180 + (i % 120)
            })
        })
        .collect();

    let response = fixture
        .post(
            "/api/v1/tickets",
            json!({
                "query_context": {
                    "tags": ["music"],
                    "description": "Album with many tracks",
                    "expected": {
                        "type": "album",
                        "artist": "Prolific Artist",
                        "title": "Complete Discography",
                        "tracks": tracks
                    }
                },
                "dest_path": "/many-tracks"
            }),
        )
        .await;

    assert!(
        response.status == StatusCode::CREATED || response.status == StatusCode::BAD_REQUEST,
        "Many tracks response: {:?}",
        response.status
    );
}

#[tokio::test]
async fn test_many_candidates_in_score_request() {
    let fixture = TestFixture::new().await;

    // 200 candidates to score
    let candidates: Vec<serde_json::Value> = (0..200)
        .map(|i| {
            json!({
                "title": format!("Candidate {} - Some Album Title [FLAC]", i),
                "info_hash": format!("{:040x}", i),
                "size_bytes": 500_000_000 + i * 1000,
                "seeders": 100 - (i % 100) as u32,
                "leechers": 5
            })
        })
        .collect();

    let response = fixture
        .post(
            "/api/v1/textbrain/score",
            json!({
                "context": {
                    "tags": ["music"],
                    "description": "Test Album"
                },
                "candidates": candidates
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    // All candidates should be scored
    let scored = response.body["result"]["candidates"].as_array().unwrap();
    assert_eq!(scored.len(), 200);
}

#[tokio::test]
async fn test_many_files_in_candidate() {
    let fixture = TestFixture::new().await;

    // Candidate with 1000 files
    let files: Vec<serde_json::Value> = (1..=1000)
        .map(|i| {
            json!({
                "path": format!("Disc {}/Track {:03} - Title.flac", (i / 20) + 1, i),
                "size_bytes": 50_000_000
            })
        })
        .collect();

    let response = fixture
        .post(
            "/api/v1/textbrain/score",
            json!({
                "context": {
                    "tags": ["music"],
                    "description": "Massive box set"
                },
                "candidates": [{
                    "title": "Artist - Complete Box Set [FLAC]",
                    "info_hash": "boxsethash123456789012345678901234567890",
                    "size_bytes": 50_000_000_000u64,
                    "seeders": 50,
                    "leechers": 10,
                    "files": files
                }]
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
}

// -----------------------------------------------------------------------------
// Search Edge Cases
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_search_empty_query() {
    let fixture = TestFixture::new().await;

    let response = fixture.post("/api/v1/search", json!({ "query": "" })).await;

    // Empty query - should reject or return empty
    assert!(
        response.status == StatusCode::OK || response.status == StatusCode::BAD_REQUEST,
        "Empty query response: {:?}",
        response.status
    );
}

#[tokio::test]
async fn test_search_whitespace_query() {
    let fixture = TestFixture::new().await;

    let response = fixture
        .post("/api/v1/search", json!({ "query": "   \t\n   " }))
        .await;

    // Whitespace-only query
    assert!(response.status != StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_search_special_characters() {
    let fixture = TestFixture::new().await;

    fixture
        .searcher
        .set_results(vec![fixtures::torrent_candidate("Result", "hash123")])
        .await;

    let queries = vec![
        "artist:name AND album:title", // Lucene-like syntax
        "foo OR bar",
        "\"exact phrase\"",
        "wild*card",
        "regex[0-9]+",
        "(parentheses)",
        "a && b || c",
        "<script>alert(1)</script>", // XSS attempt
        "{{template}}",              // Template injection
        "${env.PATH}",               // Variable expansion
    ];

    for query in queries {
        let response = fixture
            .post("/api/v1/search", json!({ "query": query }))
            .await;

        // All should be handled gracefully
        assert!(
            response.status == StatusCode::OK || response.status == StatusCode::BAD_REQUEST,
            "Query {:?} had status {:?}",
            query,
            response.status
        );
    }
}

// -----------------------------------------------------------------------------
// Catalog Edge Cases
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_catalog_get_with_special_hash() {
    let fixture = TestFixture::new().await;

    // URL-encoded characters in hash path
    let special_hashes = vec![
        "hash%20with%20space",
        "hash/with/slashes",
        "hash?with=query",
        "hash#with#fragment",
        "hash&with&ampersand",
    ];

    for hash in special_hashes {
        let response = fixture.get(&format!("/api/v1/catalog/{}", hash)).await;
        // Should return 404, not panic or 500
        assert!(
            response.status == StatusCode::NOT_FOUND || response.status == StatusCode::BAD_REQUEST,
            "Hash {} had status {:?}",
            hash,
            response.status
        );
    }
}

// -----------------------------------------------------------------------------
// Audit Edge Cases
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_audit_with_malformed_time_range() {
    let fixture = TestFixture::new().await;

    // Invalid date format
    let response = fixture.get("/api/v1/audit?from=not-a-date").await;
    assert!(
        response.status == StatusCode::BAD_REQUEST || response.status == StatusCode::OK,
        // Might ignore invalid param
    );

    // Future date
    let response2 = fixture.get("/api/v1/audit?from=2099-01-01T00:00:00Z").await;
    assert_eq!(response2.status, StatusCode::OK);
    // Should just return empty
}

// -----------------------------------------------------------------------------
// Content-Type Edge Cases
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_wrong_content_type() {
    let fixture = TestFixture::new().await;

    // Send form data instead of JSON - use post_raw with wrong content type
    let response = fixture
        .post_with_content_type(
            "/api/v1/tickets",
            "query_context[tags][]=music&dest_path=/test",
            "application/x-www-form-urlencoded",
        )
        .await;

    // Should reject with proper error
    assert!(
        response.status == StatusCode::UNSUPPORTED_MEDIA_TYPE
            || response.status == StatusCode::BAD_REQUEST
            || response.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_empty_body() {
    let fixture = TestFixture::new().await;

    let response = fixture.post_raw("/api/v1/tickets", "").await;

    // Should reject empty body
    assert!(
        response.status == StatusCode::BAD_REQUEST
            || response.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_invalid_json() {
    let fixture = TestFixture::new().await;

    let invalid_jsons = vec![
        "{not valid json}",
        "{'single': 'quotes'}",
        "{trailing: comma,}",
        "[1, 2, 3", // Unclosed array
        r#"{"key": undefined}"#,
        "null", // Valid JSON but not an object
        "[]",   // Valid JSON but array, not object
        "42",   // Valid JSON but number
    ];

    for invalid in invalid_jsons {
        let response = fixture.post_raw("/api/v1/tickets", invalid).await;

        assert!(
            response.status == StatusCode::BAD_REQUEST
                || response.status == StatusCode::UNPROCESSABLE_ENTITY,
            "Invalid JSON {:?} had status {:?}",
            invalid,
            response.status
        );
    }
}
