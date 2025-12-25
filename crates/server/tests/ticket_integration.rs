use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tempfile::{NamedTempFile, TempDir};
use tokio::time::sleep;

/// Find an available port
fn get_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Create a config with database path
fn config_with_db(port: u16, db_path: &str) -> String {
    format!(
        r#"
[auth]
method = "none"

[server]
host = "127.0.0.1"
port = {}

[database]
path = "{}"
"#,
        port, db_path
    )
}

/// Spawn the server and return a handle
async fn spawn_server(config_path: &std::path::Path) -> tokio::process::Child {
    tokio::process::Command::new(env!("CARGO_BIN_EXE_quentin"))
        .env("QUENTIN_CONFIG", config_path)
        .env("RUST_LOG", "error") // Quiet logs during tests
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to spawn server")
}

/// Wait for server to be ready
async fn wait_for_server(port: u16, max_attempts: u32) -> bool {
    let client = Client::new();
    for _ in 0..max_attempts {
        if client
            .get(format!("http://127.0.0.1:{}/api/v1/health", port))
            .send()
            .await
            .is_ok()
        {
            return true;
        }
        sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Helper to start a server for testing
async fn start_test_server() -> (u16, tokio::process::Child, TempDir) {
    let port = get_available_port();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config_content = config_with_db(port, db_path.to_str().unwrap());

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let server = spawn_server(temp_file.path()).await;

    assert!(
        wait_for_server(port, 40).await,
        "Server did not start in time"
    );

    // Give a moment for initialization
    sleep(Duration::from_millis(100)).await;

    (port, server, temp_dir)
}

#[tokio::test]
async fn test_create_ticket() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "priority": 100,
            "query_context": {
                "tags": ["music", "flac"],
                "description": "Abbey Road by The Beatles"
            },
            "dest_path": "/media/music/beatles"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 201);

    let json: Value = response.json().await.expect("Failed to parse JSON");

    assert!(json["id"].is_string());
    assert_eq!(json["priority"], 100);
    assert_eq!(json["state"]["type"], "pending");
    assert_eq!(json["query_context"]["tags"][0], "music");
    assert_eq!(json["query_context"]["tags"][1], "flac");
    assert_eq!(json["query_context"]["description"], "Abbey Road by The Beatles");
    assert_eq!(json["dest_path"], "/media/music/beatles");
    assert_eq!(json["created_by"], "anonymous");

    server.kill().await.ok();
}

#[tokio::test]
async fn test_get_ticket() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create a ticket first
    let create_response = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "query_context": {
                "tags": ["movie"],
                "description": "Test movie"
            },
            "dest_path": "/media/movies/test"
        }))
        .send()
        .await
        .expect("Failed to create ticket");

    let created: Value = create_response.json().await.unwrap();
    let ticket_id = created["id"].as_str().unwrap();

    // Get the ticket
    let get_response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets/{}", port, ticket_id))
        .send()
        .await
        .expect("Failed to get ticket");

    assert_eq!(get_response.status(), 200);

    let json: Value = get_response.json().await.unwrap();
    assert_eq!(json["id"], ticket_id);
    assert_eq!(json["dest_path"], "/media/movies/test");

    server.kill().await.ok();
}

#[tokio::test]
async fn test_get_nonexistent_ticket() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets/nonexistent-id", port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);

    let json: Value = response.json().await.unwrap();
    assert!(json["error"].as_str().unwrap().contains("not found"));

    server.kill().await.ok();
}

#[tokio::test]
async fn test_list_tickets() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create 3 tickets
    for i in 0..3 {
        client
            .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
            .json(&json!({
                "priority": i * 10,
                "query_context": {
                    "tags": ["test"],
                    "description": format!("Test ticket {}", i)
                },
                "dest_path": format!("/media/test/{}", i)
            }))
            .send()
            .await
            .expect("Failed to create ticket");
    }

    // List all tickets
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .send()
        .await
        .expect("Failed to list tickets");

    assert_eq!(response.status(), 200);

    let json: Value = response.json().await.unwrap();
    assert_eq!(json["total"], 3);
    assert_eq!(json["tickets"].as_array().unwrap().len(), 3);

    // Verify priority ordering (highest first)
    let tickets = json["tickets"].as_array().unwrap();
    assert_eq!(tickets[0]["priority"], 20);
    assert_eq!(tickets[1]["priority"], 10);
    assert_eq!(tickets[2]["priority"], 0);

    server.kill().await.ok();
}

#[tokio::test]
async fn test_list_tickets_with_state_filter() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create 2 tickets
    let response1 = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "query_context": { "tags": [], "description": "ticket 1" },
            "dest_path": "/test/1"
        }))
        .send()
        .await
        .unwrap();
    let ticket1: Value = response1.json().await.unwrap();
    let ticket1_id = ticket1["id"].as_str().unwrap();

    client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "query_context": { "tags": [], "description": "ticket 2" },
            "dest_path": "/test/2"
        }))
        .send()
        .await
        .unwrap();

    // Cancel ticket 1
    client
        .delete(format!("http://127.0.0.1:{}/api/v1/tickets/{}", port, ticket1_id))
        .send()
        .await
        .unwrap();

    // List only pending tickets
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets?state=pending", port))
        .send()
        .await
        .unwrap();

    let json: Value = response.json().await.unwrap();
    assert_eq!(json["total"], 1);

    // List only cancelled tickets
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets?state=cancelled", port))
        .send()
        .await
        .unwrap();

    let json: Value = response.json().await.unwrap();
    assert_eq!(json["total"], 1);

    server.kill().await.ok();
}

#[tokio::test]
async fn test_list_tickets_pagination() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create 5 tickets
    for i in 0..5 {
        client
            .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
            .json(&json!({
                "query_context": { "tags": [], "description": format!("ticket {}", i) },
                "dest_path": format!("/test/{}", i)
            }))
            .send()
            .await
            .unwrap();
    }

    // Get first page
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets?limit=2&offset=0", port))
        .send()
        .await
        .unwrap();

    let json: Value = response.json().await.unwrap();
    assert_eq!(json["total"], 5);
    assert_eq!(json["limit"], 2);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["tickets"].as_array().unwrap().len(), 2);

    // Get second page
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets?limit=2&offset=2", port))
        .send()
        .await
        .unwrap();

    let json: Value = response.json().await.unwrap();
    assert_eq!(json["tickets"].as_array().unwrap().len(), 2);

    // Get last page
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/tickets?limit=2&offset=4", port))
        .send()
        .await
        .unwrap();

    let json: Value = response.json().await.unwrap();
    assert_eq!(json["tickets"].as_array().unwrap().len(), 1);

    server.kill().await.ok();
}

#[tokio::test]
async fn test_cancel_ticket() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create a ticket
    let create_response = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "query_context": { "tags": [], "description": "to be cancelled" },
            "dest_path": "/test/cancel"
        }))
        .send()
        .await
        .unwrap();

    let created: Value = create_response.json().await.unwrap();
    let ticket_id = created["id"].as_str().unwrap();

    // Cancel the ticket
    let cancel_response = client
        .delete(format!("http://127.0.0.1:{}/api/v1/tickets/{}", port, ticket_id))
        .json(&json!({ "reason": "Testing cancellation" }))
        .send()
        .await
        .unwrap();

    assert_eq!(cancel_response.status(), 200);

    let json: Value = cancel_response.json().await.unwrap();
    assert_eq!(json["state"]["type"], "cancelled");
    assert_eq!(json["state"]["reason"], "Testing cancellation");
    assert_eq!(json["state"]["cancelled_by"], "anonymous");

    server.kill().await.ok();
}

#[tokio::test]
async fn test_cancel_already_cancelled_ticket() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create a ticket
    let create_response = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "query_context": { "tags": [], "description": "test" },
            "dest_path": "/test"
        }))
        .send()
        .await
        .unwrap();

    let created: Value = create_response.json().await.unwrap();
    let ticket_id = created["id"].as_str().unwrap();

    // Cancel once
    client
        .delete(format!("http://127.0.0.1:{}/api/v1/tickets/{}", port, ticket_id))
        .send()
        .await
        .unwrap();

    // Try to cancel again
    let response = client
        .delete(format!("http://127.0.0.1:{}/api/v1/tickets/{}", port, ticket_id))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 409);

    let json: Value = response.json().await.unwrap();
    assert!(json["error"].as_str().unwrap().contains("cancelled"));

    server.kill().await.ok();
}

#[tokio::test]
async fn test_cancel_nonexistent_ticket() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();
    let response = client
        .delete(format!("http://127.0.0.1:{}/api/v1/tickets/nonexistent-id", port))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);

    server.kill().await.ok();
}

#[tokio::test]
async fn test_ticket_creates_audit_event() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create a ticket
    let create_response = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "priority": 50,
            "query_context": {
                "tags": ["music"],
                "description": "Test for audit"
            },
            "dest_path": "/media/test"
        }))
        .send()
        .await
        .unwrap();

    let created: Value = create_response.json().await.unwrap();
    let ticket_id = created["id"].as_str().unwrap();

    // Give audit writer time to process
    sleep(Duration::from_millis(100)).await;

    // Check audit log for ticket_created event
    let audit_response = client
        .get(format!("http://127.0.0.1:{}/api/v1/audit?event_type=ticket_created", port))
        .send()
        .await
        .unwrap();

    let json: Value = audit_response.json().await.unwrap();
    let events = json["events"].as_array().unwrap();

    let ticket_event = events
        .iter()
        .find(|e| e["data"]["ticket_id"] == ticket_id);

    assert!(ticket_event.is_some(), "Should have ticket_created event");

    let event = ticket_event.unwrap();
    assert_eq!(event["data"]["priority"], 50);
    assert_eq!(event["data"]["tags"][0], "music");

    server.kill().await.ok();
}

#[tokio::test]
async fn test_cancel_creates_audit_event() {
    let (port, mut server, _temp_dir) = start_test_server().await;

    let client = Client::new();

    // Create and cancel a ticket
    let create_response = client
        .post(format!("http://127.0.0.1:{}/api/v1/tickets", port))
        .json(&json!({
            "query_context": { "tags": [], "description": "test" },
            "dest_path": "/test"
        }))
        .send()
        .await
        .unwrap();

    let created: Value = create_response.json().await.unwrap();
    let ticket_id = created["id"].as_str().unwrap();

    client
        .delete(format!("http://127.0.0.1:{}/api/v1/tickets/{}", port, ticket_id))
        .json(&json!({ "reason": "Audit test" }))
        .send()
        .await
        .unwrap();

    // Give audit writer time to process
    sleep(Duration::from_millis(100)).await;

    // Check audit log for ticket_cancelled event
    let audit_response = client
        .get(format!("http://127.0.0.1:{}/api/v1/audit?event_type=ticket_cancelled", port))
        .send()
        .await
        .unwrap();

    let json: Value = audit_response.json().await.unwrap();
    let events = json["events"].as_array().unwrap();

    let cancel_event = events
        .iter()
        .find(|e| e["data"]["ticket_id"] == ticket_id);

    assert!(cancel_event.is_some(), "Should have ticket_cancelled event");

    let event = cancel_event.unwrap();
    assert_eq!(event["data"]["reason"], "Audit test");
    assert_eq!(event["data"]["previous_state"], "pending");

    server.kill().await.ok();
}
