use std::sync::Arc;
use torrentino_core::{
    AuditHandle, AuditStore, Authenticator, Config, EncoderCapabilities, ExternalCatalog,
    FfmpegConverter, FsPlacer, PipelineProcessor, SanitizedConfig, Searcher, TicketOrchestrator,
    TicketStore, TorrentCatalog, TorrentClient,
};

use crate::api::WsBroadcaster;

/// Type alias for the concrete pipeline processor we use
pub type AppPipelineProcessor = PipelineProcessor<FfmpegConverter, FsPlacer>;

/// Type alias for the concrete orchestrator we use
pub type AppOrchestrator = TicketOrchestrator<FfmpegConverter, FsPlacer>;

/// Shared application state
pub struct AppState {
    config: Config,
    authenticator: Arc<dyn Authenticator>,
    audit_handle: AuditHandle,
    audit_store: Arc<dyn AuditStore>,
    ticket_store: Arc<dyn TicketStore>,
    searcher: Option<Arc<dyn Searcher>>,
    torrent_client: Option<Arc<dyn TorrentClient>>,
    catalog: Arc<dyn TorrentCatalog>,
    pipeline: Option<Arc<AppPipelineProcessor>>,
    orchestrator: Option<Arc<AppOrchestrator>>,
    external_catalog: Option<Arc<dyn ExternalCatalog>>,
    ws_broadcaster: WsBroadcaster,
    encoder_capabilities: EncoderCapabilities,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Config,
        authenticator: Arc<dyn Authenticator>,
        audit_handle: AuditHandle,
        audit_store: Arc<dyn AuditStore>,
        ticket_store: Arc<dyn TicketStore>,
        searcher: Option<Arc<dyn Searcher>>,
        torrent_client: Option<Arc<dyn TorrentClient>>,
        catalog: Arc<dyn TorrentCatalog>,
        pipeline: Option<Arc<AppPipelineProcessor>>,
        orchestrator: Option<Arc<AppOrchestrator>>,
        external_catalog: Option<Arc<dyn ExternalCatalog>>,
        ws_broadcaster: WsBroadcaster,
        encoder_capabilities: EncoderCapabilities,
    ) -> Self {
        Self {
            config,
            authenticator,
            audit_handle,
            audit_store,
            ticket_store,
            searcher,
            torrent_client,
            catalog,
            pipeline,
            orchestrator,
            external_catalog,
            ws_broadcaster,
            encoder_capabilities,
        }
    }

    pub fn sanitized_config(&self) -> SanitizedConfig {
        SanitizedConfig::from(&self.config)
    }

    #[allow(dead_code)]
    pub fn authenticator(&self) -> &dyn Authenticator {
        self.authenticator.as_ref()
    }

    /// Get the audit handle for emitting events
    pub fn audit(&self) -> &AuditHandle {
        &self.audit_handle
    }

    /// Get the audit store for querying events
    pub fn audit_store(&self) -> &Arc<dyn AuditStore> {
        &self.audit_store
    }

    /// Get the ticket store
    pub fn ticket_store(&self) -> &Arc<dyn TicketStore> {
        &self.ticket_store
    }

    /// Get the searcher (if configured)
    pub fn searcher(&self) -> Option<&Arc<dyn Searcher>> {
        self.searcher.as_ref()
    }

    /// Get the torrent client (if configured)
    pub fn torrent_client(&self) -> Option<&Arc<dyn TorrentClient>> {
        self.torrent_client.as_ref()
    }

    /// Get the torrent catalog
    pub fn catalog(&self) -> &Arc<dyn TorrentCatalog> {
        &self.catalog
    }

    /// Get the pipeline processor (if initialized)
    pub fn pipeline(&self) -> Option<&Arc<AppPipelineProcessor>> {
        self.pipeline.as_ref()
    }

    /// Get the orchestrator (if enabled)
    pub fn orchestrator(&self) -> Option<&Arc<AppOrchestrator>> {
        self.orchestrator.as_ref()
    }

    /// Get the external catalog client (if configured)
    pub fn external_catalog(&self) -> Option<&Arc<dyn ExternalCatalog>> {
        self.external_catalog.as_ref()
    }

    /// Get the WebSocket broadcaster for real-time updates
    pub fn ws_broadcaster(&self) -> &WsBroadcaster {
        &self.ws_broadcaster
    }

    /// Get the detected encoder capabilities
    pub fn encoder_capabilities(&self) -> &EncoderCapabilities {
        &self.encoder_capabilities
    }
}
