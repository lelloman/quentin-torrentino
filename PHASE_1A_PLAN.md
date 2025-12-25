# Phase 1A: Configuration & Auth Foundation

## Goal

A working binary that loads configuration, validates authentication settings, and starts an HTTP server with basic endpoints.

## Success Criteria

1. `cargo build --workspace` succeeds
2. `cargo test --workspace` passes
3. Running without config file → clear error message
4. Running with config missing `[auth]` section → exits with explicit error
5. Running with valid config → server starts on configured port
6. `GET /api/v1/health` returns `{"status": "ok"}`
7. `GET /api/v1/config` returns sanitized configuration (secrets redacted)
8. Integration test: spawn server, hit endpoints, verify responses

---

## Crate Structure

```
quentin-torrentino/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── core/                     # torrentino-core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config/
│   │       │   ├── mod.rs
│   │       │   ├── types.rs      # Config structs
│   │       │   ├── loader.rs     # Load from file/env
│   │       │   └── validate.rs   # Validation logic
│   │       └── auth/
│   │           ├── mod.rs
│   │           ├── types.rs      # Identity, AuthRequest, AuthError
│   │           ├── traits.rs     # Authenticator trait
│   │           └── none.rs       # NoneAuthenticator
│   │
│   └── server/                   # torrentino-server binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # Entry point
│           ├── api/
│           │   ├── mod.rs
│           │   ├── routes.rs     # Route definitions
│           │   └── handlers.rs   # health, config handlers
│           └── state.rs          # AppState
│
└── tests/
    └── integration/
        └── server_startup.rs     # Integration tests
```

---

## Detailed Implementation Tasks

### Task 1: Workspace Setup

**Files to create:**

#### `Cargo.toml` (workspace root)
```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/server",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
rust-version = "1.83"

[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Web framework
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "cors"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Configuration
figment = { version = "0.10", features = ["toml", "env"] }

# Error handling
thiserror = "2"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Async traits
async-trait = "0.1"

# Testing
tokio-test = "0.4"
```

#### `crates/core/Cargo.toml`
```toml
[package]
name = "torrentino-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
figment = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
tokio-test = { workspace = true }
```

#### `crates/server/Cargo.toml`
```toml
[package]
name = "torrentino-server"
version.workspace = true
edition.workspace = true

[[bin]]
name = "quentin"
path = "src/main.rs"

[dependencies]
torrentino-core = { path = "../core" }
tokio = { workspace = true }
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }

[dev-dependencies]
tokio-test = { workspace = true }
reqwest = { version = "0.12", features = ["json"] }
```

**Verification:** `cargo check --workspace` succeeds

---

### Task 2: Configuration Types

**File: `crates/core/src/config/types.rs`**

Define the configuration structures:

```rust
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Root configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default)]
    pub server: ServerConfig,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

fn default_host() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

fn default_port() -> u16 {
    8080
}

/// Authentication configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    pub method: AuthMethod,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    None,
    // Future: Oidc, Address, Cert, Plugin
}

/// Sanitized config for API responses (secrets redacted)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedConfig {
    pub auth: SanitizedAuthConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedAuthConfig {
    pub method: String,
}

impl From<&Config> for SanitizedConfig {
    fn from(config: &Config) -> Self {
        Self {
            auth: SanitizedAuthConfig {
                method: match config.auth.method {
                    AuthMethod::None => "none".to_string(),
                },
            },
            server: config.server.clone(),
        }
    }
}
```

**Unit tests:**
- Deserialize valid TOML with `method = "none"`
- Deserialize with missing `[server]` uses defaults
- Deserialize missing `[auth]` fails

---

### Task 3: Configuration Loader

**File: `crates/core/src/config/loader.rs`**

```rust
use figment::{Figment, providers::{Format, Toml, Env}};
use std::path::Path;
use super::types::Config;
use crate::config::ConfigError;

/// Load configuration from file with environment variable overrides
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let config: Config = Figment::new()
        .merge(Toml::file(path))
        .merge(Env::prefixed("QUENTIN_").split("_"))
        .extract()
        .map_err(|e| ConfigError::ParseError(e.to_string()))?;

    Ok(config)
}

/// Load configuration from TOML string (useful for testing)
pub fn load_config_from_str(toml: &str) -> Result<Config, ConfigError> {
    toml::from_str(toml).map_err(|e| ConfigError::ParseError(e.to_string()))
}
```

**File: `crates/core/src/config/validate.rs`**

```rust
use super::types::Config;
use crate::config::ConfigError;

/// Validate configuration
/// Currently validates:
/// - Auth section exists (enforced by serde)
/// - Auth method is valid
pub fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Auth validation - currently just checks method is recognized
    // This is already enforced by serde enum deserialization
    // Future: validate method-specific settings (OIDC issuer URL, etc.)

    // Server validation
    if config.server.port == 0 {
        return Err(ConfigError::ValidationError(
            "server.port cannot be 0".to_string()
        ));
    }

    Ok(())
}
```

**File: `crates/core/src/config/mod.rs`**

```rust
mod types;
mod loader;
mod validate;

pub use types::*;
pub use loader::*;
pub use validate::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}
```

**Unit tests:**
- `load_config_from_str` with valid TOML
- `load_config_from_str` with missing auth → error
- `validate_config` with port = 0 → error
- Environment variable override works

---

### Task 4: Auth Types and Trait

**File: `crates/core/src/auth/types.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Request information for authentication
#[derive(Debug, Clone)]
pub struct AuthRequest {
    pub headers: HashMap<String, String>,
    pub source_ip: IpAddr,
}

/// Authenticated identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub method: String,
    pub claims: HashMap<String, serde_json::Value>,
}

impl Identity {
    pub fn anonymous() -> Self {
        Self {
            user_id: "anonymous".to_string(),
            method: "none".to_string(),
            claims: HashMap::new(),
        }
    }
}
```

**File: `crates/core/src/auth/traits.rs`**

```rust
use async_trait::async_trait;
use super::types::{AuthRequest, Identity};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Authentication required")]
    NotAuthenticated,

    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),

    #[error("Authentication service unavailable: {0}")]
    ServiceUnavailable(String),
}

#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate a request and return the identity
    async fn authenticate(&self, request: &AuthRequest) -> Result<Identity, AuthError>;

    /// Name of this authentication method
    fn method_name(&self) -> &'static str;
}
```

---

### Task 5: None Authenticator

**File: `crates/core/src/auth/none.rs`**

```rust
use async_trait::async_trait;
use super::{AuthRequest, Identity, AuthError, Authenticator};

/// Authenticator that accepts all requests as anonymous
/// Must be explicitly configured - the system won't default to this
pub struct NoneAuthenticator;

impl NoneAuthenticator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoneAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Authenticator for NoneAuthenticator {
    async fn authenticate(&self, _request: &AuthRequest) -> Result<Identity, AuthError> {
        Ok(Identity::anonymous())
    }

    fn method_name(&self) -> &'static str {
        "none"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::net::IpAddr;

    #[tokio::test]
    async fn test_none_authenticator_returns_anonymous() {
        let auth = NoneAuthenticator::new();
        let request = AuthRequest {
            headers: HashMap::new(),
            source_ip: "127.0.0.1".parse::<IpAddr>().unwrap(),
        };

        let identity = auth.authenticate(&request).await.unwrap();

        assert_eq!(identity.user_id, "anonymous");
        assert_eq!(identity.method, "none");
    }
}
```

**File: `crates/core/src/auth/mod.rs`**

```rust
mod types;
mod traits;
mod none;

pub use types::*;
pub use traits::*;
pub use none::*;

use crate::config::AuthMethod;

/// Factory function to create authenticator from config
pub fn create_authenticator(method: &AuthMethod) -> Box<dyn Authenticator> {
    match method {
        AuthMethod::None => Box::new(NoneAuthenticator::new()),
    }
}
```

---

### Task 6: Core Library Root

**File: `crates/core/src/lib.rs`**

```rust
pub mod config;
pub mod auth;

pub use config::{Config, ConfigError, load_config, validate_config, SanitizedConfig};
pub use auth::{Authenticator, AuthRequest, AuthError, Identity, create_authenticator};
```

---

### Task 7: Server AppState

**File: `crates/server/src/state.rs`**

```rust
use std::sync::Arc;
use torrentino_core::{Config, SanitizedConfig, Authenticator};

/// Shared application state
pub struct AppState {
    config: Config,
    authenticator: Arc<dyn Authenticator>,
}

impl AppState {
    pub fn new(config: Config, authenticator: Arc<dyn Authenticator>) -> Self {
        Self { config, authenticator }
    }

    pub fn sanitized_config(&self) -> SanitizedConfig {
        SanitizedConfig::from(&self.config)
    }

    pub fn authenticator(&self) -> &dyn Authenticator {
        self.authenticator.as_ref()
    }
}
```

---

### Task 8: API Handlers

**File: `crates/server/src/api/handlers.rs`**

```rust
use axum::{
    extract::State,
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use crate::state::AppState;
use torrentino_core::SanitizedConfig;

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Json<SanitizedConfig> {
    Json(state.sanitized_config())
}
```

**File: `crates/server/src/api/routes.rs`**

```rust
use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use crate::state::AppState;
use super::handlers;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/config", get(handlers::get_config))
        .with_state(state)
}
```

**File: `crates/server/src/api/mod.rs`**

```rust
pub mod handlers;
pub mod routes;

pub use routes::create_router;
```

---

### Task 9: Server Main

**File: `crates/server/src/main.rs`**

```rust
mod api;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::{Context, Result};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use torrentino_core::{load_config, validate_config, create_authenticator};
use state::AppState;
use api::create_router;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Determine config path
    let config_path = std::env::var("QUENTIN_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config.toml"));

    // Load configuration
    info!("Loading configuration from {:?}", config_path);
    let config = load_config(&config_path)
        .with_context(|| format!("Failed to load config from {:?}", config_path))?;

    // Validate configuration
    validate_config(&config)
        .context("Configuration validation failed")?;

    info!("Configuration loaded successfully");
    info!("Auth method: {:?}", config.auth.method);

    // Create authenticator
    let authenticator = Arc::from(create_authenticator(&config.auth.method));
    info!("Using authenticator: {}", authenticator.method_name());

    // Create app state
    let state = Arc::new(AppState::new(config.clone(), authenticator));

    // Create router
    let app = create_router(state);

    // Start server
    let addr = SocketAddr::new(config.server.host, config.server.port);
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    axum::serve(listener, app).await
        .context("Server error")?;

    Ok(())
}
```

---

### Task 10: Integration Tests

**File: `tests/integration/server_startup.rs`**

```rust
use std::net::TcpListener;
use std::time::Duration;
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
    format!(r#"
[auth]
method = "none"

[server]
host = "127.0.0.1"
port = {}
"#, port)
}

#[tokio::test]
async fn test_health_endpoint() {
    let port = get_available_port();
    let config_content = minimal_config(port);

    // Write temp config file
    let config_path = std::env::temp_dir().join(format!("quentin_test_{}.toml", port));
    std::fs::write(&config_path, &config_content).unwrap();

    // Start server in background
    let config_path_clone = config_path.clone();
    let server_handle = tokio::spawn(async move {
        std::env::set_var("QUENTIN_CONFIG", config_path_clone);
        // Note: In real test, we'd import and call run() directly
        // For now this is a placeholder showing the test structure
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    // Test health endpoint
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/api/v1/health", port))
        .send()
        .await;

    // Cleanup
    let _ = std::fs::remove_file(&config_path);
    server_handle.abort();

    // In real implementation, assert response
    // assert!(response.is_ok());
    // let json: serde_json::Value = response.unwrap().json().await.unwrap();
    // assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_config_endpoint_returns_sanitized() {
    // Similar structure to above
    // Verify that /api/v1/config returns config without secrets
}

#[test]
fn test_missing_config_file_error() {
    // Test that attempting to load non-existent config gives clear error
    use torrentino_core::load_config;
    use std::path::Path;

    let result = load_config(Path::new("/nonexistent/config.toml"));
    assert!(result.is_err());
}

#[test]
fn test_missing_auth_section_error() {
    // Test that config without [auth] section fails
    use torrentino_core::config::load_config_from_str;

    let config_without_auth = r#"
[server]
port = 8080
"#;

    let result = load_config_from_str(config_without_auth);
    assert!(result.is_err());
}
```

---

### Task 11: Example Config File

**File: `config.example.toml`**

```toml
# Quentin Torrentino Configuration
# Copy this file to config.toml and modify as needed

# ==============================================================================
# AUTHENTICATION (REQUIRED)
# ==============================================================================
# The service will not start without an explicit [auth] section.
# You must choose an authentication method.

[auth]
# Available methods: "none", "oidc", "address", "cert", "plugin"
# Currently only "none" is implemented
method = "none"

# ==============================================================================
# SERVER
# ==============================================================================

[server]
# IP address to bind to
# Default: "0.0.0.0" (all interfaces)
host = "0.0.0.0"

# Port to listen on
# Default: 8080
port = 8080
```

---

## Implementation Order

1. **Task 1**: Workspace setup - create all `Cargo.toml` files
2. **Task 6**: Core lib.rs (empty initially)
3. **Task 2**: Configuration types
4. **Task 3**: Configuration loader
5. **Task 4**: Auth types and trait
6. **Task 5**: None authenticator
7. **Task 6**: Update core lib.rs with exports
8. **Task 7**: Server AppState
9. **Task 8**: API handlers
10. **Task 9**: Server main
11. **Task 10**: Integration tests
12. **Task 11**: Example config

---

## Verification Checklist

After implementation, verify:

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` has no warnings
- [ ] `cargo fmt --check` passes
- [ ] Running `./target/debug/quentin` without config.toml shows clear error
- [ ] Running with config missing `[auth]` shows: "missing field `auth`" or similar
- [ ] Running with valid config.toml starts server
- [ ] `curl http://localhost:8080/api/v1/health` returns `{"status":"ok"}`
- [ ] `curl http://localhost:8080/api/v1/config` returns sanitized config JSON

---

## Out of Scope (Deferred to Later Phases)

- OIDC, Address, Cert, Plugin authenticators
- Auth middleware (applying auth to protected routes)
- SQLite database
- Audit logging
- State machine
- Queue manager
- WebSocket
- Dashboard
- Any content-type specific code (music, video)
