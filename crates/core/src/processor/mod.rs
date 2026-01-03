//! Processor module for the media processing pipeline.
//!
//! This module provides the `PipelineProcessor` which coordinates:
//! - Conversion: Transcoding downloaded files to target format
//! - Placement: Moving converted files to their final destinations
//!
//! The processor uses semaphores to limit concurrent operations and
//! provides progress reporting through channels.
//!
//! # Example
//!
//! ```ignore
//! use torrentino_core::processor::{PipelineProcessor, ProcessorConfig, PipelineJob};
//! use torrentino_core::converter::{FfmpegConverter, ConverterConfig};
//! use torrentino_core::placer::{FsPlacer, PlacerConfig};
//!
//! // Create processor with converters and placers
//! let config = ProcessorConfig::default();
//! let converter = FfmpegConverter::new(ConverterConfig::default());
//! let placer = FsPlacer::new(PlacerConfig::default());
//!
//! let processor = PipelineProcessor::new(config, converter, placer);
//! processor.start().await;
//!
//! // Get pipeline status
//! let status = processor.status().await;
//! println!("Active conversions: {}", status.conversion_pool.active_jobs);
//!
//! // Process a job
//! let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(100);
//! processor.process(job, Some(progress_tx)).await?;
//!
//! // Monitor progress
//! while let Some(progress) = progress_rx.recv().await {
//!     println!("Progress: {:?}", progress);
//! }
//! ```

mod config;
mod pipeline;
mod types;

pub use config::{ProcessorConfig, RetryConfig};
pub use pipeline::{PipelineError, PipelineProcessor, PipelineProgressCallback, PipelineUpdateCallback};
pub use types::{
    PipelineJob, PipelineMetadata, PipelineProgress, PipelineResult, PipelineStatus,
    PlacedFileInfo, PoolStatus, SourceFile,
};
