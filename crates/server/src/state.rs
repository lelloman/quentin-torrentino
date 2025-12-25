use std::sync::Arc;
use torrentino_core::{AuditHandle, AuditStore, Authenticator, Config, SanitizedConfig, TicketStore};

/// Shared application state
pub struct AppState {
    config: Config,
    authenticator: Arc<dyn Authenticator>,
    audit_handle: AuditHandle,
    audit_store: Arc<dyn AuditStore>,
    ticket_store: Arc<dyn TicketStore>,
}

impl AppState {
    pub fn new(
        config: Config,
        authenticator: Arc<dyn Authenticator>,
        audit_handle: AuditHandle,
        audit_store: Arc<dyn AuditStore>,
        ticket_store: Arc<dyn TicketStore>,
    ) -> Self {
        Self {
            config,
            authenticator,
            audit_handle,
            audit_store,
            ticket_store,
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
}
