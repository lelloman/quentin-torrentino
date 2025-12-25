use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;

/// Root configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

fn default_host() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

fn default_port() -> u16 {
    8080
}

/// Authentication configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    pub method: AuthMethod,
}

/// Database configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

fn default_db_path() -> PathBuf {
    PathBuf::from("quentin.db")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    None,
    // Future: Oidc, Address, Cert, Plugin
}

/// Sanitized config for API responses (secrets redacted)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedConfig {
    pub auth: SanitizedAuthConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedAuthConfig {
    pub method: String,
}

impl From<&Config> for SanitizedConfig {
    fn from(config: &Config) -> Self {
        Self {
            auth: SanitizedAuthConfig {
                method: match config.auth.method {
                    AuthMethod::None => "none".to_string(),
                },
            },
            server: config.server.clone(),
            database: config.database.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_valid_config_with_none_auth() {
        let toml = r#"
[auth]
method = "none"

[server]
host = "127.0.0.1"
port = 9000
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(matches!(config.auth.method, AuthMethod::None));
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.host.to_string(), "127.0.0.1");
    }

    #[test]
    fn test_deserialize_with_default_server() {
        let toml = r#"
[auth]
method = "none"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(matches!(config.auth.method, AuthMethod::None));
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.host.to_string(), "0.0.0.0");
    }

    #[test]
    fn test_deserialize_missing_auth_fails() {
        let toml = r#"
[server]
port = 8080
"#;
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitized_config() {
        let config = Config {
            auth: AuthConfig {
                method: AuthMethod::None,
            },
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
        };
        let sanitized = SanitizedConfig::from(&config);
        assert_eq!(sanitized.auth.method, "none");
        assert_eq!(sanitized.server.port, 8080);
        assert_eq!(sanitized.database.path.to_str().unwrap(), "quentin.db");
    }

    #[test]
    fn test_deserialize_with_default_database() {
        let toml = r#"
[auth]
method = "none"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.database.path.to_str().unwrap(), "quentin.db");
    }

    #[test]
    fn test_deserialize_with_custom_database_path() {
        let toml = r#"
[auth]
method = "none"

[database]
path = "/data/my-db.sqlite"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.database.path.to_str().unwrap(), "/data/my-db.sqlite");
    }
}
