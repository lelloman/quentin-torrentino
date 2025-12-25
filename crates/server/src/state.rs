use std::sync::Arc;
use torrentino_core::{AuditHandle, AuditStore, Authenticator, Config, SanitizedConfig};

/// Shared application state
pub struct AppState {
    config: Config,
    authenticator: Arc<dyn Authenticator>,
    #[allow(dead_code)] // Will be used by handlers that emit audit events
    audit_handle: AuditHandle,
    audit_store: Arc<dyn AuditStore>,
}

impl AppState {
    pub fn new(
        config: Config,
        authenticator: Arc<dyn Authenticator>,
        audit_handle: AuditHandle,
        audit_store: Arc<dyn AuditStore>,
    ) -> Self {
        Self {
            config,
            authenticator,
            audit_handle,
            audit_store,
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
    #[allow(dead_code)] // Will be used by handlers that emit audit events
    pub fn audit(&self) -> &AuditHandle {
        &self.audit_handle
    }

    /// Get the audit store for querying events
    pub fn audit_store(&self) -> &Arc<dyn AuditStore> {
        &self.audit_store
    }
}
