//! Trait definitions for the placer module.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::error::PlacerError;
use super::types::{PlacementJob, PlacementProgress, PlacementResult, RollbackPlan, RollbackResult};

/// A placer that can move files to their final destinations.
#[async_trait]
pub trait Placer: Send + Sync {
    /// Returns the name of this placer implementation.
    fn name(&self) -> &str;

    /// Places files according to the job specification.
    async fn place(&self, job: PlacementJob) -> Result<PlacementResult, PlacerError>;

    /// Places files with progress reporting.
    ///
    /// The progress sender will receive updates during placement.
    /// If the sender is dropped, placement continues without progress reporting.
    async fn place_with_progress(
        &self,
        job: PlacementJob,
        progress_tx: mpsc::Sender<PlacementProgress>,
    ) -> Result<PlacementResult, PlacerError>;

    /// Rolls back a failed placement using the rollback plan.
    async fn rollback(&self, plan: RollbackPlan) -> RollbackResult;

    /// Validates that the placer is properly configured and ready.
    async fn validate(&self) -> Result<(), PlacerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct MockPlacer;

    #[async_trait]
    impl Placer for MockPlacer {
        fn name(&self) -> &str {
            "mock"
        }

        async fn place(&self, job: PlacementJob) -> Result<PlacementResult, PlacerError> {
            Ok(PlacementResult {
                job_id: job.job_id,
                files_placed: vec![],
                total_bytes: 0,
                duration_ms: 100,
            })
        }

        async fn place_with_progress(
            &self,
            job: PlacementJob,
            _progress_tx: mpsc::Sender<PlacementProgress>,
        ) -> Result<PlacementResult, PlacerError> {
            self.place(job).await
        }

        async fn rollback(&self, plan: RollbackPlan) -> RollbackResult {
            RollbackResult {
                job_id: plan.job_id,
                files_removed: 0,
                directories_removed: 0,
                errors: vec![],
                success: true,
            }
        }

        async fn validate(&self) -> Result<(), PlacerError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mock_placer() {
        let placer = MockPlacer;
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![],
            atomic: true,
            cleanup_sources: false,
            enable_rollback: true,
        };

        let result = placer.place(job).await.unwrap();
        assert_eq!(result.job_id, "test-job");
    }
}
