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
    AuditStore, Authenticator, CombinedCatalogClient, ConverterConfig, ExternalCatalog,
    FfmpegConverter, FsPlacer, JackettSearcher, LibrqbitClient, MusicBrainzClient,
    PipelineProcessor, PlacerConfig, ProcessorConfig, QBittorrentClient, Searcher, SearcherBackend,
    SqliteAuditStore, SqliteCatalog, SqliteTicketStore, TicketOrchestrator, TicketStore, TmdbClient,
    TorrentCatalog, TorrentClient, TorrentClientBackend,
};

use api::{create_router, WsBroadcaster};
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
    let authenticator: Arc<dyn Authenticator> = Arc::from(
        create_authenticator(&config.auth).context("Failed to create authenticator")?,
    );
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

    // Create pipeline processor
    // The processor is always created with default config for now
    // Future: Add [processor] config section to config.toml
    let processor_config = ProcessorConfig::default();
    let converter_config = ConverterConfig::default();
    let placer_config = PlacerConfig::default();

    let converter = FfmpegConverter::new(converter_config);
    let placer = FsPlacer::new(placer_config);

    let pipeline = PipelineProcessor::new(processor_config, converter, placer)
        .with_audit(audit_handle.clone())
        .with_ticket_store(Arc::clone(&ticket_store));

    // Start the pipeline processor
    pipeline.start().await;
    info!("Pipeline processor started");

    let pipeline = Arc::new(pipeline);

    // Create WebSocket broadcaster for real-time updates (before orchestrator so we can pass callback)
    let ws_broadcaster = WsBroadcaster::default();
    info!("WebSocket broadcaster initialized");

    // Create orchestrator if enabled
    let orchestrator = if config.orchestrator.enabled {
        match (&searcher, &torrent_client) {
            (Some(s), Some(tc)) => {
                info!("Initializing ticket orchestrator");

                // Create update callback that broadcasts via WebSocket
                let broadcaster_for_callback = ws_broadcaster.clone();
                let update_callback: torrentino_core::orchestrator::TicketUpdateCallback =
                    Arc::new(move |ticket_id: &str, state_type: &str| {
                        broadcaster_for_callback.ticket_updated(ticket_id, state_type);
                    });

                let orch = TicketOrchestrator::new(
                    config.orchestrator.clone(),
                    Arc::clone(&ticket_store),
                    Arc::clone(s),
                    Arc::clone(tc),
                    Arc::clone(&pipeline),
                    Arc::clone(&catalog),
                    Some(audit_handle.clone()),
                    config.textbrain.clone(),
                )
                .with_update_callback(update_callback);

                orch.start().await;
                info!("Ticket orchestrator started");
                Some(Arc::new(orch))
            }
            _ => {
                error!(
                    "Orchestrator enabled but missing dependencies (searcher: {}, torrent_client: {})",
                    searcher.is_some(),
                    torrent_client.is_some()
                );
                None
            }
        }
    } else {
        info!("Orchestrator disabled in config");
        None
    };

    let pipeline = Some(pipeline);

    // Initialize external catalog client if configured
    let external_catalog: Option<Arc<dyn ExternalCatalog>> =
        if let Some(ref ec_config) = config.external_catalogs {
            let mb_client = ec_config
                .musicbrainz
                .as_ref()
                .map(|mb_cfg| {
                    info!("Initializing MusicBrainz client");
                    MusicBrainzClient::new(mb_cfg.clone())
                })
                .transpose()
                .map_err(|e| error!("Failed to create MusicBrainz client: {}", e))
                .ok()
                .flatten();

            let tmdb_client = ec_config
                .tmdb
                .as_ref()
                .map(|tmdb_cfg| {
                    info!("Initializing TMDB client");
                    TmdbClient::new(tmdb_cfg.clone())
                })
                .transpose()
                .map_err(|e| error!("Failed to create TMDB client: {}", e))
                .ok()
                .flatten();

            if mb_client.is_some() || tmdb_client.is_some() {
                Some(Arc::new(CombinedCatalogClient::new(mb_client, tmdb_client)))
            } else {
                None
            }
        } else {
            info!("External catalogs not configured");
            None
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
        pipeline,
        orchestrator.clone(),
        external_catalog,
        ws_broadcaster,
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

    // Stop orchestrator if running
    if let Some(ref orch) = orchestrator {
        info!("Stopping orchestrator...");
        orch.stop().await;
        info!("Orchestrator stopped");
    }

    // Emit ServiceStopped event
    info!("Server shutting down...");
    audit_handle
        .emit(AuditEvent::ServiceStopped {
            reason: "graceful_shutdown".to_string(),
        })
        .await;

    // Drop all holders of AuditHandle so the writer's channel closes.
    // The orchestrator holds an AuditHandle clone, so we must drop it.
    // The pipeline was moved into AppState which is already dropped.
    // Order matters: we emit the final event BEFORE dropping handles.
    drop(orchestrator);
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
