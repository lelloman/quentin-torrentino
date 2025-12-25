use async_trait::async_trait;

use super::{AuthError, AuthRequest, Authenticator, Identity};

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

    #[test]
    fn test_none_authenticator_method_name() {
        let auth = NoneAuthenticator::new();
        assert_eq!(auth.method_name(), "none");
    }

    #[test]
    fn test_none_authenticator_default() {
        let auth = NoneAuthenticator::default();
        assert_eq!(auth.method_name(), "none");
    }
}
