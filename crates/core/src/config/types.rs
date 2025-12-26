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
    #[serde(default)]
    pub searcher: Option<SearcherConfig>,
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

/// Searcher configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearcherConfig {
    /// Search backend type
    pub backend: SearcherBackend,
    /// Jackett-specific configuration (required when backend = "jackett")
    #[serde(default)]
    pub jackett: Option<JackettConfig>,
}

/// Available search backends
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearcherBackend {
    Jackett,
    // Future: Prowlarr, DirectApi
}

/// Jackett search backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JackettConfig {
    /// Jackett server URL (e.g., "http://localhost:9117")
    pub url: String,
    /// Jackett API key
    pub api_key: String,
    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
}

fn default_timeout() -> u32 {
    30
}

/// Sanitized config for API responses (secrets redacted)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedConfig {
    pub auth: SanitizedAuthConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searcher: Option<SanitizedSearcherConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedAuthConfig {
    pub method: String,
}

/// Sanitized searcher config (API key redacted)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedSearcherConfig {
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jackett: Option<SanitizedJackettConfig>,
}

/// Sanitized Jackett config (API key hidden)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedJackettConfig {
    pub url: String,
    pub api_key_configured: bool,
    pub timeout_secs: u32,
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
            searcher: config.searcher.as_ref().map(|s| SanitizedSearcherConfig {
                backend: match s.backend {
                    SearcherBackend::Jackett => "jackett".to_string(),
                },
                jackett: s.jackett.as_ref().map(|j| SanitizedJackettConfig {
                    url: j.url.clone(),
                    api_key_configured: !j.api_key.is_empty(),
                    timeout_secs: j.timeout_secs,
                }),
            }),
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
            searcher: None,
        };
        let sanitized = SanitizedConfig::from(&config);
        assert_eq!(sanitized.auth.method, "none");
        assert_eq!(sanitized.server.port, 8080);
        assert_eq!(sanitized.database.path.to_str().unwrap(), "quentin.db");
        assert!(sanitized.searcher.is_none());
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

    #[test]
    fn test_deserialize_with_searcher_config() {
        let toml = r#"
[auth]
method = "none"

[searcher]
backend = "jackett"

[searcher.jackett]
url = "http://localhost:9117"
api_key = "test-api-key"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        let searcher = config.searcher.as_ref().unwrap();
        assert_eq!(searcher.backend, SearcherBackend::Jackett);

        let jackett = searcher.jackett.as_ref().unwrap();
        assert_eq!(jackett.url, "http://localhost:9117");
        assert_eq!(jackett.api_key, "test-api-key");
        assert_eq!(jackett.timeout_secs, 30); // default
    }

    #[test]
    fn test_sanitized_config_with_searcher() {
        let config = Config {
            auth: AuthConfig {
                method: AuthMethod::None,
            },
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            searcher: Some(SearcherConfig {
                backend: SearcherBackend::Jackett,
                jackett: Some(JackettConfig {
                    url: "http://localhost:9117".to_string(),
                    api_key: "secret-key".to_string(),
                    timeout_secs: 60,
                }),
            }),
        };

        let sanitized = SanitizedConfig::from(&config);
        let searcher = sanitized.searcher.as_ref().unwrap();
        assert_eq!(searcher.backend, "jackett");

        let jackett = searcher.jackett.as_ref().unwrap();
        assert_eq!(jackett.url, "http://localhost:9117");
        assert!(jackett.api_key_configured); // API key is hidden, just shows if configured
        assert_eq!(jackett.timeout_secs, 60);
    }
}
