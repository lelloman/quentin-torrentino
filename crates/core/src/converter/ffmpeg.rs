//! FFmpeg-based converter implementation.

use async_trait::async_trait;
use regex_lite::Regex;
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

use super::config::ConverterConfig;
use super::error::ConverterError;
use super::traits::Converter;
use super::types::{
    AudioConstraints, AudioFormat, ConversionConstraints, ConversionJob, ConversionProgress,
    ConversionResult, MediaInfo, VideoConstraints,
};

/// FFmpeg-based converter implementation.
pub struct FfmpegConverter {
    config: ConverterConfig,
}

impl FfmpegConverter {
    /// Creates a new FFmpeg converter with the given configuration.
    pub fn new(config: ConverterConfig) -> Self {
        Self { config }
    }

    /// Creates a converter with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ConverterConfig::default())
    }

    /// Builds ffmpeg arguments for audio conversion.
    fn build_audio_args(
        &self,
        input_path: &Path,
        output_path: &Path,
        constraints: &AudioConstraints,
        metadata_args: &[String],
        cover_art_path: Option<&Path>,
    ) -> Vec<String> {
        let mut args = vec![
            "-y".to_string(), // Overwrite output
            "-i".to_string(),
            input_path.to_string_lossy().to_string(),
        ];

        // Add cover art if provided (for formats that support it)
        if let Some(cover_path) = cover_art_path {
            if matches!(
                constraints.format,
                AudioFormat::Mp3 | AudioFormat::Flac | AudioFormat::OggVorbis
            ) {
                args.extend([
                    "-i".to_string(),
                    cover_path.to_string_lossy().to_string(),
                    "-map".to_string(),
                    "0:a".to_string(),
                    "-map".to_string(),
                    "1:v".to_string(),
                    "-c:v".to_string(),
                    "copy".to_string(),
                    "-disposition:v:0".to_string(),
                    "attached_pic".to_string(),
                ]);
            }
        }

        // Audio codec
        args.extend([
            "-c:a".to_string(),
            constraints.format.ffmpeg_codec().to_string(),
        ]);

        // Bitrate (for lossy formats)
        if !constraints.format.is_lossless() {
            if let Some(bitrate) = constraints.bitrate_kbps {
                args.extend(["-b:a".to_string(), format!("{}k", bitrate)]);
            }
        }

        // Compression level (for lossless formats)
        if constraints.format.is_lossless() {
            if let Some(level) = constraints.compression_level {
                args.extend(["-compression_level".to_string(), level.to_string()]);
            }
        }

        // Sample rate
        if let Some(rate) = constraints.sample_rate_hz {
            args.extend(["-ar".to_string(), rate.to_string()]);
        }

        // Channels
        if let Some(channels) = constraints.channels {
            args.extend(["-ac".to_string(), channels.to_string()]);
        }

        // Metadata
        args.extend(metadata_args.iter().cloned());

        // Log level
        args.extend([
            "-loglevel".to_string(),
            self.config.ffmpeg_log_level.clone(),
        ]);

        // Progress output for parsing
        args.extend(["-progress".to_string(), "pipe:2".to_string()]);

        // Extra args
        args.extend(self.config.extra_ffmpeg_args.iter().cloned());

        // Output
        args.push(output_path.to_string_lossy().to_string());

        args
    }

    /// Builds ffmpeg arguments for video conversion.
    fn build_video_args(
        &self,
        input_path: &Path,
        output_path: &Path,
        constraints: &VideoConstraints,
        metadata_args: &[String],
    ) -> Vec<String> {
        let mut args = vec![
            "-y".to_string(),
            "-i".to_string(),
            input_path.to_string_lossy().to_string(),
        ];

        // Video codec
        args.extend([
            "-c:v".to_string(),
            constraints.format.ffmpeg_codec().to_string(),
        ]);

        // Quality settings
        if constraints.format != super::types::VideoFormat::Copy {
            if let Some(crf) = constraints.crf {
                args.extend(["-crf".to_string(), crf.to_string()]);
            } else if let Some(bitrate) = constraints.bitrate_kbps {
                args.extend(["-b:v".to_string(), format!("{}k", bitrate)]);
            }

            // Resolution scaling
            if constraints.max_width.is_some() || constraints.max_height.is_some() {
                let width = constraints.max_width.unwrap_or(u32::MAX);
                let height = constraints.max_height.unwrap_or(u32::MAX);
                // Scale while maintaining aspect ratio, only if larger than max
                args.extend([
                    "-vf".to_string(),
                    format!(
                        "scale='min({},iw)':'min({},ih)':force_original_aspect_ratio=decrease",
                        width, height
                    ),
                ]);
            }

            // Frame rate
            if let Some(fps) = constraints.fps {
                args.extend(["-r".to_string(), fps.to_string()]);
            }
        }

        // Audio settings
        if let Some(ref audio) = constraints.audio {
            args.extend(["-c:a".to_string(), audio.format.ffmpeg_codec().to_string()]);
            if let Some(bitrate) = audio.bitrate_kbps {
                args.extend(["-b:a".to_string(), format!("{}k", bitrate)]);
            }
            if let Some(rate) = audio.sample_rate_hz {
                args.extend(["-ar".to_string(), rate.to_string()]);
            }
        }

        // Metadata
        args.extend(metadata_args.iter().cloned());

        // Log level and progress
        args.extend([
            "-loglevel".to_string(),
            self.config.ffmpeg_log_level.clone(),
            "-progress".to_string(),
            "pipe:2".to_string(),
        ]);

        // Extra args
        args.extend(self.config.extra_ffmpeg_args.iter().cloned());

        // Output
        args.push(output_path.to_string_lossy().to_string());

        args
    }

    /// Parses ffprobe JSON output into MediaInfo.
    fn parse_probe_output(path: &Path, output: &str) -> Result<MediaInfo, ConverterError> {
        #[derive(Deserialize)]
        struct ProbeOutput {
            format: ProbeFormat,
            streams: Vec<ProbeStream>,
        }

        #[derive(Deserialize)]
        struct ProbeFormat {
            #[allow(dead_code)]
            filename: String,
            format_name: String,
            duration: Option<String>,
            size: Option<String>,
        }

        #[derive(Deserialize)]
        struct ProbeStream {
            codec_type: String,
            codec_name: Option<String>,
            bit_rate: Option<String>,
            sample_rate: Option<String>,
            channels: Option<u8>,
            width: Option<u32>,
            height: Option<u32>,
            r_frame_rate: Option<String>,
        }

        let probe: ProbeOutput =
            serde_json::from_str(output).map_err(|e| ConverterError::ParseError {
                reason: format!("Failed to parse ffprobe output: {}", e),
            })?;

        let duration_secs = probe
            .format
            .duration
            .as_ref()
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0);

        let size_bytes = probe
            .format
            .size
            .as_ref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        // Find audio stream
        let audio_stream = probe.streams.iter().find(|s| s.codec_type == "audio");

        // Find video stream
        let video_stream = probe.streams.iter().find(|s| s.codec_type == "video");

        let format_name = probe
            .format
            .format_name
            .split(',')
            .next()
            .unwrap_or("unknown");

        Ok(MediaInfo {
            path: path.to_path_buf(),
            size_bytes,
            duration_secs,
            format: format_name.to_string(),
            audio_codec: audio_stream.and_then(|s| s.codec_name.clone()),
            audio_bitrate_kbps: audio_stream
                .and_then(|s| s.bit_rate.as_ref())
                .and_then(|b| b.parse::<u32>().ok())
                .map(|b| b / 1000),
            audio_sample_rate: audio_stream
                .and_then(|s| s.sample_rate.as_ref())
                .and_then(|r| r.parse::<u32>().ok()),
            audio_channels: audio_stream.and_then(|s| s.channels),
            video_codec: video_stream.and_then(|s| s.codec_name.clone()),
            video_width: video_stream.and_then(|s| s.width),
            video_height: video_stream.and_then(|s| s.height),
            video_fps: video_stream
                .and_then(|s| s.r_frame_rate.as_ref())
                .and_then(|r| {
                    // Parse frame rate like "24000/1001" or "30/1"
                    let parts: Vec<&str> = r.split('/').collect();
                    if parts.len() == 2 {
                        let num = parts[0].parse::<f32>().ok()?;
                        let den = parts[1].parse::<f32>().ok()?;
                        if den > 0.0 {
                            Some(num / den)
                        } else {
                            None
                        }
                    } else {
                        r.parse::<f32>().ok()
                    }
                }),
        })
    }

    /// Runs the conversion with optional progress reporting.
    async fn run_conversion(
        &self,
        job: &ConversionJob,
        progress_tx: Option<mpsc::Sender<ConversionProgress>>,
    ) -> Result<ConversionResult, ConverterError> {
        let start = Instant::now();

        // Ensure output directory exists
        if let Some(parent) = job.output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|_| {
                ConverterError::OutputDirectoryFailed {
                    path: parent.to_path_buf(),
                }
            })?;
        }

        // Get input duration for progress calculation
        let input_info = self.probe(&job.input_path).await.ok();
        let duration_secs = input_info.as_ref().map(|i| i.duration_secs);

        // Build arguments
        let metadata_args = job
            .metadata
            .as_ref()
            .map(|m| m.to_ffmpeg_args())
            .unwrap_or_default();

        let args = match &job.constraints {
            ConversionConstraints::Audio(audio) => self.build_audio_args(
                &job.input_path,
                &job.output_path,
                audio,
                &metadata_args,
                job.cover_art_path.as_deref(),
            ),
            ConversionConstraints::Video(video) => {
                self.build_video_args(&job.input_path, &job.output_path, video, &metadata_args)
            }
        };

        // Run ffmpeg
        let mut child = Command::new(&self.config.ffmpeg_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ConverterError::FfmpegNotFound {
                        path: self.config.ffmpeg_path.clone(),
                    }
                } else {
                    ConverterError::Io(e)
                }
            })?;

        let stderr = child.stderr.take().expect("stderr should be captured");
        let mut reader = BufReader::new(stderr).lines();

        // Track progress
        let mut current_time = 0.0;
        let mut current_speed = None;
        let time_regex = Regex::new(r"out_time_ms=(\d+)").ok();
        let speed_regex = Regex::new(r"speed=(\d+\.?\d*)x").ok();

        // Read progress from stderr
        let timeout_duration = Duration::from_secs(self.config.timeout_secs);
        let result = timeout(timeout_duration, async {
            let mut last_progress_send = Instant::now();
            let progress_interval = Duration::from_millis(500);
            let mut error_output = String::new();

            while let Ok(Some(line)) = reader.next_line().await {
                // Capture error output
                if line.contains("Error") || line.contains("error") {
                    error_output.push_str(&line);
                    error_output.push('\n');
                }

                // Parse progress
                if let Some(ref re) = time_regex {
                    if let Some(caps) = re.captures(&line) {
                        if let Some(ms_str) = caps.get(1) {
                            if let Ok(ms) = ms_str.as_str().parse::<f64>() {
                                current_time = ms / 1_000_000.0; // Convert microseconds to seconds
                            }
                        }
                    }
                }

                if let Some(ref re) = speed_regex {
                    if let Some(caps) = re.captures(&line) {
                        if let Some(speed_str) = caps.get(1) {
                            current_speed = Some(format!("{}x", speed_str.as_str()));
                        }
                    }
                }

                // Send progress update
                if let Some(ref tx) = progress_tx {
                    if last_progress_send.elapsed() >= progress_interval {
                        let percent = if let Some(dur) = duration_secs {
                            if dur > 0.0 {
                                (current_time / dur * 100.0).min(100.0) as f32
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        };

                        let progress = ConversionProgress {
                            job_id: job.job_id.clone(),
                            percent,
                            time_secs: current_time,
                            duration_secs,
                            speed: current_speed.clone(),
                        };

                        // Non-blocking send
                        let _ = tx.try_send(progress);
                        last_progress_send = Instant::now();
                    }
                }
            }

            // Wait for process to complete
            let status = child.wait().await?;
            Ok::<(std::process::ExitStatus, String), std::io::Error>((status, error_output))
        })
        .await;

        match result {
            Ok(Ok((status, error_output))) => {
                if !status.success() {
                    return Err(ConverterError::conversion_failed(
                        format!("FFmpeg exited with code: {:?}", status.code()),
                        if error_output.is_empty() {
                            None
                        } else {
                            Some(error_output)
                        },
                    ));
                }
            }
            Ok(Err(e)) => return Err(ConverterError::Io(e)),
            Err(_) => {
                // Kill the process on timeout
                let _ = child.kill().await;
                return Err(ConverterError::Timeout {
                    timeout_secs: self.config.timeout_secs,
                });
            }
        }

        // Verify output exists and get size
        let output_meta = tokio::fs::metadata(&job.output_path)
            .await
            .map_err(|_| ConverterError::conversion_failed("Output file not created", None))?;

        let output_format = match &job.constraints {
            ConversionConstraints::Audio(a) => a.format.extension().to_string(),
            ConversionConstraints::Video(v) => v
                .container
                .extension(v.audio.as_ref().map(|a| &a.format))
                .to_string(),
        };

        let input_format = input_info
            .map(|i| i.format)
            .unwrap_or_else(|| "unknown".to_string());

        Ok(ConversionResult {
            job_id: job.job_id.clone(),
            output_path: job.output_path.clone(),
            output_size_bytes: output_meta.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            input_format,
            output_format,
        })
    }
}

#[async_trait]
impl Converter for FfmpegConverter {
    fn name(&self) -> &str {
        "ffmpeg"
    }

    async fn probe(&self, path: &Path) -> Result<MediaInfo, ConverterError> {
        if !path.exists() {
            return Err(ConverterError::InputNotFound {
                path: path.to_path_buf(),
            });
        }

        let output = Command::new(&self.config.ffprobe_path)
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(path)
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ConverterError::FfprobeNotFound {
                        path: self.config.ffprobe_path.clone(),
                    }
                } else {
                    ConverterError::Io(e)
                }
            })?;

        if !output.status.success() {
            return Err(ConverterError::probe_failed(format!(
                "ffprobe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::parse_probe_output(path, &stdout)
    }

    async fn convert(&self, job: ConversionJob) -> Result<ConversionResult, ConverterError> {
        self.run_conversion(&job, None).await
    }

    async fn convert_with_progress(
        &self,
        job: ConversionJob,
        progress_tx: mpsc::Sender<ConversionProgress>,
    ) -> Result<ConversionResult, ConverterError> {
        self.run_conversion(&job, Some(progress_tx)).await
    }

    async fn validate(&self) -> Result<(), ConverterError> {
        // Check ffmpeg exists
        let ffmpeg_result = Command::new(&self.config.ffmpeg_path)
            .arg("-version")
            .output()
            .await;

        if let Err(e) = ffmpeg_result {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(ConverterError::FfmpegNotFound {
                    path: self.config.ffmpeg_path.clone(),
                });
            }
            return Err(ConverterError::Io(e));
        }

        // Check ffprobe exists
        let ffprobe_result = Command::new(&self.config.ffprobe_path)
            .arg("-version")
            .output()
            .await;

        if let Err(e) = ffprobe_result {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(ConverterError::FfprobeNotFound {
                    path: self.config.ffprobe_path.clone(),
                });
            }
            return Err(ConverterError::Io(e));
        }

        // Ensure temp dir exists
        tokio::fs::create_dir_all(&self.config.temp_dir).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_audio_args_mp3() {
        let converter = FfmpegConverter::with_defaults();
        let constraints = AudioConstraints {
            format: AudioFormat::Mp3,
            bitrate_kbps: Some(320),
            sample_rate_hz: Some(44100),
            channels: None,
            compression_level: None,
        };

        let args = converter.build_audio_args(
            Path::new("/input.flac"),
            Path::new("/output.mp3"),
            &constraints,
            &[],
            None,
        );

        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"libmp3lame".to_string()));
        assert!(args.contains(&"-b:a".to_string()));
        assert!(args.contains(&"320k".to_string()));
        assert!(args.contains(&"-ar".to_string()));
        assert!(args.contains(&"44100".to_string()));
    }

    #[test]
    fn test_build_audio_args_flac() {
        let converter = FfmpegConverter::with_defaults();
        let constraints = AudioConstraints {
            format: AudioFormat::Flac,
            bitrate_kbps: None,
            sample_rate_hz: None,
            channels: None,
            compression_level: Some(8),
        };

        let args = converter.build_audio_args(
            Path::new("/input.wav"),
            Path::new("/output.flac"),
            &constraints,
            &[],
            None,
        );

        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"flac".to_string()));
        assert!(args.contains(&"-compression_level".to_string()));
        assert!(args.contains(&"8".to_string()));
        // Should not have bitrate for lossless
        assert!(!args.contains(&"-b:a".to_string()));
    }

    #[test]
    fn test_build_video_args() {
        let converter = FfmpegConverter::with_defaults();
        let constraints = VideoConstraints {
            format: super::super::types::VideoFormat::H264,
            container: super::super::types::ContainerFormat::Mp4,
            crf: Some(23),
            bitrate_kbps: None,
            max_width: Some(1920),
            max_height: Some(1080),
            fps: None,
            audio: Some(AudioConstraints {
                format: AudioFormat::Aac,
                bitrate_kbps: Some(192),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            }),
        };

        let args = converter.build_video_args(
            Path::new("/input.mkv"),
            Path::new("/output.mp4"),
            &constraints,
            &[],
        );

        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-crf".to_string()));
        assert!(args.contains(&"23".to_string()));
        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"aac".to_string()));
    }

    #[test]
    fn test_parse_probe_output() {
        let json = r#"{
            "format": {
                "filename": "test.flac",
                "format_name": "flac",
                "duration": "180.5",
                "size": "30000000"
            },
            "streams": [
                {
                    "codec_type": "audio",
                    "codec_name": "flac",
                    "bit_rate": "1411000",
                    "sample_rate": "44100",
                    "channels": 2
                }
            ]
        }"#;

        let info = FfmpegConverter::parse_probe_output(Path::new("test.flac"), json).unwrap();
        assert_eq!(info.format, "flac");
        assert!((info.duration_secs - 180.5).abs() < 0.01);
        assert_eq!(info.size_bytes, 30000000);
        assert_eq!(info.audio_codec, Some("flac".to_string()));
        assert_eq!(info.audio_sample_rate, Some(44100));
        assert_eq!(info.audio_channels, Some(2));
    }

    #[test]
    fn test_parse_probe_output_video() {
        let json = r#"{
            "format": {
                "filename": "test.mkv",
                "format_name": "matroska,webm",
                "duration": "7200.0",
                "size": "5000000000"
            },
            "streams": [
                {
                    "codec_type": "video",
                    "codec_name": "h264",
                    "width": 1920,
                    "height": 1080,
                    "r_frame_rate": "24000/1001"
                },
                {
                    "codec_type": "audio",
                    "codec_name": "aac",
                    "bit_rate": "192000",
                    "sample_rate": "48000",
                    "channels": 6
                }
            ]
        }"#;

        let info = FfmpegConverter::parse_probe_output(Path::new("test.mkv"), json).unwrap();
        assert_eq!(info.format, "matroska");
        assert_eq!(info.video_codec, Some("h264".to_string()));
        assert_eq!(info.video_width, Some(1920));
        assert_eq!(info.video_height, Some(1080));
        // 24000/1001 ≈ 23.976
        assert!(info.video_fps.is_some());
        let fps = info.video_fps.unwrap();
        assert!((fps - 23.976).abs() < 0.01);
        assert_eq!(info.audio_codec, Some("aac".to_string()));
        assert_eq!(info.audio_channels, Some(6));
    }
}
