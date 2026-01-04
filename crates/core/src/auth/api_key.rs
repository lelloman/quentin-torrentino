//! API Key authentication.

use async_trait::async_trait;

use super::{AuthError, AuthRequest, Authenticator, Identity};

/// Authenticator that validates requests against a configured API key.
///
/// Accepts the key in either:
/// - `Authorization: Bearer <key>` header
/// - `X-API-Key: <key>` header
pub struct ApiKeyAuthenticator {
    expected_key: String,
}

impl ApiKeyAuthenticator {
    pub fn new(api_key: String) -> Self {
        Self {
            expected_key: api_key,
        }
    }

    /// Extract API key from request headers.
    /// Checks Authorization: Bearer and X-API-Key headers.
    fn extract_key(&self, request: &AuthRequest) -> Option<String> {
        // Check Authorization: Bearer <key>
        if let Some(auth_header) = request.headers.get("authorization") {
            if let Some(key) = auth_header.strip_prefix("Bearer ") {
                return Some(key.to_string());
            }
            // Also support lowercase
            if let Some(key) = auth_header.strip_prefix("bearer ") {
                return Some(key.to_string());
            }
        }

        // Check X-API-Key header
        if let Some(key) = request.headers.get("x-api-key") {
            return Some(key.clone());
        }

        None
    }
}

#[async_trait]
impl Authenticator for ApiKeyAuthenticator {
    async fn authenticate(&self, request: &AuthRequest) -> Result<Identity, AuthError> {
        let provided_key = self
            .extract_key(request)
            .ok_or(AuthError::NotAuthenticated)?;

        // Constant-time comparison to prevent timing attacks
        if constant_time_eq(provided_key.as_bytes(), self.expected_key.as_bytes()) {
            Ok(Identity {
                user_id: "api_key_user".to_string(),
                method: "api_key".to_string(),
                claims: std::collections::HashMap::new(),
            })
        } else {
            Err(AuthError::InvalidCredentials("Invalid API key".to_string()))
        }
    }

    fn method_name(&self) -> &'static str {
        "api_key"
    }
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn make_request(headers: Vec<(&str, &str)>) -> AuthRequest {
        AuthRequest {
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v.to_string()))
                .collect(),
            source_ip: "127.0.0.1".parse::<IpAddr>().unwrap(),
        }
    }

    #[tokio::test]
    async fn test_bearer_token_valid() {
        let auth = ApiKeyAuthenticator::new("secret-key-123".to_string());
        let request = make_request(vec![("Authorization", "Bearer secret-key-123")]);

        let identity = auth.authenticate(&request).await.unwrap();

        assert_eq!(identity.user_id, "api_key_user");
        assert_eq!(identity.method, "api_key");
    }

    #[tokio::test]
    async fn test_x_api_key_header_valid() {
        let auth = ApiKeyAuthenticator::new("secret-key-123".to_string());
        let request = make_request(vec![("X-API-Key", "secret-key-123")]);

        let identity = auth.authenticate(&request).await.unwrap();

        assert_eq!(identity.user_id, "api_key_user");
        assert_eq!(identity.method, "api_key");
    }

    #[tokio::test]
    async fn test_invalid_key() {
        let auth = ApiKeyAuthenticator::new("secret-key-123".to_string());
        let request = make_request(vec![("Authorization", "Bearer wrong-key")]);

        let result = auth.authenticate(&request).await;

        assert!(matches!(result, Err(AuthError::InvalidCredentials(_))));
    }

    #[tokio::test]
    async fn test_missing_header() {
        let auth = ApiKeyAuthenticator::new("secret-key-123".to_string());
        let request = make_request(vec![]);

        let result = auth.authenticate(&request).await;

        assert!(matches!(result, Err(AuthError::NotAuthenticated)));
    }

    #[tokio::test]
    async fn test_bearer_lowercase() {
        let auth = ApiKeyAuthenticator::new("secret-key-123".to_string());
        let request = make_request(vec![("Authorization", "bearer secret-key-123")]);

        let identity = auth.authenticate(&request).await.unwrap();
        assert_eq!(identity.user_id, "api_key_user");
    }

    #[test]
    fn test_method_name() {
        let auth = ApiKeyAuthenticator::new("test".to_string());
        assert_eq!(auth.method_name(), "api_key");
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }
}
