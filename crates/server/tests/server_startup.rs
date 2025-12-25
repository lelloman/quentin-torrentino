use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use reqwest::Client;
use tempfile::NamedTempFile;
use tokio::time::{sleep, timeout};

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
async fn test_health_endpoint() {
    let port = get_available_port();
    let config_content = minimal_config(port);

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

    // Test health endpoint
    let client = Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/health", port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(json["status"], "ok");

    // Cleanup
    server.kill().await.ok();
}

#[tokio::test]
async fn test_config_endpoint_returns_sanitized() {
    let port = get_available_port();
    let config_content = minimal_config(port);

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

    // Test config endpoint
    let client = Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/config", port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(json["auth"]["method"], "none");
    assert_eq!(json["server"]["port"], port);

    // Cleanup
    server.kill().await.ok();
}

#[tokio::test]
async fn test_missing_config_file_exits_with_error() {
    let result = timeout(
        Duration::from_secs(5),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_quentin"))
            .env("QUENTIN_CONFIG", "/nonexistent/config.toml")
            .env("RUST_LOG", "error")
            .output(),
    )
    .await
    .expect("Command timed out")
    .expect("Failed to execute command");

    assert!(!result.status.success());
}

#[tokio::test]
async fn test_missing_auth_section_exits_with_error() {
    let config_without_auth = r#"
[server]
port = 8080
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_without_auth.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let result = timeout(
        Duration::from_secs(5),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_quentin"))
            .env("QUENTIN_CONFIG", temp_file.path())
            .env("RUST_LOG", "error")
            .output(),
    )
    .await
    .expect("Command timed out")
    .expect("Failed to execute command");

    assert!(!result.status.success());
}
