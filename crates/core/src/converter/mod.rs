//! Converter module for transcoding media files.
//!
//! This module provides the `Converter` trait and implementations for converting
//! media files between different formats using FFmpeg.
//!
//! # Features
//!
//! - Audio transcoding (FLAC, MP3, AAC, Vorbis, Opus, WAV, ALAC)
//! - Video transcoding (H.264, H.265, VP9, AV1)
//! - Metadata embedding
//! - Cover art embedding for audio files
//! - Progress reporting during conversion
//!
//! # Example
//!
//! ```ignore
//! use torrentino_core::converter::{FfmpegConverter, Converter, ConversionJob, AudioConstraints, AudioFormat};
//!
//! let converter = FfmpegConverter::with_defaults();
//!
//! // Validate ffmpeg is available
//! converter.validate().await?;
//!
//! // Probe a media file
//! let info = converter.probe(Path::new("/path/to/file.flac")).await?;
//! println!("Duration: {} seconds", info.duration_secs);
//!
//! // Convert to OGG Vorbis
//! let job = ConversionJob {
//!     job_id: "job-1".to_string(),
//!     input_path: PathBuf::from("/path/to/input.flac"),
//!     output_path: PathBuf::from("/path/to/output.ogg"),
//!     constraints: ConversionConstraints::Audio(AudioConstraints {
//!         format: AudioFormat::OggVorbis,
//!         bitrate_kbps: Some(320),
//!         ..Default::default()
//!     }),
//!     metadata: Some(EmbeddedMetadata {
//!         title: Some("Song Title".to_string()),
//!         artist: Some("Artist Name".to_string()),
//!         ..Default::default()
//!     }),
//!     cover_art_path: None,
//! };
//!
//! let result = converter.convert(job).await?;
//! println!("Converted in {} ms", result.duration_ms);
//! ```

mod capabilities;
mod config;
mod error;
mod ffmpeg;
mod traits;
mod types;

pub use capabilities::EncoderCapabilities;
pub use config::ConverterConfig;
pub use error::ConverterError;
pub use ffmpeg::FfmpegConverter;
pub use traits::Converter;
pub use types::{
    AudioConstraints, AudioFormat, ContainerFormat, ConversionConstraints, ConversionJob,
    ConversionProgress, ConversionResult, EmbeddedMetadata, MediaInfo, VideoConstraints,
    VideoFormat,
};
