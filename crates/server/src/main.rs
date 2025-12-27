mod api;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use torrentino_core::{
    create_audit_system, create_authenticator, load_config, validate_config, AuditEvent,
    AuditStore, Authenticator, JackettSearcher, LibrqbitClient, QBittorrentClient, Searcher,
    SearcherBackend, SqliteAuditStore, SqliteCatalog, SqliteTicketStore, TicketStore,
    TorrentCatalog, TorrentClient, TorrentClientBackend,
};

use api::create_router;
use state::AppState;

/// Application version
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Buffer size for audit event channel
const AUDIT_BUFFER_SIZE: usize = 1000;

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
    info!("Database path: {:?}", config.database.path);

    // Compute config hash for audit
    let config_json = serde_json::to_string(&config).unwrap_or_default();
    let config_hash = format!("{:x}", Sha256::digest(config_json.as_bytes()));
    let config_hash_short = &config_hash[..16];

    // Create authenticator
    let authenticator: Arc<dyn Authenticator> =
        Arc::from(create_authenticator(&config.auth.method));
    info!("Using authenticator: {}", authenticator.method_name());

    // Create SQLite audit store
    let audit_store: Arc<dyn AuditStore> = Arc::new(
        SqliteAuditStore::new(&config.database.path).context("Failed to create audit store")?,
    );
    info!("Audit store initialized");

    // Create SQLite ticket store
    let ticket_store: Arc<dyn TicketStore> = Arc::new(
        SqliteTicketStore::new(&config.database.path).context("Failed to create ticket store")?,
    );
    info!("Ticket store initialized");

    // Create SQLite catalog (torrent search result cache)
    let catalog: Arc<dyn TorrentCatalog> = Arc::new(
        SqliteCatalog::new(&config.database.path).context("Failed to create torrent catalog")?,
    );
    info!("Torrent catalog initialized");

    // Create audit system
    let (audit_handle, audit_writer) =
        create_audit_system(Arc::clone(&audit_store), AUDIT_BUFFER_SIZE);

    // Spawn audit writer task
    let writer_handle = tokio::spawn(audit_writer.run());

    // Emit ServiceStarted event
    audit_handle
        .emit(AuditEvent::ServiceStarted {
            version: VERSION.to_string(),
            config_hash: config_hash_short.to_string(),
        })
        .await;
    info!("Emitted ServiceStarted audit event");

    // Create searcher if configured
    let searcher: Option<Arc<dyn Searcher>> = match &config.searcher {
        Some(searcher_config) => match searcher_config.backend {
            SearcherBackend::Jackett => {
                if let Some(jackett_config) = &searcher_config.jackett {
                    info!(
                        "Initializing Jackett searcher (indexers auto-discovered from Jackett)"
                    );
                    Some(Arc::new(JackettSearcher::new(jackett_config.clone())))
                } else {
                    error!("Jackett backend selected but no jackett config provided");
                    None
                }
            }
        },
        None => {
            info!("No searcher configured");
            None
        }
    };

    // Create torrent client if configured
    let torrent_client: Option<Arc<dyn TorrentClient>> = match &config.torrent_client {
        Some(tc_config) => match tc_config.backend {
            TorrentClientBackend::QBittorrent => {
                if let Some(qbit_config) = &tc_config.qbittorrent {
                    info!("Initializing qBittorrent client at {}", qbit_config.url);
                    Some(Arc::new(QBittorrentClient::new(qbit_config.clone())))
                } else {
                    error!("qBittorrent backend selected but no qbittorrent config provided");
                    None
                }
            }
            TorrentClientBackend::Librqbit => {
                if let Some(librqbit_config) = &tc_config.librqbit {
                    info!(
                        "Initializing embedded librqbit client (download path: {})",
                        librqbit_config.download_path
                    );
                    match LibrqbitClient::new(librqbit_config).await {
                        Ok(client) => Some(Arc::new(client)),
                        Err(e) => {
                            error!("Failed to initialize librqbit client: {}", e);
                            None
                        }
                    }
                } else {
                    error!("librqbit backend selected but no librqbit config provided");
                    None
                }
            }
        },
        None => {
            info!("No torrent client configured");
            None
        }
    };

    // Create app state
    let state = Arc::new(AppState::new(
        config.clone(),
        authenticator,
        audit_handle.clone(),
        audit_store,
        ticket_store,
        searcher,
        torrent_client,
        catalog,
    ));

    // Create router
    let app = create_router(state);

    // Start server
    let addr = SocketAddr::new(config.server.host, config.server.port);
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server error")?;

    // Emit ServiceStopped event
    info!("Server shutting down...");
    audit_handle
        .emit(AuditEvent::ServiceStopped {
            reason: "graceful_shutdown".to_string(),
        })
        .await;

    // Close the audit handle to signal the writer to stop
    drop(audit_handle);

    // Wait for writer to finish processing remaining events
    let _ = writer_handle.await;
    info!("Audit writer stopped");

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
