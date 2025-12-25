mod api;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use torrentino_core::{create_authenticator, load_config, validate_config, Authenticator};

use api::create_router;
use state::AppState;

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
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
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
    validate_config(&config).context("Configuration validation failed")?;

    info!("Configuration loaded successfully");
    info!("Auth method: {:?}", config.auth.method);

    // Create authenticator
    let authenticator: Arc<dyn Authenticator> =
        Arc::from(create_authenticator(&config.auth.method));
    info!("Using authenticator: {}", authenticator.method_name());

    // Create app state
    let state = Arc::new(AppState::new(config.clone(), authenticator));

    // Create router
    let app = create_router(state);

    // Start server
    let addr = SocketAddr::new(config.server.host, config.server.port);
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    axum::serve(listener, app).await.context("Server error")?;

    Ok(())
}
