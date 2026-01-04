//! Configuration for the converter module.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the FFmpeg-based converter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConverterConfig {
    /// Path to ffmpeg binary.
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: PathBuf,

    /// Path to ffprobe binary.
    #[serde(default = "default_ffprobe_path")]
    pub ffprobe_path: PathBuf,

    /// Temporary directory for intermediate files.
    #[serde(default = "default_temp_dir")]
    pub temp_dir: PathBuf,

    /// Maximum parallel conversions.
    #[serde(default = "default_max_parallel")]
    pub max_parallel_conversions: usize,

    /// Timeout for a single conversion job in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Whether to preserve original files after conversion.
    #[serde(default)]
    pub preserve_originals: bool,

    /// FFmpeg log level (quiet, panic, fatal, error, warning, info, verbose, debug, trace).
    #[serde(default = "default_log_level")]
    pub ffmpeg_log_level: String,

    /// Additional global ffmpeg arguments.
    #[serde(default)]
    pub extra_ffmpeg_args: Vec<String>,
}

fn default_ffmpeg_path() -> PathBuf {
    PathBuf::from("ffmpeg")
}

fn default_ffprobe_path() -> PathBuf {
    PathBuf::from("ffprobe")
}

fn default_temp_dir() -> PathBuf {
    std::env::temp_dir().join("quentin-converter")
}

fn default_max_parallel() -> usize {
    4
}

fn default_timeout() -> u64 {
    3600 // 1 hour
}

fn default_log_level() -> String {
    "warning".to_string()
}

impl Default for ConverterConfig {
    fn default() -> Self {
        Self {
            ffmpeg_path: default_ffmpeg_path(),
            ffprobe_path: default_ffprobe_path(),
            temp_dir: default_temp_dir(),
            max_parallel_conversions: default_max_parallel(),
            timeout_secs: default_timeout(),
            preserve_originals: false,
            ffmpeg_log_level: default_log_level(),
            extra_ffmpeg_args: Vec::new(),
        }
    }
}

impl ConverterConfig {
    /// Creates a new config with custom ffmpeg/ffprobe paths.
    pub fn with_paths(ffmpeg_path: PathBuf, ffprobe_path: PathBuf) -> Self {
        Self {
            ffmpeg_path,
            ffprobe_path,
            ..Default::default()
        }
    }

    /// Sets the temp directory.
    pub fn with_temp_dir(mut self, temp_dir: PathBuf) -> Self {
        self.temp_dir = temp_dir;
        self
    }

    /// Sets the maximum parallel conversions.
    pub fn with_max_parallel(mut self, max: usize) -> Self {
        self.max_parallel_conversions = max;
        self
    }

    /// Sets the timeout in seconds.
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConverterConfig::default();
        assert_eq!(config.ffmpeg_path, PathBuf::from("ffmpeg"));
        assert_eq!(config.ffprobe_path, PathBuf::from("ffprobe"));
        assert_eq!(config.max_parallel_conversions, 4);
        assert_eq!(config.timeout_secs, 3600);
    }

    #[test]
    fn test_config_builder() {
        let config = ConverterConfig::with_paths(
            PathBuf::from("/usr/local/bin/ffmpeg"),
            PathBuf::from("/usr/local/bin/ffprobe"),
        )
        .with_temp_dir(PathBuf::from("/tmp/test"))
        .with_max_parallel(8)
        .with_timeout(7200);

        assert_eq!(config.ffmpeg_path, PathBuf::from("/usr/local/bin/ffmpeg"));
        assert_eq!(config.temp_dir, PathBuf::from("/tmp/test"));
        assert_eq!(config.max_parallel_conversions, 8);
        assert_eq!(config.timeout_secs, 7200);
    }

    #[test]
    fn test_config_serialization() {
        let config = ConverterConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ConverterConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.max_parallel_conversions,
            config.max_parallel_conversions
        );
    }
}
