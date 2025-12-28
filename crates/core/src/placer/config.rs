//! Configuration for the placer module.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the file system placer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacerConfig {
    /// Buffer size for file copies in bytes.
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Whether to use atomic moves when possible.
    #[serde(default = "default_true")]
    pub prefer_atomic_moves: bool,

    /// Whether to verify checksums after copying.
    #[serde(default)]
    pub verify_checksums: bool,

    /// Whether to clean up source files after placement.
    #[serde(default)]
    pub cleanup_sources: bool,

    /// Whether to create directories with intermediate paths.
    #[serde(default = "default_true")]
    pub create_parents: bool,

    /// Maximum parallel file operations.
    #[serde(default = "default_max_parallel")]
    pub max_parallel_operations: usize,

    /// Permissions for created directories (Unix only, octal).
    #[serde(default = "default_dir_mode")]
    pub directory_mode: u32,

    /// Backup directory for replaced files (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<PathBuf>,
}

fn default_buffer_size() -> usize {
    8 * 1024 * 1024 // 8 MB
}

fn default_true() -> bool {
    true
}

fn default_max_parallel() -> usize {
    4
}

fn default_dir_mode() -> u32 {
    0o755
}

impl Default for PlacerConfig {
    fn default() -> Self {
        Self {
            buffer_size: default_buffer_size(),
            prefer_atomic_moves: true,
            verify_checksums: false,
            cleanup_sources: false,
            create_parents: true,
            max_parallel_operations: default_max_parallel(),
            directory_mode: default_dir_mode(),
            backup_dir: None,
        }
    }
}

impl PlacerConfig {
    /// Creates a new config with atomic moves enabled.
    pub fn with_atomic_moves(mut self, enabled: bool) -> Self {
        self.prefer_atomic_moves = enabled;
        self
    }

    /// Enables checksum verification.
    pub fn with_checksum_verification(mut self, enabled: bool) -> Self {
        self.verify_checksums = enabled;
        self
    }

    /// Enables source cleanup.
    pub fn with_cleanup(mut self, enabled: bool) -> Self {
        self.cleanup_sources = enabled;
        self
    }

    /// Sets the buffer size for copies.
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Sets the backup directory.
    pub fn with_backup_dir(mut self, path: PathBuf) -> Self {
        self.backup_dir = Some(path);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PlacerConfig::default();
        assert_eq!(config.buffer_size, 8 * 1024 * 1024);
        assert!(config.prefer_atomic_moves);
        assert!(!config.verify_checksums);
        assert!(config.create_parents);
    }

    #[test]
    fn test_config_builder() {
        let config = PlacerConfig::default()
            .with_atomic_moves(false)
            .with_checksum_verification(true)
            .with_cleanup(true)
            .with_buffer_size(1024 * 1024);

        assert!(!config.prefer_atomic_moves);
        assert!(config.verify_checksums);
        assert!(config.cleanup_sources);
        assert_eq!(config.buffer_size, 1024 * 1024);
    }
}
