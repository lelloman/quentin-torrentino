mod api_key;
mod none;
mod traits;
mod types;

pub use api_key::*;
pub use none::*;
pub use traits::*;
pub use types::*;

use crate::config::AuthConfig;

/// Factory function to create authenticator from config
pub fn create_authenticator(config: &AuthConfig) -> Result<Box<dyn Authenticator>, AuthError> {
    use crate::config::AuthMethod;

    match config.method {
        AuthMethod::None => Ok(Box::new(NoneAuthenticator::new())),
        AuthMethod::ApiKey => {
            let api_key = config.api_key.clone().ok_or_else(|| {
                AuthError::ConfigurationError(
                    "api_key must be set when using ApiKey auth method".to_string(),
                )
            })?;
            Ok(Box::new(ApiKeyAuthenticator::new(api_key)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthMethod;

    #[test]
    fn test_create_authenticator_none() {
        let config = AuthConfig {
            method: AuthMethod::None,
            api_key: None,
        };
        let auth = create_authenticator(&config).unwrap();
        assert_eq!(auth.method_name(), "none");
    }

    #[test]
    fn test_create_authenticator_api_key() {
        let config = AuthConfig {
            method: AuthMethod::ApiKey,
            api_key: Some("secret-key".to_string()),
        };
        let auth = create_authenticator(&config).unwrap();
        assert_eq!(auth.method_name(), "api_key");
    }

    #[test]
    fn test_create_authenticator_api_key_missing_key() {
        let config = AuthConfig {
            method: AuthMethod::ApiKey,
            api_key: None,
        };
        let result = create_authenticator(&config);
        assert!(matches!(result, Err(AuthError::ConfigurationError(_))));
    }
}
