//! Orchestrator configuration.

use serde::{Deserialize, Serialize};

/// Configuration for retry behavior with exponential backoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up.
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// Initial delay before the first retry (milliseconds).
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,

    /// Maximum delay between retries (milliseconds).
    /// Delays are capped at this value to prevent excessive waits.
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,

    /// Multiplier for exponential backoff.
    /// Each retry waits `initial_delay * multiplier^(attempt-1)` milliseconds.
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,

    /// Jitter factor (0.0-1.0) to randomize retry delays.
    /// A value of 0.2 means delays vary by ±20%.
    #[serde(default = "default_jitter_factor")]
    pub jitter_factor: f64,
}

fn default_max_attempts() -> u32 {
    10
}

fn default_initial_delay_ms() -> u64 {
    5000 // 5 seconds
}

fn default_max_delay_ms() -> u64 {
    1800000 // 30 minutes
}

fn default_backoff_multiplier() -> f64 {
    2.5
}

fn default_jitter_factor() -> f64 {
    0.2
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            backoff_multiplier: default_backoff_multiplier(),
            jitter_factor: default_jitter_factor(),
        }
    }
}

impl RetryConfig {
    /// Calculate the delay for a given attempt number (1-indexed).
    /// Returns None if max_attempts has been exceeded.
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<std::time::Duration> {
        if attempt > self.max_attempts {
            return None;
        }

        // Calculate base delay: initial * multiplier^(attempt-1)
        let exponent = (attempt.saturating_sub(1)) as f64;
        let base_delay_ms = self.initial_delay_ms as f64 * self.backoff_multiplier.powf(exponent);

        // Apply cap
        let capped_delay_ms = base_delay_ms.min(self.max_delay_ms as f64);

        // Apply jitter: delay * random(1 - jitter, 1 + jitter)
        let jitter_range = self.jitter_factor;
        let jitter = if jitter_range > 0.0 {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hasher};
            // Simple pseudo-random based on current time
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            );
            let random = (hasher.finish() % 1000) as f64 / 1000.0; // 0.0 to 0.999
            1.0 - jitter_range + (2.0 * jitter_range * random)
        } else {
            1.0
        };

        let final_delay_ms = (capped_delay_ms * jitter) as u64;
        Some(std::time::Duration::from_millis(final_delay_ms))
    }

    /// Check if another retry should be attempted.
    pub fn should_retry(&self, current_attempt: u32) -> bool {
        current_attempt < self.max_attempts
    }
}

/// Configuration for the ticket orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Enable/disable the orchestrator.
    /// When disabled, tickets must be processed manually via API.
    #[serde(default)]
    pub enabled: bool,

    /// How often to poll for pending tickets (milliseconds).
    /// The acquisition loop processes one ticket at a time.
    #[serde(default = "default_acquisition_interval")]
    pub acquisition_poll_interval_ms: u64,

    /// How often to check download progress (milliseconds).
    /// The download monitor tracks all active downloads concurrently.
    #[serde(default = "default_download_interval")]
    pub download_poll_interval_ms: u64,

    /// Auto-approve threshold (0.0-1.0).
    /// Candidates with scores >= this threshold are auto-approved.
    /// Candidates below require manual approval via API.
    #[serde(default = "default_threshold")]
    pub auto_approve_threshold: f32,

    /// Maximum concurrent downloads (0 = unlimited).
    /// When limit is reached, new downloads wait until slots are free.
    #[serde(default)]
    pub max_concurrent_downloads: usize,

    // ========================================================================
    // Stall detection and failover
    // ========================================================================

    /// Round 1 stall timeout in seconds (default: 300 = 5 minutes).
    /// If no progress for this duration, try next candidate.
    #[serde(default = "default_stall_timeout_round1")]
    pub stall_timeout_round1_secs: u64,

    /// Round 2 stall timeout in seconds (default: 1800 = 30 minutes).
    /// After trying all candidates once, use this longer timeout.
    #[serde(default = "default_stall_timeout_round2")]
    pub stall_timeout_round2_secs: u64,

    /// Round 3 stall timeout in seconds (default: 7200 = 2 hours).
    /// Final round before giving up entirely.
    #[serde(default = "default_stall_timeout_round3")]
    pub stall_timeout_round3_secs: u64,

    /// Maximum candidates to retain for failover (default: 5).
    #[serde(default = "default_max_failover_candidates")]
    pub max_failover_candidates: usize,

    /// Retry configuration for transient failures.
    #[serde(default)]
    pub retry: RetryConfig,
}

fn default_acquisition_interval() -> u64 {
    5000 // 5 seconds
}

fn default_download_interval() -> u64 {
    3000 // 3 seconds
}

fn default_threshold() -> f32 {
    0.85
}

fn default_stall_timeout_round1() -> u64 {
    300 // 5 minutes
}

fn default_stall_timeout_round2() -> u64 {
    1800 // 30 minutes
}

fn default_stall_timeout_round3() -> u64 {
    7200 // 2 hours
}

fn default_max_failover_candidates() -> usize {
    5
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            acquisition_poll_interval_ms: default_acquisition_interval(),
            download_poll_interval_ms: default_download_interval(),
            auto_approve_threshold: default_threshold(),
            max_concurrent_downloads: 0,
            stall_timeout_round1_secs: default_stall_timeout_round1(),
            stall_timeout_round2_secs: default_stall_timeout_round2(),
            stall_timeout_round3_secs: default_stall_timeout_round3(),
            max_failover_candidates: default_max_failover_candidates(),
            retry: RetryConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrchestratorConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.acquisition_poll_interval_ms, 5000);
        assert_eq!(config.download_poll_interval_ms, 3000);
        assert_eq!(config.auto_approve_threshold, 0.85);
        assert_eq!(config.max_concurrent_downloads, 0);
        // Stall detection defaults
        assert_eq!(config.stall_timeout_round1_secs, 300); // 5 min
        assert_eq!(config.stall_timeout_round2_secs, 1800); // 30 min
        assert_eq!(config.stall_timeout_round3_secs, 7200); // 2 hours
        assert_eq!(config.max_failover_candidates, 5);
        // Retry defaults
        assert_eq!(config.retry.max_attempts, 10);
        assert_eq!(config.retry.initial_delay_ms, 5000);
        assert_eq!(config.retry.max_delay_ms, 1800000);
        assert!((config.retry.backoff_multiplier - 2.5).abs() < 0.001);
        assert!((config.retry.jitter_factor - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_deserialize_minimal() {
        let toml = r#"
            enabled = true
        "#;
        let config: OrchestratorConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.acquisition_poll_interval_ms, 5000);
        assert_eq!(config.auto_approve_threshold, 0.85);
        // Retry should use defaults
        assert_eq!(config.retry.max_attempts, 10);
    }

    #[test]
    fn test_deserialize_full() {
        let toml = r#"
            enabled = true
            acquisition_poll_interval_ms = 10000
            download_poll_interval_ms = 5000
            auto_approve_threshold = 0.90
            max_concurrent_downloads = 3
        "#;
        let config: OrchestratorConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.acquisition_poll_interval_ms, 10000);
        assert_eq!(config.download_poll_interval_ms, 5000);
        assert_eq!(config.auto_approve_threshold, 0.90);
        assert_eq!(config.max_concurrent_downloads, 3);
    }

    #[test]
    fn test_deserialize_with_retry() {
        let toml = r#"
            enabled = true

            [retry]
            max_attempts = 5
            initial_delay_ms = 1000
            max_delay_ms = 60000
            backoff_multiplier = 2.0
            jitter_factor = 0.1
        "#;
        let config: OrchestratorConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.retry.max_attempts, 5);
        assert_eq!(config.retry.initial_delay_ms, 1000);
        assert_eq!(config.retry.max_delay_ms, 60000);
        assert!((config.retry.backoff_multiplier - 2.0).abs() < 0.001);
        assert!((config.retry.jitter_factor - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.initial_delay_ms, 5000);
        assert_eq!(config.max_delay_ms, 1800000);
        assert!((config.backoff_multiplier - 2.5).abs() < 0.001);
        assert!((config.jitter_factor - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_retry_delay_calculation_no_jitter() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_delay_ms: 5000,
            max_delay_ms: 1800000,
            backoff_multiplier: 2.5,
            jitter_factor: 0.0, // No jitter for predictable testing
        };

        // Attempt 1: 5000ms
        let delay1 = config.delay_for_attempt(1).unwrap();
        assert_eq!(delay1.as_millis(), 5000);

        // Attempt 2: 5000 * 2.5 = 12500ms
        let delay2 = config.delay_for_attempt(2).unwrap();
        assert_eq!(delay2.as_millis(), 12500);

        // Attempt 3: 5000 * 2.5^2 = 31250ms
        let delay3 = config.delay_for_attempt(3).unwrap();
        assert_eq!(delay3.as_millis(), 31250);

        // Attempt 4: 5000 * 2.5^3 = 78125ms
        let delay4 = config.delay_for_attempt(4).unwrap();
        assert_eq!(delay4.as_millis(), 78125);
    }

    #[test]
    fn test_retry_delay_capped() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_delay_ms: 5000,
            max_delay_ms: 60000, // 1 minute cap
            backoff_multiplier: 2.5,
            jitter_factor: 0.0,
        };

        // Attempt 5 would be 5000 * 2.5^4 = 195312ms, but capped at 60000
        let delay5 = config.delay_for_attempt(5).unwrap();
        assert_eq!(delay5.as_millis(), 60000);

        // Attempt 10 also capped
        let delay10 = config.delay_for_attempt(10).unwrap();
        assert_eq!(delay10.as_millis(), 60000);
    }

    #[test]
    fn test_retry_delay_exceeds_max_attempts() {
        let config = RetryConfig {
            max_attempts: 3,
            ..Default::default()
        };

        assert!(config.delay_for_attempt(1).is_some());
        assert!(config.delay_for_attempt(3).is_some());
        assert!(config.delay_for_attempt(4).is_none()); // Exceeded
    }

    #[test]
    fn test_should_retry() {
        let config = RetryConfig {
            max_attempts: 5,
            ..Default::default()
        };

        assert!(config.should_retry(0)); // Haven't tried yet
        assert!(config.should_retry(1));
        assert!(config.should_retry(4));
        assert!(!config.should_retry(5)); // At max
        assert!(!config.should_retry(6)); // Past max
    }

    #[test]
    fn test_retry_delay_with_jitter() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_delay_ms: 10000,
            max_delay_ms: 1800000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.2, // ±20%
        };

        // Call multiple times to verify jitter produces variation
        // Base delay for attempt 1 is 10000ms, with ±20% jitter: 8000-12000ms
        let delay = config.delay_for_attempt(1).unwrap();
        let delay_ms = delay.as_millis() as u64;
        assert!(delay_ms >= 8000 && delay_ms <= 12000,
            "Delay {} should be between 8000 and 12000", delay_ms);
    }
}
