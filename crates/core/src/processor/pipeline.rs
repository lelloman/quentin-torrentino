//! Pipeline processor implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock, Semaphore};

use crate::audit::{AuditEvent, AuditHandle};
use crate::converter::{ConversionConstraints, ConversionJob, ConversionProgress, Converter, EmbeddedMetadata};
use crate::placer::{FilePlacement, PlacementJob, Placer};
use crate::ticket::{CompletionStats, TicketState, TicketStore};

use super::config::ProcessorConfig;
use super::types::{
    PipelineJob, PipelineProgress, PipelineResult, PlacedFileInfo, PoolStatus, PipelineStatus,
};

/// Callback type for pipeline update notifications.
/// Called with (ticket_id, state_type) when ticket state changes.
pub type PipelineUpdateCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Callback type for pipeline progress notifications.
/// Called with (ticket_id, phase, current, total, current_name, percent) for real-time progress.
/// The percent field (0.0-100.0) shows intra-file progress from FFmpeg.
pub type PipelineProgressCallback =
    Arc<dyn Fn(&str, &str, usize, usize, &str, f32) + Send + Sync>;

/// Error type for pipeline operations.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Conversion failed.
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),

    /// Placement failed.
    #[error("Placement failed: {0}")]
    PlacementFailed(String),

    /// Pipeline is not running.
    #[error("Pipeline is not running")]
    NotRunning,

    /// Job already exists.
    #[error("Job already exists for ticket: {0}")]
    JobExists(String),

    /// Job not found.
    #[error("Job not found: {0}")]
    JobNotFound(String),
}

/// Tracks statistics for a processing pool.
struct PoolStats {
    active: AtomicU64,
    queued: AtomicU64,
    total_processed: AtomicU64,
    total_failed: AtomicU64,
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            active: AtomicU64::new(0),
            queued: AtomicU64::new(0),
            total_processed: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
        }
    }
}

impl PoolStats {
    fn to_status(&self, name: &str, max_concurrent: usize) -> PoolStatus {
        PoolStatus {
            name: name.to_string(),
            active_jobs: self.active.load(Ordering::Relaxed) as usize,
            max_concurrent,
            queued_jobs: self.queued.load(Ordering::Relaxed) as usize,
            total_processed: self.total_processed.load(Ordering::Relaxed),
            total_failed: self.total_failed.load(Ordering::Relaxed),
        }
    }
}

/// The main pipeline processor.
pub struct PipelineProcessor<C: Converter, P: Placer> {
    config: ProcessorConfig,
    converter: Arc<C>,
    placer: Arc<P>,
    audit: Option<AuditHandle>,
    ticket_store: Option<Arc<dyn TicketStore>>,
    on_update: Option<PipelineUpdateCallback>,
    on_progress: Option<PipelineProgressCallback>,
    conversion_semaphore: Arc<Semaphore>,
    placement_semaphore: Arc<Semaphore>,
    conversion_stats: Arc<PoolStats>,
    placement_stats: Arc<PoolStats>,
    active_jobs: Arc<RwLock<HashMap<String, JobState>>>,
    running: Arc<RwLock<bool>>,
}

/// State of an active job.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for tracking/debugging
enum JobState {
    Converting {
        started_at: Instant,
        current_file: usize,
        total_files: usize,
    },
    Placing {
        started_at: Instant,
        files_placed: usize,
        total_files: usize,
    },
}

impl<C: Converter + 'static, P: Placer + 'static> PipelineProcessor<C, P> {
    /// Creates a new pipeline processor.
    pub fn new(config: ProcessorConfig, converter: C, placer: P) -> Self {
        let conversion_semaphore = Arc::new(Semaphore::new(config.max_parallel_conversions));
        let placement_semaphore = Arc::new(Semaphore::new(config.max_parallel_placements));

        Self {
            config,
            converter: Arc::new(converter),
            placer: Arc::new(placer),
            audit: None,
            ticket_store: None,
            on_update: None,
            on_progress: None,
            conversion_semaphore,
            placement_semaphore,
            conversion_stats: Arc::new(PoolStats::default()),
            placement_stats: Arc::new(PoolStats::default()),
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Sets the audit handle for logging events.
    pub fn with_audit(mut self, audit: AuditHandle) -> Self {
        self.audit = Some(audit);
        self
    }

    /// Sets the ticket store for updating ticket state.
    pub fn with_ticket_store(mut self, store: Arc<dyn TicketStore>) -> Self {
        self.ticket_store = Some(store);
        self
    }

    /// Sets the update callback for WebSocket notifications.
    pub fn with_update_callback(mut self, callback: PipelineUpdateCallback) -> Self {
        self.on_update = Some(callback);
        self
    }

    /// Sets the progress callback for real-time progress updates.
    pub fn with_progress_callback(mut self, callback: PipelineProgressCallback) -> Self {
        self.on_progress = Some(callback);
        self
    }

    /// Starts the pipeline processor.
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        *running = true;
    }

    /// Stops the pipeline processor.
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Returns the current pipeline status.
    pub async fn status(&self) -> PipelineStatus {
        let jobs = self.active_jobs.read().await;
        let running = *self.running.read().await;

        let mut converting_tickets = Vec::new();
        let mut placing_tickets = Vec::new();

        for (ticket_id, state) in jobs.iter() {
            match state {
                JobState::Converting { .. } => converting_tickets.push(ticket_id.clone()),
                JobState::Placing { .. } => placing_tickets.push(ticket_id.clone()),
            }
        }

        PipelineStatus {
            running,
            conversion_pool: self
                .conversion_stats
                .to_status("conversion", self.config.max_parallel_conversions),
            placement_pool: self
                .placement_stats
                .to_status("placement", self.config.max_parallel_placements),
            converting_tickets,
            placing_tickets,
        }
    }

    /// Processes a job through the pipeline.
    ///
    /// Returns immediately, processing happens in the background.
    /// Use the progress channel to monitor progress.
    pub async fn process(
        &self,
        job: PipelineJob,
        progress_tx: Option<mpsc::Sender<PipelineProgress>>,
    ) -> Result<(), PipelineError> {
        let running = *self.running.read().await;
        if !running {
            return Err(PipelineError::NotRunning);
        }

        // Check if job already exists
        {
            let jobs = self.active_jobs.read().await;
            if jobs.contains_key(&job.ticket_id) {
                return Err(PipelineError::JobExists(job.ticket_id.clone()));
            }
        }

        // Start processing in background
        let ticket_id = job.ticket_id.clone();
        let converter = Arc::clone(&self.converter);
        let placer = Arc::clone(&self.placer);
        let config = self.config.clone();
        let audit = self.audit.clone();
        let ticket_store = self.ticket_store.clone();
        let on_update = self.on_update.clone();
        let on_progress = self.on_progress.clone();
        let conversion_semaphore = Arc::clone(&self.conversion_semaphore);
        let placement_semaphore = Arc::clone(&self.placement_semaphore);
        let conversion_stats = Arc::clone(&self.conversion_stats);
        let placement_stats = Arc::clone(&self.placement_stats);
        let active_jobs = Arc::clone(&self.active_jobs);

        tokio::spawn(async move {
            let result = Self::run_pipeline(
                job,
                converter,
                placer,
                config,
                audit.clone(),
                ticket_store.clone(),
                on_update,
                on_progress,
                conversion_semaphore,
                placement_semaphore,
                conversion_stats,
                placement_stats,
                active_jobs.clone(),
                progress_tx.clone(),
            )
            .await;

            // Remove from active jobs
            {
                let mut jobs = active_jobs.write().await;
                jobs.remove(&ticket_id);
            }

            // Send final progress
            if let Some(tx) = progress_tx {
                let progress = match &result {
                    Ok(r) => PipelineProgress::Completed {
                        ticket_id: r.ticket_id.clone(),
                        files_placed: r.files_placed.len(),
                        total_bytes: r.total_bytes,
                    },
                    Err(e) => PipelineProgress::Failed {
                        ticket_id: ticket_id.clone(),
                        error: e.to_string(),
                        failed_phase: match e {
                            PipelineError::ConversionFailed(_) => "conversion".to_string(),
                            PipelineError::PlacementFailed(_) => "placement".to_string(),
                            _ => "unknown".to_string(),
                        },
                    },
                };
                let _ = tx.send(progress).await;
            }
        });

        Ok(())
    }

    /// Runs the full pipeline for a job.
    #[allow(clippy::too_many_arguments)]
    async fn run_pipeline(
        job: PipelineJob,
        converter: Arc<C>,
        placer: Arc<P>,
        config: ProcessorConfig,
        audit: Option<AuditHandle>,
        ticket_store: Option<Arc<dyn TicketStore>>,
        on_update: Option<PipelineUpdateCallback>,
        on_progress: Option<PipelineProgressCallback>,
        conversion_semaphore: Arc<Semaphore>,
        placement_semaphore: Arc<Semaphore>,
        conversion_stats: Arc<PoolStats>,
        placement_stats: Arc<PoolStats>,
        active_jobs: Arc<RwLock<HashMap<String, JobState>>>,
        progress_tx: Option<mpsc::Sender<PipelineProgress>>,
    ) -> Result<PipelineResult, PipelineError> {
        let _start = Instant::now();
        let ticket_id = job.ticket_id.clone();
        let total_files = job.source_files.len();

        // Helper to notify WebSocket clients
        let notify_update = |state_type: &str| {
            if let Some(ref cb) = on_update {
                cb(&ticket_id, state_type);
            }
        };

        // Helper to update ticket state and notify
        let update_ticket_state = |store: &Arc<dyn TicketStore>, state: TicketState| {
            let state_type = state.state_type();
            if let Err(e) = store.update_state(&ticket_id, state) {
                tracing::warn!("Failed to update ticket state for {}: {}", ticket_id, e);
            }
            if let Some(ref cb) = on_update {
                cb(&ticket_id, state_type);
            }
        };

        // Set initial Converting state
        if let Some(ref store) = ticket_store {
            update_ticket_state(
                store,
                TicketState::Converting {
                    started_at: chrono::Utc::now(),
                    current_idx: 0,
                    total: total_files,
                    current_name: "Starting...".to_string(),
                },
            );

            // Emit state change event
            if let Some(ref audit) = audit {
                audit.emit(AuditEvent::TicketStateChanged {
                    ticket_id: ticket_id.clone(),
                    from_state: "downloading".to_string(),
                    to_state: "converting".to_string(),
                    reason: Some(format!("Starting conversion of {} files", total_files)),
                }).await;
            }
        }

        // Emit conversion started event
        if let Some(ref audit) = audit {
            audit.emit(AuditEvent::ConversionStarted {
                ticket_id: ticket_id.clone(),
                job_id: ticket_id.clone(),
                input_path: job.source_files.first().map(|f| f.path.to_string_lossy().to_string()).unwrap_or_default(),
                output_path: job.dest_dir.to_string_lossy().to_string(),
                target_format: format!("{:?}", job.constraints),
                total_files,
            }).await;
        }

        // Register job as converting
        {
            let mut jobs = active_jobs.write().await;
            jobs.insert(
                ticket_id.clone(),
                JobState::Converting {
                    started_at: Instant::now(),
                    current_file: 0,
                    total_files,
                },
            );
        }

        // Phase 1: Conversion
        conversion_stats.queued.fetch_add(1, Ordering::Relaxed);

        let _permit = conversion_semaphore
            .acquire()
            .await
            .map_err(|_| PipelineError::NotRunning)?;
        conversion_stats.queued.fetch_sub(1, Ordering::Relaxed);
        conversion_stats.active.fetch_add(1, Ordering::Relaxed);

        let conversion_start = Instant::now();
        let mut converted_files = Vec::new();
        let temp_dir = config.temp_dir.join(&ticket_id);

        // Create temp directory
        if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
            conversion_stats.active.fetch_sub(1, Ordering::Relaxed);
            conversion_stats.total_failed.fetch_add(1, Ordering::Relaxed);
            return Err(PipelineError::ConversionFailed(format!(
                "Failed to create temp directory: {}",
                e
            )));
        }

        // Check if we need conversion or just copy
        let needs_conversion = job.constraints.is_some();

        if needs_conversion {
            // Run conversion for each file
            let constraints = job.constraints.as_ref().unwrap();

            for (idx, source_file) in job.source_files.iter().enumerate() {
                let current_file_name = source_file
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                // Update job state
                {
                    let mut jobs = active_jobs.write().await;
                    if let Some(state) = jobs.get_mut(&ticket_id) {
                        *state = JobState::Converting {
                            started_at: conversion_start,
                            current_file: idx,
                            total_files,
                        };
                    }
                }

                // Update ticket state
                if let Some(ref store) = ticket_store {
                    update_ticket_state(
                        store,
                        TicketState::Converting {
                            started_at: chrono::Utc::now(),
                            current_idx: idx,
                            total: total_files,
                            current_name: current_file_name.clone(),
                        },
                    );
                }

                // Send progress via channel
                if let Some(ref tx) = progress_tx {
                    let _ = tx
                        .send(PipelineProgress::Converting {
                            ticket_id: ticket_id.clone(),
                            current_file: idx,
                            total_files,
                            current_file_name: current_file_name.clone(),
                            percent: (idx as f32 / total_files as f32) * 100.0,
                        })
                        .await;
                }

                // Send initial progress via callback (for WebSocket) - 0% for this file
                if let Some(ref cb) = on_progress {
                    cb(&ticket_id, "converting", idx, total_files, &current_file_name, 0.0);
                }

                // Build conversion job
                let output_ext = match constraints {
                    ConversionConstraints::Audio(a) => a.format.extension(),
                    ConversionConstraints::Video(v) => v.container.extension(v.audio.as_ref().map(|a| &a.format)),
                };
                let output_path = temp_dir.join(format!("{}.{}", source_file.item_id, output_ext));

                let metadata = job.metadata.as_ref().map(|m| EmbeddedMetadata {
                    title: m.title.clone(),
                    artist: m.artist.clone(),
                    album: m.album.clone(),
                    album_artist: m.album_artist.clone(),
                    year: m.year,
                    track_number: m.track_number,
                    track_total: m.track_total,
                    disc_number: m.disc_number,
                    disc_total: m.disc_total,
                    genre: m.genre.clone(),
                    comment: m.comment.clone(),
                    extra: Default::default(),
                });

                let conv_job = ConversionJob {
                    job_id: format!("{}-{}", ticket_id, source_file.item_id),
                    input_path: source_file.path.clone(),
                    output_path: output_path.clone(),
                    constraints: constraints.clone(),
                    metadata,
                    cover_art_path: job.metadata.as_ref().and_then(|m| m.cover_art.clone()),
                };

                // Create channel for FFmpeg progress updates
                let (conv_progress_tx, mut conv_progress_rx) = mpsc::channel::<ConversionProgress>(32);

                // Spawn task to forward FFmpeg progress to the callback
                let progress_ticket_id = ticket_id.clone();
                let progress_file_name = current_file_name.clone();
                let progress_cb = on_progress.clone();
                let progress_idx = idx;
                let progress_total = total_files;
                let progress_forwarder = tokio::spawn(async move {
                    while let Some(progress) = conv_progress_rx.recv().await {
                        if let Some(ref cb) = progress_cb {
                            cb(
                                &progress_ticket_id,
                                "converting",
                                progress_idx,
                                progress_total,
                                &progress_file_name,
                                progress.percent,
                            );
                        }
                    }
                });

                // Run conversion with progress
                let conv_result = converter.convert_with_progress(conv_job, conv_progress_tx).await;

                // Wait for progress forwarder to complete
                let _ = progress_forwarder.await;

                match conv_result {
                    Ok(result) => {
                        // Send 100% progress for this file
                        if let Some(ref cb) = on_progress {
                            cb(&ticket_id, "converting", idx, total_files, &current_file_name, 100.0);
                        }
                        converted_files.push((source_file.clone(), result, output_path));
                    }
                    Err(e) => {
                        conversion_stats.active.fetch_sub(1, Ordering::Relaxed);
                        conversion_stats.total_failed.fetch_add(1, Ordering::Relaxed);

                        // Update ticket state to Failed
                        if let Some(ref store) = ticket_store {
                            update_ticket_state(
                                store,
                                TicketState::Failed {
                                    error: format!("Conversion failed: {}", e),
                                    retryable: e.is_retryable(),
                                    retry_count: 0,
                                    failed_at: chrono::Utc::now(),
                                },
                            );

                            // Emit state change event
                            if let Some(ref audit) = audit {
                                audit.emit(AuditEvent::TicketStateChanged {
                                    ticket_id: ticket_id.clone(),
                                    from_state: "converting".to_string(),
                                    to_state: "failed".to_string(),
                                    reason: Some(format!("Conversion failed: {}", e)),
                                }).await;
                            }
                        }

                        if let Some(ref audit) = audit {
                            audit.emit(AuditEvent::ConversionFailed {
                                ticket_id: ticket_id.clone(),
                                job_id: ticket_id.clone(),
                                failed_file: Some(source_file.path.to_string_lossy().to_string()),
                                error: e.to_string(),
                                files_completed: idx,
                                retryable: e.is_retryable(),
                            }).await;
                        }

                        return Err(PipelineError::ConversionFailed(e.to_string()));
                    }
                }
            }
        } else {
            // No conversion needed - files will be copied directly in placement phase
            // We still track stats but skip actual conversion
            tracing::info!(
                ticket_id = %ticket_id,
                files = job.source_files.len(),
                "Skipping conversion - no output constraints specified"
            );
        }

        let conversion_duration = conversion_start.elapsed();
        conversion_stats.active.fetch_sub(1, Ordering::Relaxed);
        conversion_stats.total_processed.fetch_add(1, Ordering::Relaxed);

        // Calculate total output bytes and build placements based on whether conversion happened
        let (total_output_bytes, placements, cleanup_sources) = if needs_conversion {
            // Conversion happened - use converted files
            let bytes: u64 = converted_files.iter().map(|(_, r, _)| r.output_size_bytes).sum();
            let placements: Vec<FilePlacement> = converted_files
                .iter()
                .map(|(source, _, temp_path)| {
                    let dest_path = job.dest_dir.join(&source.dest_filename);
                    FilePlacement {
                        item_id: source.item_id.clone(),
                        source: temp_path.clone(),
                        destination: dest_path,
                        overwrite: true,
                        verify_checksum: None,
                    }
                })
                .collect();
            (bytes, placements, true) // Clean up temp files after placement
        } else {
            // No conversion - place source files directly
            // Get file sizes asynchronously
            let mut bytes: u64 = 0;
            for source in &job.source_files {
                if let Ok(meta) = tokio::fs::metadata(&source.path).await {
                    bytes += meta.len();
                }
            }
            let placements: Vec<FilePlacement> = job.source_files
                .iter()
                .map(|source| {
                    let dest_path = job.dest_dir.join(&source.dest_filename);
                    FilePlacement {
                        item_id: source.item_id.clone(),
                        source: source.path.clone(),
                        destination: dest_path,
                        overwrite: true,
                        verify_checksum: None,
                    }
                })
                .collect();
            (bytes, placements, false) // Don't clean up source files
        };

        // Emit conversion completed event (only if conversion happened)
        if needs_conversion {
            if let Some(ref audit) = audit {
                audit.emit(AuditEvent::ConversionCompleted {
                    ticket_id: ticket_id.clone(),
                    job_id: ticket_id.clone(),
                    files_converted: converted_files.len(),
                    output_bytes: total_output_bytes,
                    duration_ms: conversion_duration.as_millis() as u64,
                    input_format: converted_files.first().map(|(_, r, _)| r.input_format.clone()).unwrap_or_default(),
                    output_format: converted_files.first().map(|(_, r, _)| r.output_format.clone()).unwrap_or_default(),
                }).await;
            }
        }

        // Phase 2: Placement
        placement_stats.queued.fetch_add(1, Ordering::Relaxed);

        let files_to_place = placements.len();

        // Update job state
        {
            let mut jobs = active_jobs.write().await;
            if let Some(state) = jobs.get_mut(&ticket_id) {
                *state = JobState::Placing {
                    started_at: Instant::now(),
                    files_placed: 0,
                    total_files: files_to_place,
                };
            }
        }

        // Update ticket state to Placing
        if let Some(ref store) = ticket_store {
            update_ticket_state(
                store,
                TicketState::Placing {
                    started_at: chrono::Utc::now(),
                    files_placed: 0,
                    total_files: files_to_place,
                },
            );

            // Emit state change event
            if let Some(ref audit) = audit {
                audit.emit(AuditEvent::TicketStateChanged {
                    ticket_id: ticket_id.clone(),
                    from_state: "converting".to_string(),
                    to_state: "placing".to_string(),
                    reason: Some(format!("Placing {} files", files_to_place)),
                }).await;
            }
        }

        // Send placing progress via callback (for WebSocket)
        if let Some(ref cb) = on_progress {
            cb(&ticket_id, "placing", 0, files_to_place, "", 0.0);
        }

        let _permit = placement_semaphore
            .acquire()
            .await
            .map_err(|_| PipelineError::NotRunning)?;
        placement_stats.queued.fetch_sub(1, Ordering::Relaxed);
        placement_stats.active.fetch_add(1, Ordering::Relaxed);

        // Emit placement started event
        if let Some(ref audit) = audit {
            audit.emit(AuditEvent::PlacementStarted {
                ticket_id: ticket_id.clone(),
                job_id: ticket_id.clone(),
                total_files: files_to_place,
                total_bytes: total_output_bytes,
            }).await;
        }

        let placement_start = Instant::now();

        let placement_job = PlacementJob {
            job_id: ticket_id.clone(),
            files: placements,
            atomic: true,
            cleanup_sources,
            enable_rollback: true,
        };

        match placer.place(placement_job).await {
            Ok(result) => {
                let placement_duration = placement_start.elapsed();
                placement_stats.active.fetch_sub(1, Ordering::Relaxed);
                placement_stats.total_processed.fetch_add(1, Ordering::Relaxed);

                // Emit placement completed event
                if let Some(ref audit) = audit {
                    audit.emit(AuditEvent::PlacementCompleted {
                        ticket_id: ticket_id.clone(),
                        job_id: ticket_id.clone(),
                        files_placed: result.files_placed.len(),
                        total_bytes: result.total_bytes,
                        duration_ms: placement_duration.as_millis() as u64,
                        dest_dir: job.dest_dir.to_string_lossy().to_string(),
                    }).await;
                }

                // Clean up temp directory (only if conversion was used)
                if needs_conversion {
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                }

                let files_placed: Vec<PlacedFileInfo> = result
                    .files_placed
                    .into_iter()
                    .map(|f| PlacedFileInfo {
                        item_id: f.item_id,
                        path: f.destination,
                        size_bytes: f.size_bytes,
                    })
                    .collect();

                // Update ticket state to Completed
                if let Some(ref store) = ticket_store {
                    update_ticket_state(
                        store,
                        TicketState::Completed {
                            completed_at: chrono::Utc::now(),
                            stats: CompletionStats {
                                total_download_bytes: 0, // Not tracked in pipeline
                                download_duration_secs: 0,
                                conversion_duration_secs: conversion_duration.as_secs() as u32,
                                final_size_bytes: result.total_bytes,
                                files_placed: files_placed.len() as u32,
                            },
                        },
                    );

                    // Emit state change event
                    if let Some(ref audit) = audit {
                        audit.emit(AuditEvent::TicketStateChanged {
                            ticket_id: ticket_id.clone(),
                            from_state: "placing".to_string(),
                            to_state: "completed".to_string(),
                            reason: Some(format!("Successfully placed {} files", files_placed.len())),
                        }).await;
                    }
                }

                Ok(PipelineResult {
                    ticket_id,
                    success: true,
                    files_placed,
                    conversion_duration_ms: conversion_duration.as_millis() as u64,
                    placement_duration_ms: placement_duration.as_millis() as u64,
                    total_bytes: result.total_bytes,
                    error: None,
                })
            }
            Err(e) => {
                placement_stats.active.fetch_sub(1, Ordering::Relaxed);
                placement_stats.total_failed.fetch_add(1, Ordering::Relaxed);

                // Update ticket state to Failed
                if let Some(ref store) = ticket_store {
                    update_ticket_state(
                        store,
                        TicketState::Failed {
                            error: format!("Placement failed: {}", e),
                            retryable: e.is_retryable(),
                            retry_count: 0,
                            failed_at: chrono::Utc::now(),
                        },
                    );

                    // Emit state change event
                    if let Some(ref audit) = audit {
                        audit.emit(AuditEvent::TicketStateChanged {
                            ticket_id: ticket_id.clone(),
                            from_state: "placing".to_string(),
                            to_state: "failed".to_string(),
                            reason: Some(format!("Placement failed: {}", e)),
                        }).await;
                    }
                }

                // Emit placement failed event
                if let Some(ref audit) = audit {
                    audit.emit(AuditEvent::PlacementFailed {
                        ticket_id: ticket_id.clone(),
                        job_id: ticket_id.clone(),
                        failed_file: None,
                        error: e.to_string(),
                        files_completed: 0,
                    }).await;
                }

                // Clean up temp directory (only if conversion was used)
                if needs_conversion {
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                }

                Err(PipelineError::PlacementFailed(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::{AudioConstraints, AudioFormat, ConverterConfig, FfmpegConverter};
    use crate::placer::{FsPlacer, PlacerConfig};
    use std::path::PathBuf;

    // Note: These tests require actual ffmpeg to be installed
    // In CI, these should be marked as #[ignore]

    #[tokio::test]
    async fn test_pipeline_status() {
        let config = ProcessorConfig::default();
        let converter = FfmpegConverter::new(ConverterConfig::default());
        let placer = FsPlacer::new(PlacerConfig::default());

        let processor = PipelineProcessor::new(config, converter, placer);
        processor.start().await;

        let status = processor.status().await;
        assert!(status.running);
        assert_eq!(status.conversion_pool.active_jobs, 0);
        assert_eq!(status.placement_pool.active_jobs, 0);
    }

    #[tokio::test]
    async fn test_pipeline_not_running() {
        let config = ProcessorConfig::default();
        let converter = FfmpegConverter::new(ConverterConfig::default());
        let placer = FsPlacer::new(PlacerConfig::default());

        let processor = PipelineProcessor::new(config, converter, placer);
        // Don't start the processor

        let job = PipelineJob {
            ticket_id: "test-1".to_string(),
            source_files: vec![],
            file_mappings: vec![],
            constraints: Some(ConversionConstraints::Audio(AudioConstraints {
                format: AudioFormat::OggVorbis,
                bitrate_kbps: Some(320),
                sample_rate_hz: None,
                channels: None,
                compression_level: None,
            })),
            dest_dir: PathBuf::from("/tmp/test"),
            metadata: None,
        };

        let result = processor.process(job, None).await;
        assert!(matches!(result, Err(PipelineError::NotRunning)));
    }
}
