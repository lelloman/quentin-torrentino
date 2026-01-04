//! Common test utilities for E2E testing with mocks.
//!
//! This module provides a test fixture that creates an in-process server
//! with mock dependencies injected, enabling comprehensive E2E testing
//! without external infrastructure.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use torrentino_core::{
    create_audit_system, AuditStore, AuthMethod, Config, DatabaseConfig, EncoderCapabilities,
    FfmpegConverter, FsPlacer, NoneAuthenticator, OrchestratorConfig, PipelineProcessor,
    PlacerConfig, ProcessorConfig, ServerConfig, SqliteAuditStore, SqliteCatalog,
    SqliteTicketStore, TextBrainConfig,
    testing::{MockExternalCatalog, MockSearcher, MockTorrentClient},
};

/// Re-export fixtures for test convenience
pub use torrentino_core::testing::fixtures;

/// Test fixture for E2E testing with mock dependencies.
///
/// Provides an in-process server with fully controllable mocks for:
/// - Torrent search (MockSearcher)
/// - Torrent client (MockTorrentClient)
/// - External catalogs (MockExternalCatalog)
///
/// # Example
///
/// ```rust,ignore
/// #[tokio::test]
/// async fn test_ticket_creation() {
///     let fixture = TestFixture::new().await;
///
///     let response = fixture.post("/api/v1/tickets", json!({
///         "query_context": { "description": "Test" },
///         "dest_path": "/test"
///     })).await;
///
///     assert_eq!(response.status, 201);
/// }
/// ```
pub struct TestFixture {
    /// The Axum router for testing
    pub router: Router,
    /// Mock searcher - configure search results
    pub searcher: Arc<MockSearcher>,
    /// Mock torrent client - control downloads
    pub torrent_client: Arc<MockTorrentClient>,
    /// Mock external catalog - configure MusicBrainz/TMDB responses
    pub external_catalog: Arc<MockExternalCatalog>,
    /// Temporary directory for test database and pipeline output
    pub temp_dir: TempDir,
    /// Pipeline output directory (if pipeline enabled)
    pub output_dir: Option<PathBuf>,
}

/// Response from a test request
#[derive(Debug)]
pub struct TestResponse {
    pub status: StatusCode,
    pub body: Value,
}

impl TestFixture {
    /// Create a new test fixture with default mocks.
    pub async fn new() -> Self {
        Self::with_config(TestConfig::default()).await
    }

    /// Create a test fixture with custom configuration.
    pub async fn with_config(test_config: TestConfig) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        // Create mocks
        let searcher = Arc::new(MockSearcher::new());
        let torrent_client = Arc::new(MockTorrentClient::new());
        let external_catalog = Arc::new(MockExternalCatalog::new());

        // Create config
        let config = Config {
            auth: torrentino_core::AuthConfig {
                method: AuthMethod::None,
                api_key: None,
            },
            server: ServerConfig {
                host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                port: 0, // Not used for in-process testing
            },
            database: DatabaseConfig {
                path: db_path.clone(),
            },
            searcher: None,
            torrent_client: None,
            textbrain: TextBrainConfig::default(),
            orchestrator: OrchestratorConfig {
                enabled: test_config.enable_orchestrator,
                ..Default::default()
            },
            external_catalogs: None,
        };

        // Create stores
        let audit_store: Arc<dyn AuditStore> = Arc::new(
            SqliteAuditStore::new(&db_path).expect("Failed to create audit store"),
        );
        let ticket_store = Arc::new(
            SqliteTicketStore::new(&db_path).expect("Failed to create ticket store"),
        );
        let catalog = Arc::new(
            SqliteCatalog::new(&db_path).expect("Failed to create catalog"),
        );

        // Create audit system
        let (audit_handle, audit_writer) = create_audit_system(Arc::clone(&audit_store), 100);

        // Spawn audit writer
        tokio::spawn(audit_writer.run());

        // Create WebSocket broadcaster
        let ws_broadcaster = torrentino_server::api::WsBroadcaster::default();

        // Optionally create pipeline with real FFmpeg/FS (pointing to temp dirs)
        let (pipeline, output_dir) = if test_config.enable_pipeline {
            let output_path = temp_dir.path().join("output");
            std::fs::create_dir_all(&output_path).expect("Failed to create output dir");

            let converter = FfmpegConverter::with_defaults();
            let placer = FsPlacer::new(PlacerConfig::default());
            let processor_config = ProcessorConfig::default();
            let pipeline = PipelineProcessor::new(processor_config, converter, placer);

            (Some(Arc::new(pipeline)), Some(output_path))
        } else {
            (None, None)
        };

        // Create app state with mocks
        let state = Arc::new(torrentino_server::state::AppState::new(
            config,
            Arc::new(NoneAuthenticator),
            audit_handle,
            audit_store,
            ticket_store,
            Some(Arc::clone(&searcher) as Arc<dyn torrentino_core::Searcher>),
            Some(Arc::clone(&torrent_client) as Arc<dyn torrentino_core::TorrentClient>),
            catalog,
            pipeline,
            None, // No orchestrator for basic tests (requires more setup)
            Some(Arc::clone(&external_catalog) as Arc<dyn torrentino_core::ExternalCatalog>),
            ws_broadcaster,
            EncoderCapabilities::default(),
        ));

        // Create router
        let router = torrentino_server::api::create_router(state);

        Self {
            router,
            searcher,
            torrent_client,
            external_catalog,
            temp_dir,
            output_dir,
        }
    }

    /// Send a GET request to the test server.
    pub async fn get(&self, path: &str) -> TestResponse {
        self.request("GET", path, None).await
    }

    /// Send a POST request with JSON body.
    pub async fn post(&self, path: &str, body: Value) -> TestResponse {
        self.request("POST", path, Some(body)).await
    }

    /// Send a PUT request with JSON body.
    pub async fn put(&self, path: &str, body: Value) -> TestResponse {
        self.request("PUT", path, Some(body)).await
    }

    /// Send a DELETE request.
    pub async fn delete(&self, path: &str) -> TestResponse {
        self.request("DELETE", path, None).await
    }

    /// Send a DELETE request with JSON body.
    pub async fn delete_with_body(&self, path: &str, body: Value) -> TestResponse {
        self.request("DELETE", path, Some(body)).await
    }

    /// Send a POST request with raw string body (for testing malformed JSON).
    pub async fn post_raw(&self, path: &str, body: &str) -> TestResponse {
        self.request_raw("POST", path, body, "application/json").await
    }

    /// Send a POST request with custom content type (for testing wrong content types).
    pub async fn post_with_content_type(
        &self,
        path: &str,
        body: &str,
        content_type: &str,
    ) -> TestResponse {
        self.request_raw("POST", path, body, content_type).await
    }

    /// Send a request with raw string body and custom content type.
    async fn request_raw(
        &self,
        method: &str,
        path: &str,
        body: &str,
        content_type: &str,
    ) -> TestResponse {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("Content-Type", content_type)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to send request");

        let status = response.status();
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .expect("Failed to collect body")
            .to_bytes();

        let body: Value = if body_bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
        };

        TestResponse { status, body }
    }

    /// Send a request to the test server.
    async fn request(&self, method: &str, path: &str, body: Option<Value>) -> TestResponse {
        let mut request_builder = Request::builder()
            .method(method)
            .uri(path);

        let body = if let Some(json_body) = body {
            request_builder = request_builder.header("Content-Type", "application/json");
            Body::from(serde_json::to_vec(&json_body).unwrap())
        } else {
            Body::empty()
        };

        let request = request_builder.body(body).unwrap();

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to send request");

        let status = response.status();
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .expect("Failed to collect body")
            .to_bytes();

        let body: Value = if body_bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
        };

        TestResponse { status, body }
    }
}

/// Configuration for test fixture.
#[derive(Debug, Clone, Default)]
pub struct TestConfig {
    /// Enable the orchestrator for lifecycle tests
    pub enable_orchestrator: bool,
    /// Enable the pipeline (FFmpeg converter + FS placer)
    pub enable_pipeline: bool,
}

impl TestConfig {
    /// Create config with orchestrator enabled.
    pub fn with_orchestrator() -> Self {
        Self {
            enable_orchestrator: true,
            enable_pipeline: false,
        }
    }

    /// Create config with pipeline enabled.
    pub fn with_pipeline() -> Self {
        Self {
            enable_orchestrator: false,
            enable_pipeline: true,
        }
    }

    /// Create config with both orchestrator and pipeline enabled.
    pub fn with_all() -> Self {
        Self {
            enable_orchestrator: true,
            enable_pipeline: true,
        }
    }
}

/// Helper to assert a response has expected status.
#[macro_export]
macro_rules! assert_status {
    ($response:expr, $status:expr) => {
        assert_eq!(
            $response.status, $status,
            "Expected status {:?}, got {:?}. Body: {}",
            $status,
            $response.status,
            serde_json::to_string_pretty(&$response.body).unwrap_or_default()
        );
    };
}

/// Helper to assert a JSON path equals expected value.
#[macro_export]
macro_rules! assert_json_path {
    ($json:expr, $path:expr, $expected:expr) => {
        let actual = &$json[$path];
        assert_eq!(
            actual, &$expected,
            "Path '{}' expected {:?}, got {:?}",
            $path, $expected, actual
        );
    };
}
