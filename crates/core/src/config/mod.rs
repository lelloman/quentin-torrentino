mod loader;
mod types;
mod validate;

pub use loader::*;
pub use types::*;
pub use validate::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}
