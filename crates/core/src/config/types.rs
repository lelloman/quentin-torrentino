use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;

use crate::external_catalog::{MusicBrainzConfig, TmdbConfig};
use crate::orchestrator::OrchestratorConfig;
use crate::textbrain::TextBrainConfig;

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
    #[serde(default)]
    pub torrent_client: Option<TorrentClientConfig>,
    #[serde(default)]
    pub textbrain: TextBrainConfig,
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,
    #[serde(default)]
    pub external_catalogs: Option<ExternalCatalogsConfig>,
}

/// External catalog configuration (MusicBrainz, TMDB)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ExternalCatalogsConfig {
    /// MusicBrainz configuration (optional, no API key required)
    #[serde(default)]
    pub musicbrainz: Option<MusicBrainzConfig>,
    /// TMDB configuration (requires API key)
    #[serde(default)]
    pub tmdb: Option<TmdbConfig>,
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
    /// API key for api_key auth method.
    /// Can use ${ENV_VAR} syntax to read from environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// No authentication - all requests allowed
    None,
    /// API key authentication - requires Authorization header
    ApiKey,
    // Future: Oidc, ProxyHeaders, Cert
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

/// Torrent client configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TorrentClientConfig {
    /// Torrent client backend type
    pub backend: TorrentClientBackend,
    /// qBittorrent-specific configuration (required when backend = "qbittorrent")
    #[serde(default)]
    pub qbittorrent: Option<QBittorrentConfig>,
    /// librqbit-specific configuration (required when backend = "librqbit")
    #[serde(default)]
    pub librqbit: Option<LibrqbitConfig>,
}

/// Available torrent client backends
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentClientBackend {
    #[serde(rename = "qbittorrent")]
    QBittorrent,
    /// Embedded librqbit (no external service needed)
    Librqbit,
}

/// qBittorrent client configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QBittorrentConfig {
    /// qBittorrent Web UI URL (e.g., "http://localhost:8080")
    pub url: String,
    /// Username for Web UI authentication
    pub username: String,
    /// Password for Web UI authentication
    pub password: String,
    /// Default download path (optional)
    #[serde(default)]
    pub download_path: Option<String>,
    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
}

/// librqbit embedded client configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibrqbitConfig {
    /// Download directory path
    pub download_path: String,
    /// Enable DHT (Distributed Hash Table) for peer discovery
    #[serde(default = "default_true")]
    pub enable_dht: bool,
    /// TCP listen port (0 for random, None to disable)
    #[serde(default)]
    pub listen_port: Option<u16>,
    /// Persistence directory for session state (optional)
    #[serde(default)]
    pub persistence_path: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Sanitized config for API responses (secrets redacted)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedConfig {
    pub auth: SanitizedAuthConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searcher: Option<SanitizedSearcherConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torrent_client: Option<SanitizedTorrentClientConfig>,
    pub textbrain: SanitizedTextBrainConfig,
    pub orchestrator: SanitizedOrchestratorConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_catalogs: Option<SanitizedExternalCatalogsConfig>,
}

/// Sanitized external catalogs config (API keys hidden)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedExternalCatalogsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz: Option<SanitizedMusicBrainzConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<SanitizedTmdbConfig>,
}

/// Sanitized MusicBrainz config
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedMusicBrainzConfig {
    pub user_agent: String,
    pub rate_limit_ms: u64,
}

/// Sanitized TMDB config (API key hidden)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedTmdbConfig {
    pub api_key_configured: bool,
}

/// Sanitized Orchestrator config
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedOrchestratorConfig {
    pub enabled: bool,
    pub acquisition_poll_interval_ms: u64,
    pub download_poll_interval_ms: u64,
    pub auto_approve_threshold: f32,
    pub max_concurrent_downloads: usize,
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

/// Sanitized torrent client config (credentials hidden)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedTorrentClientConfig {
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qbittorrent: Option<SanitizedQBittorrentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub librqbit: Option<SanitizedLibrqbitConfig>,
}

/// Sanitized qBittorrent config (password hidden)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedQBittorrentConfig {
    pub url: String,
    pub username: String,
    pub credentials_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_path: Option<String>,
    pub timeout_secs: u32,
}

/// Sanitized librqbit config
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedLibrqbitConfig {
    pub download_path: String,
    pub enable_dht: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence_path: Option<String>,
}

/// Sanitized TextBrain config (API keys hidden)
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedTextBrainConfig {
    pub mode: String,
    pub auto_approve_threshold: f32,
    pub confidence_threshold: f32,
    pub max_queries: u32,
    pub llm_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,
}

impl From<&Config> for SanitizedConfig {
    fn from(config: &Config) -> Self {
        Self {
            auth: SanitizedAuthConfig {
                method: match config.auth.method {
                    AuthMethod::None => "none".to_string(),
                    AuthMethod::ApiKey => "api_key".to_string(),
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
            torrent_client: config
                .torrent_client
                .as_ref()
                .map(|tc| SanitizedTorrentClientConfig {
                    backend: match tc.backend {
                        TorrentClientBackend::QBittorrent => "qbittorrent".to_string(),
                        TorrentClientBackend::Librqbit => "librqbit".to_string(),
                    },
                    qbittorrent: tc
                        .qbittorrent
                        .as_ref()
                        .map(|qb| SanitizedQBittorrentConfig {
                            url: qb.url.clone(),
                            username: qb.username.clone(),
                            credentials_configured: !qb.password.is_empty(),
                            download_path: qb.download_path.clone(),
                            timeout_secs: qb.timeout_secs,
                        }),
                    librqbit: tc.librqbit.as_ref().map(|lb| SanitizedLibrqbitConfig {
                        download_path: lb.download_path.clone(),
                        enable_dht: lb.enable_dht,
                        listen_port: lb.listen_port,
                        persistence_path: lb.persistence_path.clone(),
                    }),
                }),
            textbrain: SanitizedTextBrainConfig {
                mode: format!("{:?}", config.textbrain.mode).to_lowercase(),
                auto_approve_threshold: config.textbrain.auto_approve_threshold,
                confidence_threshold: config.textbrain.confidence_threshold,
                max_queries: config.textbrain.max_queries,
                llm_configured: config.textbrain.llm.is_some(),
                llm_provider: config
                    .textbrain
                    .llm
                    .as_ref()
                    .map(|l| format!("{:?}", l.provider).to_lowercase()),
                llm_model: config.textbrain.llm.as_ref().map(|l| l.model.clone()),
            },
            orchestrator: SanitizedOrchestratorConfig {
                enabled: config.orchestrator.enabled,
                acquisition_poll_interval_ms: config.orchestrator.acquisition_poll_interval_ms,
                download_poll_interval_ms: config.orchestrator.download_poll_interval_ms,
                auto_approve_threshold: config.orchestrator.auto_approve_threshold,
                max_concurrent_downloads: config.orchestrator.max_concurrent_downloads,
            },
            external_catalogs: config.external_catalogs.as_ref().map(|ec| {
                SanitizedExternalCatalogsConfig {
                    musicbrainz: ec
                        .musicbrainz
                        .as_ref()
                        .map(|mb| SanitizedMusicBrainzConfig {
                            user_agent: mb.user_agent.clone(),
                            rate_limit_ms: mb.rate_limit_ms,
                        }),
                    tmdb: ec.tmdb.as_ref().map(|t| SanitizedTmdbConfig {
                        api_key_configured: !t.api_key.is_empty(),
                    }),
                }
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
                api_key: None,
            },
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            searcher: None,
            torrent_client: None,
            textbrain: TextBrainConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            external_catalogs: None,
        };
        let sanitized = SanitizedConfig::from(&config);
        assert_eq!(sanitized.auth.method, "none");
        assert_eq!(sanitized.server.port, 8080);
        assert_eq!(sanitized.database.path.to_str().unwrap(), "quentin.db");
        assert!(sanitized.searcher.is_none());
        assert!(sanitized.torrent_client.is_none());
        assert!(!sanitized.orchestrator.enabled);
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
                api_key: None,
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
            torrent_client: None,
            textbrain: TextBrainConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            external_catalogs: None,
        };

        let sanitized = SanitizedConfig::from(&config);
        let searcher = sanitized.searcher.as_ref().unwrap();
        assert_eq!(searcher.backend, "jackett");

        let jackett = searcher.jackett.as_ref().unwrap();
        assert_eq!(jackett.url, "http://localhost:9117");
        assert!(jackett.api_key_configured); // API key is hidden, just shows if configured
        assert_eq!(jackett.timeout_secs, 60);
    }

    #[test]
    fn test_deserialize_with_torrent_client_config() {
        let toml = r#"
[auth]
method = "none"

[torrent_client]
backend = "qbittorrent"

[torrent_client.qbittorrent]
url = "http://localhost:8080"
username = "admin"
password = "adminadmin"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        let tc = config.torrent_client.as_ref().unwrap();
        assert_eq!(tc.backend, TorrentClientBackend::QBittorrent);

        let qbit = tc.qbittorrent.as_ref().unwrap();
        assert_eq!(qbit.url, "http://localhost:8080");
        assert_eq!(qbit.username, "admin");
        assert_eq!(qbit.password, "adminadmin");
        assert_eq!(qbit.timeout_secs, 30); // default
        assert!(qbit.download_path.is_none());
    }

    #[test]
    fn test_deserialize_torrent_client_with_optional_fields() {
        let toml = r#"
[auth]
method = "none"

[torrent_client]
backend = "qbittorrent"

[torrent_client.qbittorrent]
url = "http://localhost:8080"
username = "admin"
password = "secret"
download_path = "/downloads"
timeout_secs = 60
"#;
        let config: Config = toml::from_str(toml).unwrap();
        let qbit = config
            .torrent_client
            .as_ref()
            .unwrap()
            .qbittorrent
            .as_ref()
            .unwrap();
        assert_eq!(qbit.download_path, Some("/downloads".to_string()));
        assert_eq!(qbit.timeout_secs, 60);
    }

    #[test]
    fn test_sanitized_config_with_torrent_client() {
        let config = Config {
            auth: AuthConfig {
                method: AuthMethod::None,
                api_key: None,
            },
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            searcher: None,
            torrent_client: Some(TorrentClientConfig {
                backend: TorrentClientBackend::QBittorrent,
                qbittorrent: Some(QBittorrentConfig {
                    url: "http://localhost:8080".to_string(),
                    username: "admin".to_string(),
                    password: "secret-password".to_string(),
                    download_path: Some("/downloads".to_string()),
                    timeout_secs: 45,
                }),
                librqbit: None,
            }),
            textbrain: TextBrainConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            external_catalogs: None,
        };

        let sanitized = SanitizedConfig::from(&config);
        let tc = sanitized.torrent_client.as_ref().unwrap();
        assert_eq!(tc.backend, "qbittorrent");

        let qbit = tc.qbittorrent.as_ref().unwrap();
        assert_eq!(qbit.url, "http://localhost:8080");
        assert_eq!(qbit.username, "admin");
        assert!(qbit.credentials_configured); // Password is hidden
        assert_eq!(qbit.download_path, Some("/downloads".to_string()));
        assert_eq!(qbit.timeout_secs, 45);
    }
}
