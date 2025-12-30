use async_trait::async_trait;
use thiserror::Error;

use super::types::{AuthRequest, Identity};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Authentication required")]
    NotAuthenticated,

    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),

    #[error("Authentication service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate a request and return the identity
    async fn authenticate(&self, request: &AuthRequest) -> Result<Identity, AuthError>;

    /// Name of this authentication method
    fn method_name(&self) -> &'static str;
}
