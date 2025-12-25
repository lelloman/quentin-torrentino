use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use reqwest::Client;
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

#[tokio::test]
async fn test_server_creates_database_file() {
    let port = get_available_port();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config_content = config_with_db(port, db_path.to_str().unwrap());

    // Write temp config file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Start server
    let mut server = spawn_server(temp_file.path()).await;

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 40).await,
        "Server did not start in time"
    );

    // Verify database file was created
    assert!(
        db_path.exists(),
        "Database file should be created on startup"
    );

    // Cleanup
    server.kill().await.ok();
}

#[tokio::test]
async fn test_audit_endpoint_returns_service_started_event() {
    let port = get_available_port();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config_content = config_with_db(port, db_path.to_str().unwrap());

    // Write temp config file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Start server
    let mut server = spawn_server(temp_file.path()).await;

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 40).await,
        "Server did not start in time"
    );

    // Give the audit writer a moment to write the event
    sleep(Duration::from_millis(100)).await;

    // Query audit events
    let client = Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/audit", port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify we have events
    let events = json["events"]
        .as_array()
        .expect("events should be an array");
    assert!(!events.is_empty(), "Should have at least one event");

    // Verify ServiceStarted event exists
    let service_started = events.iter().find(|e| e["event_type"] == "service_started");
    assert!(
        service_started.is_some(),
        "Should have a service_started event"
    );

    // Verify event data
    let event = service_started.unwrap();
    assert!(event["data"]["version"].is_string());
    assert!(event["data"]["config_hash"].is_string());

    // Cleanup
    server.kill().await.ok();
}

#[tokio::test]
async fn test_audit_query_filter_by_event_type() {
    let port = get_available_port();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config_content = config_with_db(port, db_path.to_str().unwrap());

    // Write temp config file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Start server
    let mut server = spawn_server(temp_file.path()).await;

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 40).await,
        "Server did not start in time"
    );

    // Give the audit writer a moment to write the event
    sleep(Duration::from_millis(100)).await;

    // Query with filter
    let client = Client::new();
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/audit?event_type=service_started",
            port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let events = json["events"]
        .as_array()
        .expect("events should be an array");

    // All returned events should be service_started
    for event in events {
        assert_eq!(event["event_type"], "service_started");
    }

    // Query for non-existent event type
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/audit?event_type=ticket_created",
            port
        ))
        .send()
        .await
        .expect("Failed to send request");

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let events = json["events"]
        .as_array()
        .expect("events should be an array");
    assert!(events.is_empty(), "Should have no ticket_created events");

    // Cleanup
    server.kill().await.ok();
}

#[tokio::test]
async fn test_audit_query_pagination() {
    let port = get_available_port();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config_content = config_with_db(port, db_path.to_str().unwrap());

    // Write temp config file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Start server
    let mut server = spawn_server(temp_file.path()).await;

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 40).await,
        "Server did not start in time"
    );

    // Give the audit writer a moment to write the event
    sleep(Duration::from_millis(100)).await;

    // Query with limit
    let client = Client::new();
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/audit?limit=10&offset=0",
            port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify pagination fields
    assert!(json["total"].is_i64());
    assert_eq!(json["limit"], 10);
    assert_eq!(json["offset"], 0);

    // Cleanup
    server.kill().await.ok();
}

#[tokio::test]
async fn test_database_persists_across_restarts() {
    let port1 = get_available_port();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config_content = config_with_db(port1, db_path.to_str().unwrap());

    // Write temp config file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Start server first time
    let mut server1 = spawn_server(temp_file.path()).await;
    assert!(
        wait_for_server(port1, 40).await,
        "Server 1 did not start in time"
    );
    sleep(Duration::from_millis(100)).await;

    // Stop first server
    server1.kill().await.ok();
    sleep(Duration::from_millis(100)).await;

    // Start server second time on different port
    let port2 = get_available_port();
    let config_content2 = config_with_db(port2, db_path.to_str().unwrap());
    let mut temp_file2 = NamedTempFile::new().unwrap();
    temp_file2.write_all(config_content2.as_bytes()).unwrap();
    temp_file2.flush().unwrap();

    let mut server2 = spawn_server(temp_file2.path()).await;
    assert!(
        wait_for_server(port2, 40).await,
        "Server 2 did not start in time"
    );
    sleep(Duration::from_millis(100)).await;

    // Query audit events - should have events from both starts
    let client = Client::new();
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/audit?event_type=service_started",
            port2
        ))
        .send()
        .await
        .expect("Failed to send request");

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let events = json["events"]
        .as_array()
        .expect("events should be an array");

    // Should have at least 2 service_started events (one from each start)
    assert!(
        events.len() >= 2,
        "Should have at least 2 service_started events after restart, got {}",
        events.len()
    );

    // Cleanup
    server2.kill().await.ok();
}
