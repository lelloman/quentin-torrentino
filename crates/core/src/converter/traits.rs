//! Trait definitions for the converter module.

use async_trait::async_trait;
use std::path::Path;
use tokio::sync::mpsc;

use super::error::ConverterError;
use super::types::{ConversionJob, ConversionProgress, ConversionResult, MediaInfo};

/// A converter that can transcode media files.
#[async_trait]
pub trait Converter: Send + Sync {
    /// Returns the name of this converter implementation.
    fn name(&self) -> &str;

    /// Probes a media file to get its information.
    async fn probe(&self, path: &Path) -> Result<MediaInfo, ConverterError>;

    /// Converts a media file according to the job specification.
    async fn convert(&self, job: ConversionJob) -> Result<ConversionResult, ConverterError>;

    /// Converts a media file with progress reporting.
    ///
    /// The progress sender will receive updates during conversion.
    /// If the sender is dropped, conversion continues without progress reporting.
    async fn convert_with_progress(
        &self,
        job: ConversionJob,
        progress_tx: mpsc::Sender<ConversionProgress>,
    ) -> Result<ConversionResult, ConverterError>;

    /// Validates that the converter is properly configured and ready.
    async fn validate(&self) -> Result<(), ConverterError>;

    /// Returns the supported input formats.
    fn supported_input_formats(&self) -> &[&str] {
        // Common formats supported by ffmpeg
        &[
            // Audio
            "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "wma", "ape", "alac",
            // Video
            "mkv", "mp4", "avi", "mov", "wmv", "webm", "ts", "m2ts",
        ]
    }

    /// Returns the supported output formats.
    fn supported_output_formats(&self) -> &[&str] {
        &[
            // Audio
            "flac", "mp3", "m4a", "ogg", "opus", "wav",
            // Video
            "mkv", "mp4", "webm",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct MockConverter;

    #[async_trait]
    impl Converter for MockConverter {
        fn name(&self) -> &str {
            "mock"
        }

        async fn probe(&self, path: &Path) -> Result<MediaInfo, ConverterError> {
            Ok(MediaInfo {
                path: path.to_path_buf(),
                size_bytes: 1024,
                duration_secs: 180.0,
                format: "flac".to_string(),
                audio_codec: Some("flac".to_string()),
                audio_bitrate_kbps: Some(1411),
                audio_sample_rate: Some(44100),
                audio_channels: Some(2),
                video_codec: None,
                video_width: None,
                video_height: None,
                video_fps: None,
            })
        }

        async fn convert(&self, job: ConversionJob) -> Result<ConversionResult, ConverterError> {
            Ok(ConversionResult {
                job_id: job.job_id,
                output_path: job.output_path,
                output_size_bytes: 512,
                duration_ms: 1000,
                input_format: "flac".to_string(),
                output_format: "ogg".to_string(),
            })
        }

        async fn convert_with_progress(
            &self,
            job: ConversionJob,
            _progress_tx: mpsc::Sender<ConversionProgress>,
        ) -> Result<ConversionResult, ConverterError> {
            self.convert(job).await
        }

        async fn validate(&self) -> Result<(), ConverterError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mock_converter_probe() {
        let converter = MockConverter;
        let info = converter.probe(Path::new("/test/file.flac")).await.unwrap();
        assert_eq!(info.format, "flac");
        assert_eq!(info.duration_secs, 180.0);
    }

    #[tokio::test]
    async fn test_mock_converter_convert() {
        let converter = MockConverter;
        let job = ConversionJob {
            job_id: "test-job".to_string(),
            input_path: PathBuf::from("/test/input.flac"),
            output_path: PathBuf::from("/test/output.ogg"),
            constraints: super::super::types::ConversionConstraints::default(),
            metadata: None,
            cover_art_path: None,
        };
        let result = converter.convert(job).await.unwrap();
        assert_eq!(result.job_id, "test-job");
    }

    #[test]
    fn test_supported_formats() {
        let converter = MockConverter;
        let input_formats = converter.supported_input_formats();
        assert!(input_formats.contains(&"flac"));
        assert!(input_formats.contains(&"mp3"));
        assert!(input_formats.contains(&"mkv"));
    }
}
