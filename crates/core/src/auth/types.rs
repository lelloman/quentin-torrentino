use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Request information for authentication
#[derive(Debug, Clone)]
pub struct AuthRequest {
    pub headers: HashMap<String, String>,
    pub source_ip: IpAddr,
}

/// Authenticated identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub method: String,
    pub claims: HashMap<String, serde_json::Value>,
}

impl Identity {
    pub fn anonymous() -> Self {
        Self {
            user_id: "anonymous".to_string(),
            method: "none".to_string(),
            claims: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anonymous_identity() {
        let identity = Identity::anonymous();
        assert_eq!(identity.user_id, "anonymous");
        assert_eq!(identity.method, "none");
        assert!(identity.claims.is_empty());
    }

    #[test]
    fn test_identity_serialization() {
        let identity = Identity {
            user_id: "user123".to_string(),
            method: "oidc".to_string(),
            claims: {
                let mut map = HashMap::new();
                map.insert("email".to_string(), serde_json::json!("user@example.com"));
                map
            },
        };

        let json = serde_json::to_string(&identity).unwrap();
        let deserialized: Identity = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.user_id, "user123");
        assert_eq!(deserialized.method, "oidc");
        assert_eq!(
            deserialized.claims.get("email"),
            Some(&serde_json::json!("user@example.com"))
        );
    }
}
