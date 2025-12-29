//! Pipeline integration tests.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use tempfile::NamedTempFile;
use tokio::time::sleep;

/// Find an available port
fn get_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Create a minimal valid config
fn minimal_config(port: u16) -> String {
    format!(
        r#"
[auth]
method = "none"

[server]
host = "127.0.0.1"
port = {}
"#,
        port
    )
}

/// Spawn the server and return a handle
async fn spawn_server(config_path: &std::path::Path) -> tokio::process::Child {
    tokio::process::Command::new(env!("CARGO_BIN_EXE_quentin"))
        .env("QUENTIN_CONFIG", config_path)
        .env("RUST_LOG", "error")
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
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn test_pipeline_status_returns_running() {
    let port = get_available_port();
    let config = minimal_config(port);

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut _server = spawn_server(config_file.path()).await;

    assert!(wait_for_server(port, 50).await, "Server failed to start");

    let client = Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/pipeline/status", port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let body: Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(body["available"], true);
    assert_eq!(body["running"], true);
    assert!(body["conversion_pool"].is_object());
    assert!(body["placement_pool"].is_object());
}

#[tokio::test]
async fn test_pipeline_converter_info() {
    let port = get_available_port();
    let config = minimal_config(port);

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut _server = spawn_server(config_file.path()).await;

    assert!(wait_for_server(port, 50).await, "Server failed to start");

    let client = Client::new();
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/pipeline/converter",
            port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let body: Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(body["available"], true);
    assert_eq!(body["name"], "ffmpeg");
    assert!(body["supported_input_formats"].is_array());
    assert!(body["supported_output_formats"].is_array());
}

#[tokio::test]
async fn test_pipeline_placer_info() {
    let port = get_available_port();
    let config = minimal_config(port);

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut _server = spawn_server(config_file.path()).await;

    assert!(wait_for_server(port, 50).await, "Server failed to start");

    let client = Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/pipeline/placer", port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let body: Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(body["available"], true);
    assert_eq!(body["name"], "fs");
    assert!(body["config"].is_object());
}

#[tokio::test]
async fn test_pipeline_progress_nonexistent_ticket() {
    let port = get_available_port();
    let config = minimal_config(port);

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut _server = spawn_server(config_file.path()).await;

    assert!(wait_for_server(port, 50).await, "Server failed to start");

    let client = Client::new();
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/pipeline/progress/nonexistent-ticket",
            port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);

    let body: Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(body["phase"], "unknown");
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn test_pipeline_process_nonexistent_ticket() {
    let port = get_available_port();
    let config = minimal_config(port);

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut _server = spawn_server(config_file.path()).await;

    assert!(wait_for_server(port, 50).await, "Server failed to start");

    let client = Client::new();
    let response = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/pipeline/process/nonexistent-ticket",
            port
        ))
        .json(&serde_json::json!({
            "source_files": [],
            "dest_dir": "/tmp/test"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);

    let body: Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(body["success"], false);
    assert!(body["message"].as_str().unwrap().contains("not found"));
}
