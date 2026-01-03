//! Hardware encoder capability detection.

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

use super::config::ConverterConfig;
use super::types::VideoFormat;

/// Available hardware encoders detected on the system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EncoderCapabilities {
    /// NVIDIA NVENC H.264 available
    pub h264_nvenc: bool,
    /// NVIDIA NVENC H.265/HEVC available
    pub hevc_nvenc: bool,
    /// NVIDIA NVENC AV1 available (RTX 40 series+)
    pub av1_nvenc: bool,
    /// Intel Quick Sync H.264 available
    pub h264_qsv: bool,
    /// Intel Quick Sync H.265/HEVC available
    pub hevc_qsv: bool,
    /// AMD AMF H.264 available
    pub h264_amf: bool,
    /// AMD AMF H.265/HEVC available
    pub hevc_amf: bool,
    /// VA-API H.264 available (Linux)
    pub h264_vaapi: bool,
    /// VA-API H.265/HEVC available (Linux)
    pub hevc_vaapi: bool,
}

impl EncoderCapabilities {
    /// Detect available hardware encoders by probing ffmpeg.
    pub async fn detect(config: &ConverterConfig) -> Self {
        let output = Command::new(&config.ffmpeg_path)
            .args(["-encoders"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await;

        let stdout = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return Self::default(),
        };

        Self {
            h264_nvenc: stdout.contains("h264_nvenc"),
            hevc_nvenc: stdout.contains("hevc_nvenc"),
            av1_nvenc: stdout.contains("av1_nvenc"),
            h264_qsv: stdout.contains("h264_qsv"),
            hevc_qsv: stdout.contains("hevc_qsv"),
            h264_amf: stdout.contains("h264_amf"),
            hevc_amf: stdout.contains("hevc_amf"),
            h264_vaapi: stdout.contains("h264_vaapi"),
            hevc_vaapi: stdout.contains("hevc_vaapi"),
        }
    }

    /// Returns a list of all available video formats (including hardware encoders).
    pub fn available_video_formats(&self) -> Vec<VideoFormat> {
        let mut formats = vec![
            VideoFormat::Copy,
            VideoFormat::H264,
            VideoFormat::H265,
            VideoFormat::Vp9,
            VideoFormat::Av1,
        ];

        // Add NVENC formats if available
        if self.h264_nvenc {
            formats.push(VideoFormat::H264Nvenc);
        }
        if self.hevc_nvenc {
            formats.push(VideoFormat::H265Nvenc);
        }
        if self.av1_nvenc {
            formats.push(VideoFormat::Av1Nvenc);
        }

        formats
    }

    /// Check if any hardware encoder is available.
    pub fn has_hardware_encoder(&self) -> bool {
        self.h264_nvenc
            || self.hevc_nvenc
            || self.av1_nvenc
            || self.h264_qsv
            || self.hevc_qsv
            || self.h264_amf
            || self.hevc_amf
            || self.h264_vaapi
            || self.hevc_vaapi
    }

    /// Check if NVENC is available.
    pub fn has_nvenc(&self) -> bool {
        self.h264_nvenc || self.hevc_nvenc || self.av1_nvenc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_capabilities() {
        let caps = EncoderCapabilities::default();
        assert!(!caps.h264_nvenc);
        assert!(!caps.has_hardware_encoder());
    }

    #[test]
    fn test_available_formats_no_hardware() {
        let caps = EncoderCapabilities::default();
        let formats = caps.available_video_formats();
        assert!(formats.contains(&VideoFormat::H264));
        assert!(formats.contains(&VideoFormat::Copy));
        assert!(!formats.contains(&VideoFormat::H264Nvenc));
    }

    #[test]
    fn test_available_formats_with_nvenc() {
        let caps = EncoderCapabilities {
            h264_nvenc: true,
            hevc_nvenc: true,
            ..Default::default()
        };
        let formats = caps.available_video_formats();
        assert!(formats.contains(&VideoFormat::H264Nvenc));
        assert!(formats.contains(&VideoFormat::H265Nvenc));
        assert!(!formats.contains(&VideoFormat::Av1Nvenc));
    }
}
