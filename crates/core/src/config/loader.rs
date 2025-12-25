use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use std::path::Path;

use super::{types::Config, ConfigError};

/// Load configuration from file with environment variable overrides
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path.display().to_string()));
    }

    let config: Config = Figment::new()
        .merge(Toml::file(path))
        .merge(Env::prefixed("QUENTIN_").split("_"))
        .extract()
        .map_err(|e| ConfigError::ParseError(e.to_string()))?;

    Ok(config)
}

/// Load configuration from TOML string (useful for testing)
pub fn load_config_from_str(toml_str: &str) -> Result<Config, ConfigError> {
    toml::from_str(toml_str).map_err(|e| ConfigError::ParseError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_from_str_valid() {
        let toml = r#"
[auth]
method = "none"

[server]
port = 9000
"#;
        let config = load_config_from_str(toml).unwrap();
        assert_eq!(config.server.port, 9000);
    }

    #[test]
    fn test_load_config_from_str_missing_auth() {
        let toml = r#"
[server]
port = 8080
"#;
        let result = load_config_from_str(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::ParseError(_)));
    }

    #[test]
    fn test_load_config_file_not_found() {
        let result = load_config(Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::FileNotFound(_)));
    }

    #[test]
    fn test_load_config_from_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
[auth]
method = "none"

[server]
host = "127.0.0.1"
port = 3000
"#
        )
        .unwrap();

        let config = load_config(temp_file.path()).unwrap();
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.server.host.to_string(), "127.0.0.1");
    }
}
