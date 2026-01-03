//! Ticket orchestrator for automated pipeline processing.
//!
//! The orchestrator drives tickets through the state machine automatically:
//! - **Acquisition**: Sequential (one ticket at a time) - CPU-bound
//! - **Download**: Concurrent monitoring (many downloads) - IO-bound
//! - **Pipeline**: Sequential (one ticket at a time) - CPU-bound (handled by PipelineProcessor)

mod config;
mod runner;
mod types;

pub use config::OrchestratorConfig;
pub use runner::{TicketOrchestrator, TicketUpdateCallback};
pub use types::{ActiveDownload, OrchestratorError, OrchestratorStatus};
