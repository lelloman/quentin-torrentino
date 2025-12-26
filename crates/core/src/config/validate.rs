use super::{types::Config, ConfigError};

/// Validate configuration
/// Currently validates:
/// - Auth section exists (enforced by serde)
/// - Server port is not 0
pub fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Server validation
    if config.server.port == 0 {
        return Err(ConfigError::ValidationError(
            "server.port cannot be 0".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, AuthMethod, DatabaseConfig, ServerConfig};
    use std::net::IpAddr;

    #[test]
    fn test_validate_valid_config() {
        let config = Config {
            auth: AuthConfig {
                method: AuthMethod::None,
            },
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            searcher: None,
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_port_zero_fails() {
        let config = Config {
            auth: AuthConfig {
                method: AuthMethod::None,
            },
            server: ServerConfig {
                host: "0.0.0.0".parse::<IpAddr>().unwrap(),
                port: 0,
            },
            database: DatabaseConfig::default(),
            searcher: None,
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::ValidationError(_)));
    }
}
