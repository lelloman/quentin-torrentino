//! Mock placer for testing.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use crate::placer::{
    PlacedFile, PlacementJob, PlacementProgress, PlacementResult, Placer, PlacerError,
    RollbackPlan, RollbackResult,
};

/// A recorded placement job for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedPlacement {
    /// The job that was submitted.
    pub job: PlacementJob,
    /// Whether the placement succeeded.
    pub success: bool,
}

/// A recorded rollback for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedRollback {
    /// The rollback plan that was executed.
    pub plan: RollbackPlan,
    /// Whether the rollback succeeded.
    pub success: bool,
}

/// Mock implementation of the Placer trait.
///
/// Provides controllable behavior for testing:
/// - Track placement jobs for assertions
/// - Simulate success/failure
/// - Track rollbacks
/// - Simulate progress updates
///
/// # Example
///
/// ```rust,ignore
/// use torrentino_core::testing::MockPlacer;
///
/// let placer = MockPlacer::new();
///
/// // Place files
/// let result = placer.place(job).await?;
///
/// // Check what was placed
/// let placements = placer.recorded_placements().await;
/// assert_eq!(placements.len(), 1);
/// assert!(placements[0].success);
/// ```
#[derive(Debug)]
pub struct MockPlacer {
    /// Recorded placements.
    placements: Arc<RwLock<Vec<RecordedPlacement>>>,
    /// Recorded rollbacks.
    rollbacks: Arc<RwLock<Vec<RecordedRollback>>>,
    /// If set, the next operation will fail with this error.
    next_error: Arc<RwLock<Option<PlacerError>>>,
    /// Simulated placement duration in milliseconds.
    placement_duration_ms: Arc<RwLock<u64>>,
    /// Whether to send progress updates during placement.
    send_progress: Arc<RwLock<bool>>,
    /// Whether rollback should succeed.
    rollback_success: Arc<RwLock<bool>>,
}

impl Default for MockPlacer {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPlacer {
    /// Create a new mock placer.
    pub fn new() -> Self {
        Self {
            placements: Arc::new(RwLock::new(Vec::new())),
            rollbacks: Arc::new(RwLock::new(Vec::new())),
            next_error: Arc::new(RwLock::new(None)),
            placement_duration_ms: Arc::new(RwLock::new(50)),
            send_progress: Arc::new(RwLock::new(true)),
            rollback_success: Arc::new(RwLock::new(true)),
        }
    }

    /// Get all recorded placements.
    pub async fn recorded_placements(&self) -> Vec<RecordedPlacement> {
        self.placements.read().await.clone()
    }

    /// Clear recorded placements.
    pub async fn clear_recorded_placements(&self) {
        self.placements.write().await.clear();
    }

    /// Get the number of placements performed.
    pub async fn placement_count(&self) -> usize {
        self.placements.read().await.len()
    }

    /// Get all recorded rollbacks.
    pub async fn recorded_rollbacks(&self) -> Vec<RecordedRollback> {
        self.rollbacks.read().await.clone()
    }

    /// Clear recorded rollbacks.
    pub async fn clear_recorded_rollbacks(&self) {
        self.rollbacks.write().await.clear();
    }

    /// Get the number of rollbacks performed.
    pub async fn rollback_count(&self) -> usize {
        self.rollbacks.read().await.len()
    }

    /// Configure the next operation to fail with the given error.
    pub async fn set_next_error(&self, error: PlacerError) {
        *self.next_error.write().await = Some(error);
    }

    /// Clear any pending error.
    pub async fn clear_next_error(&self) {
        *self.next_error.write().await = None;
    }

    /// Set the simulated placement duration.
    pub async fn set_placement_duration(&self, duration: Duration) {
        *self.placement_duration_ms.write().await = duration.as_millis() as u64;
    }

    /// Enable or disable progress updates during placement.
    pub async fn set_send_progress(&self, send: bool) {
        *self.send_progress.write().await = send;
    }

    /// Set whether rollback should succeed.
    pub async fn set_rollback_success(&self, success: bool) {
        *self.rollback_success.write().await = success;
    }

    /// Take the next error if set.
    async fn take_error(&self) -> Option<PlacerError> {
        self.next_error.write().await.take()
    }
}

#[async_trait]
impl Placer for MockPlacer {
    fn name(&self) -> &str {
        "mock"
    }

    async fn place(&self, job: PlacementJob) -> Result<PlacementResult, PlacerError> {
        if let Some(err) = self.take_error().await {
            self.placements.write().await.push(RecordedPlacement {
                job,
                success: false,
            });
            return Err(err);
        }

        // Calculate totals
        let total_bytes: u64 = job.files.iter().map(|_| 50 * 1024 * 1024).sum(); // 50 MB per file

        // Simulate placement time
        let duration_ms = *self.placement_duration_ms.read().await;
        if duration_ms > 0 {
            tokio::time::sleep(Duration::from_millis(duration_ms)).await;
        }

        // Create placed files
        let files_placed: Vec<PlacedFile> = job
            .files
            .iter()
            .map(|f| PlacedFile {
                item_id: f.item_id.clone(),
                destination: f.destination.clone(),
                size_bytes: 50 * 1024 * 1024,
                checksum: f.verify_checksum.map(|_| "mock-checksum-abc123".to_string()),
            })
            .collect();

        // Record the placement
        self.placements.write().await.push(RecordedPlacement {
            job: job.clone(),
            success: true,
        });

        Ok(PlacementResult {
            job_id: job.job_id,
            files_placed,
            total_bytes,
            duration_ms,
        })
    }

    async fn place_with_progress(
        &self,
        job: PlacementJob,
        progress_tx: mpsc::Sender<PlacementProgress>,
    ) -> Result<PlacementResult, PlacerError> {
        let send_progress = *self.send_progress.read().await;
        let duration_ms = *self.placement_duration_ms.read().await;

        if send_progress && !job.files.is_empty() && duration_ms > 0 {
            let job_id = job.job_id.clone();
            let total_files = job.files.len();
            let total_bytes = (total_files as u64) * 50 * 1024 * 1024;

            let step_duration = duration_ms / (total_files as u64).max(1);

            for (i, file) in job.files.iter().enumerate() {
                let _ = progress_tx
                    .send(PlacementProgress {
                        job_id: job_id.clone(),
                        files_placed: i,
                        total_files,
                        current_file: file.destination.to_string_lossy().to_string(),
                        bytes_copied: (i as u64) * 50 * 1024 * 1024,
                        total_bytes,
                    })
                    .await;

                tokio::time::sleep(Duration::from_millis(step_duration)).await;
            }

            // Final progress
            let _ = progress_tx
                .send(PlacementProgress {
                    job_id: job_id.clone(),
                    files_placed: total_files,
                    total_files,
                    current_file: "complete".to_string(),
                    bytes_copied: total_bytes,
                    total_bytes,
                })
                .await;
        }

        self.place(job).await
    }

    async fn rollback(&self, plan: RollbackPlan) -> RollbackResult {
        let success = *self.rollback_success.read().await;

        let files_removed = if success { plan.placed_files.len() } else { 0 };
        let directories_removed = if success {
            plan.created_directories.len()
        } else {
            0
        };

        // Record the rollback
        self.rollbacks.write().await.push(RecordedRollback {
            plan: plan.clone(),
            success,
        });

        RollbackResult {
            job_id: plan.job_id,
            files_removed,
            directories_removed,
            errors: if success {
                vec![]
            } else {
                vec!["Simulated rollback failure".to_string()]
            },
            success,
        }
    }

    async fn validate(&self) -> Result<(), PlacerError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placer::FilePlacement;
    use std::path::PathBuf;

    fn create_test_job(id: &str, file_count: usize) -> PlacementJob {
        PlacementJob {
            job_id: id.to_string(),
            files: (0..file_count)
                .map(|i| FilePlacement {
                    item_id: format!("file-{}", i),
                    source: PathBuf::from(format!("/source/file{}.mp3", i)),
                    destination: PathBuf::from(format!("/dest/file{}.mp3", i)),
                    overwrite: false,
                    verify_checksum: None,
                })
                .collect(),
            atomic: true,
            cleanup_sources: false,
            enable_rollback: true,
        }
    }

    #[tokio::test]
    async fn test_basic_placement() {
        let placer = MockPlacer::new();
        placer.set_placement_duration(Duration::ZERO).await;

        let job = create_test_job("test-1", 3);
        let result = placer.place(job).await.unwrap();

        assert_eq!(result.job_id, "test-1");
        assert_eq!(result.files_placed.len(), 3);
    }

    #[tokio::test]
    async fn test_recorded_placements() {
        let placer = MockPlacer::new();
        placer.set_placement_duration(Duration::ZERO).await;

        placer.place(create_test_job("job-1", 2)).await.unwrap();
        placer.place(create_test_job("job-2", 1)).await.unwrap();

        let placements = placer.recorded_placements().await;
        assert_eq!(placements.len(), 2);
        assert!(placements[0].success);
        assert_eq!(placements[0].job.job_id, "job-1");
        assert_eq!(placements[0].job.files.len(), 2);
    }

    #[tokio::test]
    async fn test_error_injection() {
        let placer = MockPlacer::new();
        placer
            .set_next_error(PlacerError::PartialFailure {
                files_placed: 0,
                reason: "test error".to_string(),
            })
            .await;

        let result = placer.place(create_test_job("fail", 1)).await;
        assert!(result.is_err());

        // Error should be consumed, placement recorded as failed
        let placements = placer.recorded_placements().await;
        assert_eq!(placements.len(), 1);
        assert!(!placements[0].success);
    }

    #[tokio::test]
    async fn test_rollback() {
        let placer = MockPlacer::new();

        let plan = RollbackPlan {
            job_id: "rollback-test".to_string(),
            placed_files: vec![],
            created_directories: vec![PathBuf::from("/test/dir")],
        };

        let result = placer.rollback(plan).await;
        assert!(result.success);
        assert_eq!(result.directories_removed, 1);

        let rollbacks = placer.recorded_rollbacks().await;
        assert_eq!(rollbacks.len(), 1);
        assert!(rollbacks[0].success);
    }

    #[tokio::test]
    async fn test_rollback_failure() {
        let placer = MockPlacer::new();
        placer.set_rollback_success(false).await;

        let plan = RollbackPlan {
            job_id: "fail-rollback".to_string(),
            placed_files: vec![],
            created_directories: vec![],
        };

        let result = placer.rollback(plan).await;
        assert!(!result.success);
        assert!(!result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_progress_updates() {
        let placer = MockPlacer::new();
        placer.set_placement_duration(Duration::from_millis(30)).await;

        let (tx, mut rx) = mpsc::channel(10);

        let job = create_test_job("progress-test", 3);
        tokio::spawn(async move {
            placer.place_with_progress(job, tx).await.unwrap();
        });

        let mut progress_count = 0;
        while rx.recv().await.is_some() {
            progress_count += 1;
        }

        // Should have progress for each file + final
        assert!(progress_count >= 3);
    }
}
