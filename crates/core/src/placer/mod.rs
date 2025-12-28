//! Placer module for moving files to their final destinations.
//!
//! This module provides the `Placer` trait and implementations for placing
//! converted files in their target locations with support for atomic moves,
//! checksum verification, and rollback on failure.
//!
//! # Features
//!
//! - Atomic moves when source and destination are on the same filesystem
//! - Automatic fallback to copy when atomic move fails
//! - Checksum verification after placement
//! - Rollback support for partial failures
//! - Automatic parent directory creation
//! - Source file cleanup after successful placement
//!
//! # Example
//!
//! ```ignore
//! use torrentino_core::placer::{FsPlacer, Placer, PlacementJob, FilePlacement};
//!
//! let placer = FsPlacer::with_defaults();
//!
//! let job = PlacementJob {
//!     job_id: "job-1".to_string(),
//!     files: vec![
//!         FilePlacement {
//!             item_id: "track-1".to_string(),
//!             source: PathBuf::from("/tmp/converted/track01.ogg"),
//!             destination: PathBuf::from("/music/album/01-song.ogg"),
//!             overwrite: false,
//!             verify_checksum: None,
//!         },
//!     ],
//!     atomic: true,
//!     cleanup_sources: true,
//!     enable_rollback: true,
//! };
//!
//! let result = placer.place(job).await?;
//! println!("Placed {} files ({} bytes)", result.files_placed.len(), result.total_bytes);
//! ```

mod config;
mod error;
mod fs_placer;
mod traits;
mod types;

pub use config::PlacerConfig;
pub use error::PlacerError;
pub use fs_placer::FsPlacer;
pub use traits::Placer;
pub use types::{
    ChecksumType, FilePlacement, PlacedFile, PlacementJob, PlacementProgress, PlacementResult,
    RollbackFile, RollbackPlan, RollbackResult,
};
