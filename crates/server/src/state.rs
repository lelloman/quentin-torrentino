use std::sync::Arc;
use torrentino_core::{Authenticator, Config, SanitizedConfig};

/// Shared application state
pub struct AppState {
    config: Config,
    authenticator: Arc<dyn Authenticator>,
}

impl AppState {
    pub fn new(config: Config, authenticator: Arc<dyn Authenticator>) -> Self {
        Self {
            config,
            authenticator,
        }
    }

    pub fn sanitized_config(&self) -> SanitizedConfig {
        SanitizedConfig::from(&self.config)
    }

    #[allow(dead_code)]
    pub fn authenticator(&self) -> &dyn Authenticator {
        self.authenticator.as_ref()
    }
}
