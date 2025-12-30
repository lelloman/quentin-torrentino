//! Orchestrator configuration.

use serde::{Deserialize, Serialize};

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
}
