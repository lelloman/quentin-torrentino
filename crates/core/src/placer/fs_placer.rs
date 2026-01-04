//! File system placer implementation.

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc;

use super::config::PlacerConfig;
use super::error::PlacerError;
use super::traits::Placer;
use super::types::{
    ChecksumType, FilePlacement, PlacedFile, PlacementJob, PlacementProgress, PlacementResult,
    RollbackPlan, RollbackResult,
};

/// File system based placer implementation.
pub struct FsPlacer {
    config: PlacerConfig,
}

impl FsPlacer {
    /// Creates a new file system placer with the given configuration.
    pub fn new(config: PlacerConfig) -> Self {
        Self { config }
    }

    /// Creates a placer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(PlacerConfig::default())
    }

    /// Attempts to move a file atomically (rename).
    async fn try_atomic_move(source: &Path, destination: &Path) -> Result<bool, std::io::Error> {
        match fs::rename(source, destination).await {
            Ok(()) => Ok(true),
            Err(e) => {
                // Cross-filesystem moves fail with EXDEV (18 on Linux)
                // We check for CrossesDevices error kind or the raw EXDEV code
                if e.kind() == std::io::ErrorKind::CrossesDevices || e.raw_os_error() == Some(18) {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Copies a file with optional checksum calculation.
    async fn copy_file(
        &self,
        source: &Path,
        destination: &Path,
        calculate_checksum: bool,
    ) -> Result<(u64, Option<String>), PlacerError> {
        let source_file = File::open(source).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PlacerError::SourceNotFound {
                    path: source.to_path_buf(),
                }
            } else {
                PlacerError::Io(e)
            }
        })?;

        let dest_file = File::create(destination).await.map_err(|e| {
            PlacerError::copy_failed(source.to_path_buf(), destination.to_path_buf(), e)
        })?;

        let mut reader = BufReader::with_capacity(self.config.buffer_size, source_file);
        let mut writer = BufWriter::with_capacity(self.config.buffer_size, dest_file);

        let mut hasher = if calculate_checksum {
            Some(Sha256::new())
        } else {
            None
        };

        let mut total_bytes = 0u64;
        let mut buffer = vec![0u8; self.config.buffer_size];

        loop {
            let bytes_read = reader.read(&mut buffer).await.map_err(|e| {
                PlacerError::copy_failed(source.to_path_buf(), destination.to_path_buf(), e)
            })?;

            if bytes_read == 0 {
                break;
            }

            if let Some(ref mut h) = hasher {
                h.update(&buffer[..bytes_read]);
            }

            writer.write_all(&buffer[..bytes_read]).await.map_err(|e| {
                PlacerError::copy_failed(source.to_path_buf(), destination.to_path_buf(), e)
            })?;

            total_bytes += bytes_read as u64;
        }

        writer.flush().await.map_err(|e| {
            PlacerError::copy_failed(source.to_path_buf(), destination.to_path_buf(), e)
        })?;

        let checksum = hasher.map(|h| format!("{:x}", h.finalize()));

        Ok((total_bytes, checksum))
    }

    /// Calculates the checksum of a file using the specified algorithm.
    async fn calculate_checksum(
        &self,
        path: &Path,
        checksum_type: ChecksumType,
    ) -> Result<String, PlacerError> {
        let file = File::open(path)
            .await
            .map_err(|e| PlacerError::ChecksumCalculationFailed {
                path: path.to_path_buf(),
                source: e,
            })?;

        let mut reader = BufReader::with_capacity(self.config.buffer_size, file);
        let mut buffer = vec![0u8; self.config.buffer_size];

        match checksum_type {
            ChecksumType::Sha256 => {
                let mut hasher = Sha256::new();
                loop {
                    let bytes_read = reader.read(&mut buffer).await.map_err(|e| {
                        PlacerError::ChecksumCalculationFailed {
                            path: path.to_path_buf(),
                            source: e,
                        }
                    })?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes_read]);
                }
                Ok(format!("{:x}", hasher.finalize()))
            }
            ChecksumType::Md5 => {
                let mut context = md5::Context::new();
                loop {
                    let bytes_read = reader.read(&mut buffer).await.map_err(|e| {
                        PlacerError::ChecksumCalculationFailed {
                            path: path.to_path_buf(),
                            source: e,
                        }
                    })?;
                    if bytes_read == 0 {
                        break;
                    }
                    context.consume(&buffer[..bytes_read]);
                }
                Ok(format!("{:x}", context.compute()))
            }
        }
    }

    /// Creates parent directories for a path.
    async fn ensure_parent_dirs(
        &self,
        path: &Path,
        plan: &mut RollbackPlan,
    ) -> Result<(), PlacerError> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                // Track which directories we create for rollback
                let mut dirs_to_create = Vec::new();
                let mut current = parent;

                while !current.exists() {
                    dirs_to_create.push(current.to_path_buf());
                    current = match current.parent() {
                        Some(p) => p,
                        None => break,
                    };
                }

                // Create directories from parent to child
                fs::create_dir_all(parent).await.map_err(|e| {
                    PlacerError::DirectoryCreationFailed {
                        path: parent.to_path_buf(),
                        source: e,
                    }
                })?;

                // Record created directories for rollback (in reverse order for cleanup)
                for dir in dirs_to_create.into_iter().rev() {
                    plan.record_directory(dir);
                }
            }
        }
        Ok(())
    }

    /// Places a single file.
    async fn place_file(
        &self,
        placement: &FilePlacement,
        plan: &mut RollbackPlan,
        keep_source: bool,
    ) -> Result<PlacedFile, PlacerError> {
        // Check source exists
        if !placement.source.exists() {
            return Err(PlacerError::SourceNotFound {
                path: placement.source.clone(),
            });
        }

        // Check destination doesn't exist (unless overwrite)
        if placement.destination.exists() && !placement.overwrite {
            return Err(PlacerError::DestinationExists {
                path: placement.destination.clone(),
            });
        }

        // Create parent directories
        self.ensure_parent_dirs(&placement.destination, plan)
            .await?;

        // Try atomic move first if preferred and we're not keeping source
        let (size_bytes, checksum) = if self.config.prefer_atomic_moves && !keep_source {
            if Self::try_atomic_move(&placement.source, &placement.destination).await? {
                // Atomic move succeeded
                let meta = fs::metadata(&placement.destination).await?;
                let checksum = if let Some(ct) = placement.verify_checksum {
                    Some(self.calculate_checksum(&placement.destination, ct).await?)
                } else {
                    None
                };
                (meta.len(), checksum)
            } else {
                // Fall back to copy
                let (size, cs) = self
                    .copy_file(
                        &placement.source,
                        &placement.destination,
                        placement.verify_checksum.is_some(),
                    )
                    .await?;
                (size, cs)
            }
        } else {
            // Copy the file
            let (size, cs) = self
                .copy_file(
                    &placement.source,
                    &placement.destination,
                    placement.verify_checksum.is_some(),
                )
                .await?;
            (size, cs)
        };

        // Record for rollback
        plan.record_placement(
            placement.destination.clone(),
            Some(placement.source.clone()),
            size_bytes,
        );

        Ok(PlacedFile {
            item_id: placement.item_id.clone(),
            destination: placement.destination.clone(),
            size_bytes,
            checksum,
        })
    }

    /// Runs the placement with optional progress reporting.
    async fn run_placement(
        &self,
        job: PlacementJob,
        progress_tx: Option<mpsc::Sender<PlacementProgress>>,
    ) -> Result<PlacementResult, PlacerError> {
        let start = Instant::now();
        let mut placed_files = Vec::new();
        let mut bytes_copied = 0u64;
        let mut rollback_plan = RollbackPlan::new(job.job_id.clone());

        let total_files = job.files.len();
        let keep_sources = !job.cleanup_sources;

        // Pre-calculate total bytes for progress reporting
        let total_bytes = if progress_tx.is_some() {
            let mut sum = 0u64;
            for placement in &job.files {
                if let Ok(meta) = tokio::fs::metadata(&placement.source).await {
                    sum += meta.len();
                }
            }
            sum
        } else {
            0
        };

        for (idx, placement) in job.files.iter().enumerate() {
            // Send progress update
            if let Some(ref tx) = progress_tx {
                let progress = PlacementProgress {
                    job_id: job.job_id.clone(),
                    files_placed: idx,
                    total_files,
                    current_file: placement
                        .source
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    bytes_copied,
                    total_bytes,
                };
                let _ = tx.try_send(progress);
            }

            // Place the file
            match self
                .place_file(placement, &mut rollback_plan, keep_sources)
                .await
            {
                Ok(placed) => {
                    bytes_copied += placed.size_bytes;
                    placed_files.push(placed);
                }
                Err(e) => {
                    // Rollback if enabled and we have changes
                    if job.enable_rollback && rollback_plan.has_changes() {
                        let rollback_result = self.rollback(rollback_plan).await;
                        if !rollback_result.success {
                            return Err(PlacerError::RollbackFailed {
                                reason: rollback_result.errors.join(", "),
                            });
                        }
                    }
                    return Err(e);
                }
            }
        }

        // Cleanup sources if requested
        if job.cleanup_sources {
            for placement in &job.files {
                if placement.source.exists() {
                    if let Err(e) = fs::remove_file(&placement.source).await {
                        // Log but don't fail - files are already placed
                        tracing::warn!(
                            "Failed to cleanup source file {}: {}",
                            placement.source.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(PlacementResult {
            job_id: job.job_id,
            files_placed: placed_files,
            total_bytes: bytes_copied,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[async_trait]
impl Placer for FsPlacer {
    fn name(&self) -> &str {
        "fs"
    }

    async fn place(&self, job: PlacementJob) -> Result<PlacementResult, PlacerError> {
        self.run_placement(job, None).await
    }

    async fn place_with_progress(
        &self,
        job: PlacementJob,
        progress_tx: mpsc::Sender<PlacementProgress>,
    ) -> Result<PlacementResult, PlacerError> {
        self.run_placement(job, Some(progress_tx)).await
    }

    async fn rollback(&self, plan: RollbackPlan) -> RollbackResult {
        let mut files_removed = 0;
        let mut directories_removed = 0;
        let mut errors = Vec::new();

        // Remove placed files (in reverse order)
        for file in plan.placed_files.iter().rev() {
            if file.destination.exists() {
                match fs::remove_file(&file.destination).await {
                    Ok(()) => files_removed += 1,
                    Err(e) => errors.push(format!(
                        "Failed to remove {}: {}",
                        file.destination.display(),
                        e
                    )),
                }
            }
        }

        // Remove created directories (in reverse order - child first)
        // Only remove empty directories
        let mut attempted_dirs: HashSet<PathBuf> = HashSet::new();
        for dir in plan.created_directories.iter().rev() {
            if attempted_dirs.contains(dir) {
                continue;
            }
            attempted_dirs.insert(dir.clone());

            if dir.exists() {
                // Check if directory is empty
                match fs::read_dir(dir).await {
                    Ok(mut entries) => {
                        match entries.next_entry().await {
                            Ok(None) => {
                                // Directory is empty, safe to remove
                                match fs::remove_dir(dir).await {
                                    Ok(()) => directories_removed += 1,
                                    Err(e) => errors.push(format!(
                                        "Failed to remove directory {}: {}",
                                        dir.display(),
                                        e
                                    )),
                                }
                            }
                            Ok(Some(_)) => {
                                // Directory not empty, skip
                            }
                            Err(e) => errors.push(format!(
                                "Failed to check directory {}: {}",
                                dir.display(),
                                e
                            )),
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Failed to read directory {}: {}", dir.display(), e))
                    }
                }
            }
        }

        RollbackResult {
            job_id: plan.job_id,
            files_removed,
            directories_removed,
            errors: errors.clone(),
            success: errors.is_empty(),
        }
    }

    async fn validate(&self) -> Result<(), PlacerError> {
        // Basic validation - nothing specific needed for fs placer
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_place_single_file() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("source.txt");
        let dest_path = temp.path().join("dest/subdir/output.txt");

        // Create source file
        fs::write(&source_path, "test content").await.unwrap();

        let placer = FsPlacer::with_defaults();
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![FilePlacement {
                item_id: "item-1".to_string(),
                source: source_path.clone(),
                destination: dest_path.clone(),
                overwrite: false,
                verify_checksum: None,
            }],
            atomic: true,
            cleanup_sources: false,
            enable_rollback: true,
        };

        let result = placer.place(job).await.unwrap();
        assert_eq!(result.files_placed.len(), 1);
        assert!(dest_path.exists());

        // Source should still exist (cleanup_sources = false)
        assert!(source_path.exists());
    }

    #[tokio::test]
    async fn test_place_with_cleanup() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("source.txt");
        let dest_path = temp.path().join("output.txt");

        fs::write(&source_path, "test content").await.unwrap();

        let placer = FsPlacer::new(PlacerConfig::default().with_atomic_moves(false));
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![FilePlacement {
                item_id: "item-1".to_string(),
                source: source_path.clone(),
                destination: dest_path.clone(),
                overwrite: false,
                verify_checksum: None,
            }],
            atomic: false,
            cleanup_sources: true,
            enable_rollback: true,
        };

        let result = placer.place(job).await.unwrap();
        assert_eq!(result.files_placed.len(), 1);
        assert!(dest_path.exists());

        // Source should be cleaned up
        assert!(!source_path.exists());
    }

    #[tokio::test]
    async fn test_rollback_on_failure() {
        let temp = TempDir::new().unwrap();
        let source1 = temp.path().join("source1.txt");
        let source2 = temp.path().join("source2.txt"); // This won't exist
        let dest1 = temp.path().join("dest/output1.txt");
        let dest2 = temp.path().join("dest/output2.txt");

        // Only create first source
        fs::write(&source1, "content 1").await.unwrap();

        let placer = FsPlacer::new(PlacerConfig::default().with_atomic_moves(false));
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![
                FilePlacement {
                    item_id: "item-1".to_string(),
                    source: source1.clone(),
                    destination: dest1.clone(),
                    overwrite: false,
                    verify_checksum: None,
                },
                FilePlacement {
                    item_id: "item-2".to_string(),
                    source: source2.clone(), // Will fail
                    destination: dest2.clone(),
                    overwrite: false,
                    verify_checksum: None,
                },
            ],
            atomic: false,
            cleanup_sources: false,
            enable_rollback: true,
        };

        let result = placer.place(job).await;
        assert!(result.is_err());

        // First file should be rolled back
        assert!(!dest1.exists());
        // Directory should be rolled back too (if empty)
        assert!(!temp.path().join("dest").exists());
    }

    #[tokio::test]
    async fn test_destination_exists_error() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("source.txt");
        let dest_path = temp.path().join("output.txt");

        fs::write(&source_path, "source content").await.unwrap();
        fs::write(&dest_path, "existing content").await.unwrap();

        let placer = FsPlacer::with_defaults();
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![FilePlacement {
                item_id: "item-1".to_string(),
                source: source_path,
                destination: dest_path.clone(),
                overwrite: false, // Don't overwrite
                verify_checksum: None,
            }],
            atomic: true,
            cleanup_sources: false,
            enable_rollback: true,
        };

        let result = placer.place(job).await;
        assert!(matches!(result, Err(PlacerError::DestinationExists { .. })));
    }

    #[tokio::test]
    async fn test_place_with_overwrite() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("source.txt");
        let dest_path = temp.path().join("output.txt");

        fs::write(&source_path, "new content").await.unwrap();
        fs::write(&dest_path, "old content").await.unwrap();

        let placer = FsPlacer::new(PlacerConfig::default().with_atomic_moves(false));
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![FilePlacement {
                item_id: "item-1".to_string(),
                source: source_path,
                destination: dest_path.clone(),
                overwrite: true, // Allow overwrite
                verify_checksum: None,
            }],
            atomic: false,
            cleanup_sources: false,
            enable_rollback: true,
        };

        let result = placer.place(job).await.unwrap();
        assert_eq!(result.files_placed.len(), 1);

        let content = fs::read_to_string(&dest_path).await.unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_checksum_verification() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("source.txt");
        let dest_path = temp.path().join("output.txt");

        fs::write(&source_path, "test content for checksum")
            .await
            .unwrap();

        let placer = FsPlacer::new(PlacerConfig::default().with_atomic_moves(false));
        let job = PlacementJob {
            job_id: "test-job".to_string(),
            files: vec![FilePlacement {
                item_id: "item-1".to_string(),
                source: source_path,
                destination: dest_path,
                overwrite: false,
                verify_checksum: Some(ChecksumType::Sha256),
            }],
            atomic: false,
            cleanup_sources: false,
            enable_rollback: true,
        };

        let result = placer.place(job).await.unwrap();
        assert!(result.files_placed[0].checksum.is_some());
    }
}
