//! End-to-end tests with mocked external dependencies.
//!
//! These tests run the full server stack in-process with mock implementations
//! for external services (Jackett, qBittorrent, MusicBrainz, TMDB).

mod common;

use axum::http::StatusCode;
use serde_json::json;
use torrentino_core::TorrentClient;

use common::{fixtures, TestFixture};

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
