//! Configuration for the processor module.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the processing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    /// Maximum parallel conversions.
    #[serde(default = "default_max_conversions")]
    pub max_parallel_conversions: usize,

    /// Maximum parallel placements.
    #[serde(default = "default_max_placements")]
    pub max_parallel_placements: usize,

    /// Temporary directory for intermediate files.
    #[serde(default = "default_temp_dir")]
    pub temp_dir: PathBuf,

    /// Whether to clean up source files after successful placement.
    #[serde(default)]
    pub cleanup_after_placement: bool,

    /// Retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Conversion timeout in seconds.
    #[serde(default = "default_conversion_timeout")]
    pub conversion_timeout_secs: u64,

    /// Progress update interval in milliseconds.
    #[serde(default = "default_progress_interval")]
    pub progress_interval_ms: u64,
}

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_attempts: u32,

    /// Initial delay between retries in seconds.
    #[serde(default = "default_retry_delay")]
    pub initial_delay_secs: u64,

    /// Maximum delay between retries in seconds.
    #[serde(default = "default_max_delay")]
    pub max_delay_secs: u64,

    /// Exponential backoff multiplier.
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
}

fn default_max_conversions() -> usize {
    4
}

fn default_max_placements() -> usize {
    8
}

fn default_temp_dir() -> PathBuf {
    std::env::temp_dir().join("quentin-processor")
}

fn default_conversion_timeout() -> u64 {
    3600 // 1 hour
}

fn default_progress_interval() -> u64 {
    1000 // 1 second
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    60 // 1 minute
}

fn default_max_delay() -> u64 {
    3600 // 1 hour
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_retries(),
            initial_delay_secs: default_retry_delay(),
            max_delay_secs: default_max_delay(),
            backoff_multiplier: default_backoff_multiplier(),
        }
    }
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            max_parallel_conversions: default_max_conversions(),
            max_parallel_placements: default_max_placements(),
            temp_dir: default_temp_dir(),
            cleanup_after_placement: false,
            retry: RetryConfig::default(),
            conversion_timeout_secs: default_conversion_timeout(),
            progress_interval_ms: default_progress_interval(),
        }
    }
}

impl ProcessorConfig {
    /// Sets the maximum parallel conversions.
    pub fn with_max_conversions(mut self, max: usize) -> Self {
        self.max_parallel_conversions = max;
        self
    }

    /// Sets the maximum parallel placements.
    pub fn with_max_placements(mut self, max: usize) -> Self {
        self.max_parallel_placements = max;
        self
    }

    /// Sets the temp directory.
    pub fn with_temp_dir(mut self, dir: PathBuf) -> Self {
        self.temp_dir = dir;
        self
    }

    /// Enables cleanup after placement.
    pub fn with_cleanup(mut self, enabled: bool) -> Self {
        self.cleanup_after_placement = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ProcessorConfig::default();
        assert_eq!(config.max_parallel_conversions, 4);
        assert_eq!(config.max_parallel_placements, 8);
        assert!(!config.cleanup_after_placement);
    }

    #[test]
    fn test_retry_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay_secs, 60);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_config_builder() {
        let config = ProcessorConfig::default()
            .with_max_conversions(8)
            .with_max_placements(16)
            .with_cleanup(true);

        assert_eq!(config.max_parallel_conversions, 8);
        assert_eq!(config.max_parallel_placements, 16);
        assert!(config.cleanup_after_placement);
    }
}
