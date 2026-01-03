//! Mock converter for testing.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use crate::converter::{
    ConversionConstraints, ConversionJob, ConversionProgress, ConversionResult, Converter,
    ConverterError, MediaInfo,
};

/// A recorded conversion job for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedConversion {
    /// The job that was submitted.
    pub job: ConversionJob,
    /// Whether the conversion succeeded.
    pub success: bool,
}

/// Mock implementation of the Converter trait.
///
/// Provides controllable behavior for testing:
/// - Track conversion jobs for assertions
/// - Simulate success/failure
/// - Control probe results
/// - Simulate progress updates
///
/// # Example
///
/// ```rust,ignore
/// use torrentino_core::testing::MockConverter;
///
/// let converter = MockConverter::new();
///
/// // Configure probe results
/// converter.set_probe_result("/path/to/file.mkv", MediaInfo {
///     path: PathBuf::from("/path/to/file.mkv"),
///     duration_secs: 7200.0, // 2 hours
///     // ...
/// }).await;
///
/// // Convert
/// let result = converter.convert(job).await?;
///
/// // Check what was converted
/// let conversions = converter.recorded_conversions().await;
/// assert_eq!(conversions.len(), 1);
/// ```
#[derive(Debug)]
pub struct MockConverter {
    /// Recorded conversions.
    conversions: Arc<RwLock<Vec<RecordedConversion>>>,
    /// Pre-configured probe results by path.
    probe_results: Arc<RwLock<HashMap<PathBuf, MediaInfo>>>,
    /// If set, the next operation will fail with this error.
    next_error: Arc<RwLock<Option<ConverterError>>>,
    /// Simulated conversion duration in milliseconds.
    conversion_duration_ms: Arc<RwLock<u64>>,
    /// Whether to send progress updates during conversion.
    send_progress: Arc<RwLock<bool>>,
    /// Default media info for probing unknown files.
    default_media_info: Arc<RwLock<Option<MediaInfo>>>,
}

impl Default for MockConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl MockConverter {
    /// Create a new mock converter.
    pub fn new() -> Self {
        Self {
            conversions: Arc::new(RwLock::new(Vec::new())),
            probe_results: Arc::new(RwLock::new(HashMap::new())),
            next_error: Arc::new(RwLock::new(None)),
            conversion_duration_ms: Arc::new(RwLock::new(100)),
            send_progress: Arc::new(RwLock::new(true)),
            default_media_info: Arc::new(RwLock::new(None)),
        }
    }

    /// Get all recorded conversions.
    pub async fn recorded_conversions(&self) -> Vec<RecordedConversion> {
        self.conversions.read().await.clone()
    }

    /// Clear recorded conversions.
    pub async fn clear_recorded(&self) {
        self.conversions.write().await.clear();
    }

    /// Get the number of conversions performed.
    pub async fn conversion_count(&self) -> usize {
        self.conversions.read().await.len()
    }

    /// Set a probe result for a specific path.
    pub async fn set_probe_result(&self, path: impl AsRef<Path>, info: MediaInfo) {
        self.probe_results
            .write()
            .await
            .insert(path.as_ref().to_path_buf(), info);
    }

    /// Set the default media info for probing unknown files.
    pub async fn set_default_media_info(&self, info: MediaInfo) {
        *self.default_media_info.write().await = Some(info);
    }

    /// Clear all probe results.
    pub async fn clear_probe_results(&self) {
        self.probe_results.write().await.clear();
    }

    /// Configure the next operation to fail with the given error.
    pub async fn set_next_error(&self, error: ConverterError) {
        *self.next_error.write().await = Some(error);
    }

    /// Clear any pending error.
    pub async fn clear_next_error(&self) {
        *self.next_error.write().await = None;
    }

    /// Set the simulated conversion duration.
    pub async fn set_conversion_duration(&self, duration: Duration) {
        *self.conversion_duration_ms.write().await = duration.as_millis() as u64;
    }

    /// Enable or disable progress updates during conversion.
    pub async fn set_send_progress(&self, send: bool) {
        *self.send_progress.write().await = send;
    }

    /// Take the next error if set.
    async fn take_error(&self) -> Option<ConverterError> {
        self.next_error.write().await.take()
    }

    /// Create a default MediaInfo for testing.
    fn create_default_info(path: &Path) -> MediaInfo {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");

        let is_video = matches!(extension, "mkv" | "mp4" | "avi" | "mov" | "webm");

        MediaInfo {
            path: path.to_path_buf(),
            size_bytes: 100 * 1024 * 1024, // 100 MB
            duration_secs: if is_video { 7200.0 } else { 180.0 },
            format: extension.to_string(),
            audio_codec: Some("aac".to_string()),
            audio_bitrate_kbps: Some(320),
            audio_sample_rate: Some(48000),
            audio_channels: Some(2),
            video_codec: if is_video {
                Some("h264".to_string())
            } else {
                None
            },
            video_width: if is_video { Some(1920) } else { None },
            video_height: if is_video { Some(1080) } else { None },
            video_fps: if is_video { Some(24.0) } else { None },
        }
    }
}

#[async_trait]
impl Converter for MockConverter {
    fn name(&self) -> &str {
        "mock"
    }

    async fn probe(&self, path: &Path) -> Result<MediaInfo, ConverterError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        // Check for pre-configured result
        if let Some(info) = self.probe_results.read().await.get(path) {
            return Ok(info.clone());
        }

        // Check for default media info
        if let Some(info) = self.default_media_info.read().await.as_ref() {
            let mut info = info.clone();
            info.path = path.to_path_buf();
            return Ok(info);
        }

        // Generate default info based on path
        Ok(Self::create_default_info(path))
    }

    async fn convert(&self, job: ConversionJob) -> Result<ConversionResult, ConverterError> {
        if let Some(err) = self.take_error().await {
            self.conversions.write().await.push(RecordedConversion {
                job,
                success: false,
            });
            return Err(err);
        }

        // Record the conversion
        self.conversions.write().await.push(RecordedConversion {
            job: job.clone(),
            success: true,
        });

        // Simulate conversion time
        let duration_ms = *self.conversion_duration_ms.read().await;
        if duration_ms > 0 {
            tokio::time::sleep(Duration::from_millis(duration_ms)).await;
        }

        // Determine output format
        let output_format = match &job.constraints {
            ConversionConstraints::Audio(audio) => audio.format.extension().to_string(),
            ConversionConstraints::Video(video) => {
                video.container.extension(None).to_string()
            }
        };

        Ok(ConversionResult {
            job_id: job.job_id,
            output_path: job.output_path,
            output_size_bytes: 50 * 1024 * 1024, // 50 MB (compressed)
            duration_ms,
            input_format: job
                .input_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string(),
            output_format,
        })
    }

    async fn convert_with_progress(
        &self,
        job: ConversionJob,
        progress_tx: mpsc::Sender<ConversionProgress>,
    ) -> Result<ConversionResult, ConverterError> {
        let send_progress = *self.send_progress.read().await;
        let duration_ms = *self.conversion_duration_ms.read().await;

        if send_progress && duration_ms > 0 {
            let job_id = job.job_id.clone();

            // Send progress updates
            let steps = 5;
            let step_duration = duration_ms / steps;

            for i in 0..steps {
                let percent = ((i + 1) as f32 / steps as f32) * 100.0;
                let _ = progress_tx
                    .send(ConversionProgress {
                        job_id: job_id.clone(),
                        percent,
                        time_secs: (i as f64 + 1.0) * (step_duration as f64 / 1000.0),
                        duration_secs: Some(duration_ms as f64 / 1000.0),
                        speed: Some("10x".to_string()),
                    })
                    .await;

                tokio::time::sleep(Duration::from_millis(step_duration)).await;
            }
        }

        self.convert(job).await
    }

    async fn validate(&self) -> Result<(), ConverterError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }
        Ok(())
    }

    fn supported_input_formats(&self) -> &[&str] {
        &[
            "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "mkv", "mp4", "avi", "mov", "webm",
        ]
    }

    fn supported_output_formats(&self) -> &[&str] {
        &["flac", "mp3", "m4a", "ogg", "opus", "wav", "mkv", "mp4", "webm"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::{AudioConstraints, AudioFormat};

    fn create_test_job(id: &str) -> ConversionJob {
        ConversionJob {
            job_id: id.to_string(),
            input_path: PathBuf::from("/input/test.flac"),
            output_path: PathBuf::from("/output/test.ogg"),
            constraints: ConversionConstraints::Audio(AudioConstraints {
                format: AudioFormat::OggVorbis,
                bitrate_kbps: Some(320),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }),
            metadata: None,
            cover_art_path: None,
        }
    }

    #[tokio::test]
    async fn test_basic_conversion() {
        let converter = MockConverter::new();
        converter.set_conversion_duration(Duration::ZERO).await;

        let job = create_test_job("test-1");
        let result = converter.convert(job).await.unwrap();

        assert_eq!(result.job_id, "test-1");
        assert_eq!(result.output_format, "ogg");
    }

    #[tokio::test]
    async fn test_probe() {
        let converter = MockConverter::new();

        let info = converter.probe(Path::new("/test/video.mkv")).await.unwrap();
        assert_eq!(info.format, "mkv");
        assert!(info.video_codec.is_some());
        assert_eq!(info.video_width, Some(1920));
    }

    #[tokio::test]
    async fn test_custom_probe_result() {
        let converter = MockConverter::new();

        let custom_info = MediaInfo {
            path: PathBuf::from("/custom/file.mp3"),
            size_bytes: 5000000,
            duration_secs: 300.0,
            format: "mp3".to_string(),
            audio_codec: Some("mp3".to_string()),
            audio_bitrate_kbps: Some(192),
            audio_sample_rate: Some(44100),
            audio_channels: Some(2),
            video_codec: None,
            video_width: None,
            video_height: None,
            video_fps: None,
        };

        converter
            .set_probe_result("/custom/file.mp3", custom_info.clone())
            .await;

        let result = converter.probe(Path::new("/custom/file.mp3")).await.unwrap();
        assert_eq!(result.duration_secs, 300.0);
        assert_eq!(result.audio_bitrate_kbps, Some(192));
    }

    #[tokio::test]
    async fn test_recorded_conversions() {
        let converter = MockConverter::new();
        converter.set_conversion_duration(Duration::ZERO).await;

        converter.convert(create_test_job("job-1")).await.unwrap();
        converter.convert(create_test_job("job-2")).await.unwrap();

        let conversions = converter.recorded_conversions().await;
        assert_eq!(conversions.len(), 2);
        assert!(conversions[0].success);
        assert_eq!(conversions[0].job.job_id, "job-1");
    }

    #[tokio::test]
    async fn test_error_injection() {
        let converter = MockConverter::new();
        converter
            .set_next_error(ConverterError::conversion_failed("test error", None))
            .await;

        let result = converter.convert(create_test_job("fail")).await;
        assert!(result.is_err());

        // Error should be consumed, conversion recorded as failed
        let conversions = converter.recorded_conversions().await;
        assert_eq!(conversions.len(), 1);
        assert!(!conversions[0].success);
    }

    #[tokio::test]
    async fn test_progress_updates() {
        let converter = MockConverter::new();
        converter
            .set_conversion_duration(Duration::from_millis(50))
            .await;

        let (tx, mut rx) = mpsc::channel(10);

        let job = create_test_job("progress-test");
        tokio::spawn(async move {
            converter.convert_with_progress(job, tx).await.unwrap();
        });

        let mut progress_count = 0;
        while rx.recv().await.is_some() {
            progress_count += 1;
        }

        assert!(progress_count > 0);
    }
}
