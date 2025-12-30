//! Types for the converter module.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Audio format specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// Free Lossless Audio Codec (lossless)
    Flac,
    /// MPEG Audio Layer III
    Mp3,
    /// Advanced Audio Coding
    Aac,
    /// Ogg Vorbis
    OggVorbis,
    /// Opus (modern, efficient)
    Opus,
    /// WAVE (uncompressed)
    Wav,
    /// Apple Lossless
    Alac,
}

impl AudioFormat {
    /// Returns the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Mp3 => "mp3",
            Self::Aac => "m4a",
            Self::OggVorbis => "ogg",
            Self::Opus => "opus",
            Self::Wav => "wav",
            Self::Alac => "m4a",
        }
    }

    /// Returns the ffmpeg codec name for this format.
    pub fn ffmpeg_codec(&self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Mp3 => "libmp3lame",
            Self::Aac => "aac",
            Self::OggVorbis => "libvorbis",
            Self::Opus => "libopus",
            Self::Wav => "pcm_s16le",
            Self::Alac => "alac",
        }
    }

    /// Whether this format is lossless.
    pub fn is_lossless(&self) -> bool {
        matches!(self, Self::Flac | Self::Wav | Self::Alac)
    }
}

/// Video format specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoFormat {
    /// H.264 / AVC
    H264,
    /// H.265 / HEVC
    H265,
    /// VP9
    Vp9,
    /// AV1
    Av1,
    /// Copy (no re-encoding)
    Copy,
}

impl VideoFormat {
    /// Returns the ffmpeg codec name for this format.
    pub fn ffmpeg_codec(&self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
            Self::Vp9 => "libvpx-vp9",
            Self::Av1 => "libaom-av1",
            Self::Copy => "copy",
        }
    }
}

/// Container format for output files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerFormat {
    /// Matroska (.mkv)
    Mkv,
    /// MPEG-4 Part 14 (.mp4)
    Mp4,
    /// WebM
    Webm,
    /// Audio only (inferred from audio format)
    AudioOnly,
}

impl ContainerFormat {
    /// Returns the file extension for this container.
    pub fn extension(&self, audio_format: Option<&AudioFormat>) -> &'static str {
        match self {
            Self::Mkv => "mkv",
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::AudioOnly => audio_format.map(|f| f.extension()).unwrap_or("audio"),
        }
    }
}

/// Constraints for audio conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioConstraints {
    /// Target audio format.
    pub format: AudioFormat,
    /// Target bitrate in kbps (for lossy formats).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    /// Target sample rate in Hz.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
    /// Number of audio channels (1 = mono, 2 = stereo).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
    /// Compression level for lossless formats (0-12 for FLAC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_level: Option<u8>,
}

impl Default for AudioConstraints {
    fn default() -> Self {
        Self {
            format: AudioFormat::OggVorbis,
            bitrate_kbps: Some(320),
            sample_rate_hz: None, // Keep original
            channels: None,       // Keep original
            compression_level: None,
        }
    }
}

/// Constraints for video conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoConstraints {
    /// Target video codec.
    pub format: VideoFormat,
    /// Target container.
    pub container: ContainerFormat,
    /// Constant Rate Factor (quality, lower = better, 0-51 for x264/x265).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crf: Option<u8>,
    /// Target video bitrate in kbps (alternative to CRF).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    /// Maximum width (height scaled proportionally).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    /// Maximum height (width scaled proportionally).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u32>,
    /// Target frame rate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<f32>,
    /// Audio constraints for video files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioConstraints>,
}

impl Default for VideoConstraints {
    fn default() -> Self {
        Self {
            format: VideoFormat::H264,
            container: ContainerFormat::Mp4,
            crf: Some(23), // Default quality
            bitrate_kbps: None,
            max_width: None,
            max_height: None,
            fps: None,
            audio: Some(AudioConstraints {
                format: AudioFormat::Aac,
                bitrate_kbps: Some(192),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }),
        }
    }
}

/// Combined conversion constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversionConstraints {
    /// Audio-only conversion.
    Audio(AudioConstraints),
    /// Video conversion (includes audio track).
    Video(VideoConstraints),
}

impl Default for ConversionConstraints {
    fn default() -> Self {
        Self::Audio(AudioConstraints::default())
    }
}

/// Metadata to embed in the output file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddedMetadata {
    /// Track/content title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Artist/creator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Album name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// Album artist (if different from track artist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    /// Release year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Track number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u16>,
    /// Total tracks in album.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_total: Option<u16>,
    /// Disc number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u16>,
    /// Total discs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_total: Option<u16>,
    /// Genre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// Comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Additional metadata fields.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

impl EmbeddedMetadata {
    /// Convert to ffmpeg metadata arguments.
    pub fn to_ffmpeg_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(ref title) = self.title {
            args.extend(["-metadata".to_string(), format!("title={}", title)]);
        }
        if let Some(ref artist) = self.artist {
            args.extend(["-metadata".to_string(), format!("artist={}", artist)]);
        }
        if let Some(ref album) = self.album {
            args.extend(["-metadata".to_string(), format!("album={}", album)]);
        }
        if let Some(ref album_artist) = self.album_artist {
            args.extend([
                "-metadata".to_string(),
                format!("album_artist={}", album_artist),
            ]);
        }
        if let Some(year) = self.year {
            args.extend(["-metadata".to_string(), format!("date={}", year)]);
        }
        if let Some(track) = self.track_number {
            if let Some(total) = self.track_total {
                args.extend(["-metadata".to_string(), format!("track={}/{}", track, total)]);
            } else {
                args.extend(["-metadata".to_string(), format!("track={}", track)]);
            }
        }
        if let Some(disc) = self.disc_number {
            if let Some(total) = self.disc_total {
                args.extend(["-metadata".to_string(), format!("disc={}/{}", disc, total)]);
            } else {
                args.extend(["-metadata".to_string(), format!("disc={}", disc)]);
            }
        }
        if let Some(ref genre) = self.genre {
            args.extend(["-metadata".to_string(), format!("genre={}", genre)]);
        }
        if let Some(ref comment) = self.comment {
            args.extend(["-metadata".to_string(), format!("comment={}", comment)]);
        }
        for (key, value) in &self.extra {
            args.extend(["-metadata".to_string(), format!("{}={}", key, value)]);
        }

        args
    }
}

/// A conversion job request.
#[derive(Debug, Clone)]
pub struct ConversionJob {
    /// Unique job ID (usually ticket_id + item_id).
    pub job_id: String,
    /// Input file path.
    pub input_path: PathBuf,
    /// Output file path.
    pub output_path: PathBuf,
    /// Conversion constraints.
    pub constraints: ConversionConstraints,
    /// Metadata to embed.
    pub metadata: Option<EmbeddedMetadata>,
    /// Cover art to embed (for audio).
    pub cover_art_path: Option<PathBuf>,
}

/// Result of a successful conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    /// Job ID.
    pub job_id: String,
    /// Output file path.
    pub output_path: PathBuf,
    /// Output file size in bytes.
    pub output_size_bytes: u64,
    /// Conversion duration in milliseconds.
    pub duration_ms: u64,
    /// Detected input format.
    pub input_format: String,
    /// Output format used.
    pub output_format: String,
}

/// Information about a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    /// File path.
    pub path: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Container format (e.g., "flac", "mp4").
    pub format: String,
    /// Audio codec (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    /// Audio bitrate in kbps (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_bitrate_kbps: Option<u32>,
    /// Audio sample rate (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_sample_rate: Option<u32>,
    /// Audio channels (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channels: Option<u8>,
    /// Video codec (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    /// Video width (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_width: Option<u32>,
    /// Video height (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_height: Option<u32>,
    /// Video frame rate (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_fps: Option<f32>,
}

/// Progress update during conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionProgress {
    /// Job ID.
    pub job_id: String,
    /// Progress percentage (0.0 - 100.0).
    pub percent: f32,
    /// Current processing time in seconds.
    pub time_secs: f64,
    /// Estimated total duration in seconds.
    pub duration_secs: Option<f64>,
    /// Current processing speed (e.g., "1.5x").
    pub speed: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_extension() {
        assert_eq!(AudioFormat::Flac.extension(), "flac");
        assert_eq!(AudioFormat::Mp3.extension(), "mp3");
        assert_eq!(AudioFormat::OggVorbis.extension(), "ogg");
        assert_eq!(AudioFormat::Opus.extension(), "opus");
    }

    #[test]
    fn test_audio_format_codec() {
        assert_eq!(AudioFormat::Flac.ffmpeg_codec(), "flac");
        assert_eq!(AudioFormat::Mp3.ffmpeg_codec(), "libmp3lame");
        assert_eq!(AudioFormat::OggVorbis.ffmpeg_codec(), "libvorbis");
    }

    #[test]
    fn test_audio_format_lossless() {
        assert!(AudioFormat::Flac.is_lossless());
        assert!(AudioFormat::Wav.is_lossless());
        assert!(AudioFormat::Alac.is_lossless());
        assert!(!AudioFormat::Mp3.is_lossless());
        assert!(!AudioFormat::OggVorbis.is_lossless());
    }

    #[test]
    fn test_embedded_metadata_to_ffmpeg_args() {
        let metadata = EmbeddedMetadata {
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            year: Some(2024),
            track_number: Some(1),
            track_total: Some(12),
            ..Default::default()
        };

        let args = metadata.to_ffmpeg_args();
        assert!(args.contains(&"-metadata".to_string()));
        assert!(args.contains(&"title=Test Song".to_string()));
        assert!(args.contains(&"artist=Test Artist".to_string()));
        assert!(args.contains(&"album=Test Album".to_string()));
        assert!(args.contains(&"date=2024".to_string()));
        assert!(args.contains(&"track=1/12".to_string()));
    }

    #[test]
    fn test_video_format_codec() {
        assert_eq!(VideoFormat::H264.ffmpeg_codec(), "libx264");
        assert_eq!(VideoFormat::H265.ffmpeg_codec(), "libx265");
        assert_eq!(VideoFormat::Copy.ffmpeg_codec(), "copy");
    }

    #[test]
    fn test_container_extension() {
        assert_eq!(ContainerFormat::Mkv.extension(None), "mkv");
        assert_eq!(ContainerFormat::Mp4.extension(None), "mp4");
        assert_eq!(
            ContainerFormat::AudioOnly.extension(Some(&AudioFormat::Flac)),
            "flac"
        );
    }
}
