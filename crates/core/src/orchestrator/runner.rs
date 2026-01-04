//! Ticket orchestrator implementation.
//!
//! Drives tickets through the state machine automatically:
//! - Acquisition: Sequential (one ticket at a time) - CPU-bound
//! - Download: Concurrent monitoring (many downloads) - IO-bound
//! - Pipeline: Sequential (handled by PipelineProcessor)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use crate::audit::{AuditEvent, AuditHandle};
use crate::catalog::TorrentCatalog;
use crate::metrics;
use crate::processor::{PipelineJob, PipelineProcessor, SourceFile};
use crate::searcher::{FileEnricher, Searcher};
use crate::textbrain::training::create_acquisition_training_events;
use crate::textbrain::{
    AcquisitionAuditContext, AcquisitionProgress, AcquisitionStateUpdater, AnthropicClient,
    DumbMatcher, DumbQueryBuilder, LlmMatcher, LlmProvider, LlmQueryBuilder, OllamaClient,
    ScoredCandidate, ScoredCandidateSummary, TextBrain, TextBrainConfig,
};
use crate::ticket::{
    AcquisitionPhase, RetryPhase, SelectedCandidate, Ticket, TicketFilter, TicketState, TicketStore,
};
use crate::torrent_client::{AddTorrentRequest, TorrentClient, TorrentInfo, TorrentState};

use super::config::OrchestratorConfig;
use super::types::{ActiveDownload, OrchestratorError, OrchestratorStatus};

/// Callback type for ticket update notifications.
/// Called with (ticket_id, state_type) whenever a ticket's state changes.
pub type TicketUpdateCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Helper to notify about ticket updates if callback is present.
fn notify_update(callback: &Option<TicketUpdateCallback>, ticket_id: &str, state_type: &str) {
    if let Some(ref cb) = callback {
        cb(ticket_id, state_type);
    }
}

/// Helper to update ticket state and notify.
fn update_and_notify_static(
    ticket_store: &Arc<dyn TicketStore>,
    callback: &Option<TicketUpdateCallback>,
    ticket_id: &str,
    new_state: TicketState,
) -> Result<(), OrchestratorError> {
    let state_type = new_state.state_type();
    ticket_store.update_state(ticket_id, new_state)?;
    notify_update(callback, ticket_id, state_type);
    Ok(())
}

/// Helper to schedule a retry for a ticket.
///
/// Calculates the next retry delay using exponential backoff and transitions
/// the ticket to PendingRetry state. Returns Ok(true) if retry was scheduled,
/// Ok(false) if max retries exceeded (caller should transition to Failed).
///
/// Also increments the ticket's retry_count to track total retries.
fn schedule_retry(
    ticket_store: &Arc<dyn TicketStore>,
    callback: &Option<TicketUpdateCallback>,
    ticket_id: &str,
    error: &str,
    current_retry_count: u32,
    failed_phase: RetryPhase,
    config: &super::config::RetryConfig,
) -> Result<bool, OrchestratorError> {
    let next_attempt = current_retry_count + 1;

    // Check if we should retry (current_retry_count is the number of retries already done)
    if !config.should_retry(current_retry_count) {
        return Ok(false);
    }

    // Increment the retry count in the database
    ticket_store.increment_retry_count(ticket_id)?;

    // Calculate delay for the next attempt
    let delay = config
        .delay_for_attempt(next_attempt)
        .expect("should_retry returned true, so delay must exist");

    let now = Utc::now();
    let retry_after =
        now + chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::seconds(60));

    let new_state = TicketState::PendingRetry {
        error: error.to_string(),
        retry_attempt: next_attempt,
        retry_after,
        failed_phase,
        scheduled_at: now,
    };

    update_and_notify_static(ticket_store, callback, ticket_id, new_state)?;

    info!(
        "Scheduled retry {} for ticket {} after {:?} (phase: {})",
        next_attempt, ticket_id, delay, failed_phase
    );

    Ok(true)
}

/// Implementation of AcquisitionStateUpdater that persists progress to the ticket store.
struct TicketStateUpdater {
    ticket_id: String,
    started_at: DateTime<Utc>,
    ticket_store: Arc<dyn TicketStore>,
    on_update: Option<TicketUpdateCallback>,
}

#[async_trait::async_trait]
impl AcquisitionStateUpdater for TicketStateUpdater {
    async fn update_progress(&self, progress: AcquisitionProgress) {
        let new_state = TicketState::Acquiring {
            started_at: self.started_at,
            queries_tried: progress.queries_tried,
            candidates_found: progress.candidates_found,
            phase: progress.phase,
        };

        if let Err(e) = self.ticket_store.update_state(&self.ticket_id, new_state) {
            tracing::warn!(
                "Failed to update acquisition progress for ticket {}: {}",
                self.ticket_id,
                e
            );
        } else {
            notify_update(&self.on_update, &self.ticket_id, "acquiring");
        }
    }
}

/// The ticket orchestrator - drives tickets through the processing pipeline.
pub struct TicketOrchestrator<C, P>
where
    C: crate::converter::Converter + 'static,
    P: crate::placer::Placer + 'static,
{
    config: OrchestratorConfig,
    ticket_store: Arc<dyn TicketStore>,
    searcher: Arc<dyn Searcher>,
    torrent_client: Arc<dyn TorrentClient>,
    pipeline: Arc<PipelineProcessor<C, P>>,
    catalog: Arc<dyn TorrentCatalog>,
    audit: Option<AuditHandle>,
    textbrain_config: TextBrainConfig,

    /// Optional callback for ticket update notifications (for WebSocket broadcast)
    on_ticket_update: Option<TicketUpdateCallback>,

    // Runtime state
    running: Arc<AtomicBool>,
    active_downloads: Arc<RwLock<HashMap<String, ActiveDownload>>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl<C, P> TicketOrchestrator<C, P>
where
    C: crate::converter::Converter + 'static,
    P: crate::placer::Placer + 'static,
{
    /// Create a new orchestrator.
    pub fn new(
        config: OrchestratorConfig,
        ticket_store: Arc<dyn TicketStore>,
        searcher: Arc<dyn Searcher>,
        torrent_client: Arc<dyn TorrentClient>,
        pipeline: Arc<PipelineProcessor<C, P>>,
        catalog: Arc<dyn TorrentCatalog>,
        audit: Option<AuditHandle>,
        textbrain_config: TextBrainConfig,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            config,
            ticket_store,
            searcher,
            torrent_client,
            pipeline,
            catalog,
            audit,
            textbrain_config,
            on_ticket_update: None,
            running: Arc::new(AtomicBool::new(false)),
            active_downloads: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
        }
    }

    /// Set the callback for ticket update notifications.
    pub fn with_update_callback(mut self, callback: TicketUpdateCallback) -> Self {
        self.on_ticket_update = Some(callback);
        self
    }

    /// Start the orchestrator (spawns background tasks).
    pub async fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            warn!("Orchestrator already running");
            return;
        }

        info!("Starting ticket orchestrator");

        // Recover any downloads that were in progress when we shut down
        self.recover_downloading_tickets().await;

        // Spawn acquisition loop
        self.spawn_acquisition_loop();

        // Spawn download monitor loop
        self.spawn_download_monitor_loop();

        // Spawn retry monitor loop
        self.spawn_retry_monitor_loop();

        info!("Ticket orchestrator started");
    }

    /// Stop the orchestrator gracefully.
    pub async fn stop(&self) {
        if !self.running.swap(false, Ordering::SeqCst) {
            warn!("Orchestrator not running");
            return;
        }

        info!("Stopping ticket orchestrator");

        // Signal shutdown to all workers
        let _ = self.shutdown_tx.send(());

        // Give workers a moment to finish current work
        tokio::time::sleep(Duration::from_millis(500)).await;

        info!("Ticket orchestrator stopped");
    }

    /// Get current orchestrator status.
    pub async fn status(&self) -> OrchestratorStatus {
        let active_downloads = self.active_downloads.read().await.len();

        // Count tickets in various states
        let pending_count = self
            .ticket_store
            .count(&TicketFilter::new().with_state("pending"))
            .unwrap_or(0) as usize;

        let acquiring_count = self
            .ticket_store
            .count(&TicketFilter::new().with_state("acquiring"))
            .unwrap_or(0) as usize;

        let needs_approval_count = self
            .ticket_store
            .count(&TicketFilter::new().with_state("needs_approval"))
            .unwrap_or(0) as usize;

        let downloading_count = self
            .ticket_store
            .count(&TicketFilter::new().with_state("downloading"))
            .unwrap_or(0) as usize;

        OrchestratorStatus {
            running: self.running.load(Ordering::Relaxed),
            active_downloads,
            acquiring_count,
            pending_count,
            needs_approval_count,
            downloading_count,
        }
    }

    /// Recover tickets that were downloading when we shut down.
    async fn recover_downloading_tickets(&self) {
        let filter = TicketFilter::new()
            .with_state("downloading")
            .with_limit(100);

        match self.ticket_store.list(&filter) {
            Ok(tickets) => {
                let mut downloads = self.active_downloads.write().await;
                for ticket in tickets {
                    if let TicketState::Downloading {
                        info_hash,
                        started_at,
                        candidate_idx,
                        failover_round,
                        last_progress_pct,
                        last_progress_at,
                        ..
                    } = &ticket.state
                    {
                        downloads.insert(
                            ticket.id.clone(),
                            ActiveDownload {
                                ticket_id: ticket.id.clone(),
                                info_hash: info_hash.clone(),
                                started_at: *started_at,
                                candidate_idx: *candidate_idx,
                                failover_round: *failover_round,
                                last_progress_pct: *last_progress_pct,
                                last_progress_at: *last_progress_at,
                            },
                        );
                        info!("Recovered downloading ticket: {}", ticket.id);
                    }
                }
                if !downloads.is_empty() {
                    info!("Recovered {} downloading tickets", downloads.len());
                }
            }
            Err(e) => {
                error!("Failed to recover downloading tickets: {}", e);
            }
        }
    }

    /// Spawn the acquisition loop task.
    fn spawn_acquisition_loop(&self) {
        let running = Arc::clone(&self.running);
        let ticket_store = Arc::clone(&self.ticket_store);
        let searcher = Arc::clone(&self.searcher);
        let catalog = Arc::clone(&self.catalog);
        let config = self.config.clone();
        let textbrain_config = self.textbrain_config.clone();
        let audit = self.audit.clone();
        let on_update = self.on_ticket_update.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!("Acquisition loop started");
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Acquisition loop received shutdown signal");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(config.acquisition_poll_interval_ms)) => {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Err(e) = Self::process_one_pending(
                            &ticket_store,
                            &searcher,
                            &catalog,
                            &config,
                            &textbrain_config,
                            &audit,
                            &on_update,
                        ).await {
                            warn!("Acquisition error: {}", e);
                        }
                    }
                }
            }
            info!("Acquisition loop stopped");
        });
    }

    /// Spawn the download monitor loop task.
    fn spawn_download_monitor_loop(&self) {
        let running = Arc::clone(&self.running);
        let ticket_store = Arc::clone(&self.ticket_store);
        let torrent_client = Arc::clone(&self.torrent_client);
        let pipeline = Arc::clone(&self.pipeline);
        let active_downloads = Arc::clone(&self.active_downloads);
        let config = self.config.clone();
        let audit = self.audit.clone();
        let on_update = self.on_ticket_update.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!("Download monitor loop started");
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Download monitor loop received shutdown signal");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(config.download_poll_interval_ms)) => {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }

                        // Start approved downloads
                        if let Err(e) = Self::start_approved_downloads(
                            &ticket_store,
                            &torrent_client,
                            &active_downloads,
                            &config,
                            &audit,
                            &on_update,
                        ).await {
                            warn!("Failed to start downloads: {}", e);
                        }

                        // Check progress of active downloads
                        if let Err(e) = Self::check_download_progress(
                            &ticket_store,
                            &torrent_client,
                            &pipeline,
                            &active_downloads,
                            &config,
                            &audit,
                            &on_update,
                        ).await {
                            warn!("Failed to check downloads: {}", e);
                        }
                    }
                }
            }
            info!("Download monitor loop stopped");
        });
    }

    /// Spawn the retry monitor loop task.
    /// This loop checks for PendingRetry tickets whose retry_after time has passed
    /// and transitions them back to the appropriate processing state.
    fn spawn_retry_monitor_loop(&self) {
        let running = Arc::clone(&self.running);
        let ticket_store = Arc::clone(&self.ticket_store);
        let config = self.config.clone();
        let audit = self.audit.clone();
        let on_update = self.on_ticket_update.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!("Retry monitor loop started");
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Retry monitor loop received shutdown signal");
                        break;
                    }
                    // Check every 5 seconds for ready retries
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Err(e) = Self::process_ready_retries(
                            &ticket_store,
                            &config,
                            &audit,
                            &on_update,
                        ).await {
                            warn!("Retry processing error: {}", e);
                        }
                    }
                }
            }
            info!("Retry monitor loop stopped");
        });
    }

    /// Process tickets that are ready for retry.
    async fn process_ready_retries(
        ticket_store: &Arc<dyn TicketStore>,
        _config: &OrchestratorConfig,
        audit: &Option<AuditHandle>,
        on_update: &Option<TicketUpdateCallback>,
    ) -> Result<(), OrchestratorError> {
        // Get all PendingRetry tickets
        let filter = TicketFilter::new()
            .with_state("pending_retry")
            .with_limit(50);
        let tickets = ticket_store.list(&filter)?;

        let now = Utc::now();

        for ticket in tickets {
            if let TicketState::PendingRetry {
                retry_after,
                failed_phase,
                retry_attempt,
                error,
                ..
            } = &ticket.state
            {
                // Check if it's time to retry
                if now >= *retry_after {
                    info!(
                        "Ticket {} ready for retry (attempt {}, phase: {})",
                        ticket.id, retry_attempt, failed_phase
                    );

                    // Transition to the appropriate state based on the failed phase
                    let new_state = match failed_phase {
                        RetryPhase::Acquisition => {
                            // Go back to Pending to restart acquisition
                            TicketState::Pending
                        }
                        RetryPhase::Download => {
                            // Go back to AutoApproved/Approved state
                            // For now, go to Pending as we don't preserve the selected candidate
                            // TODO: In a future enhancement, store selected candidate in PendingRetry
                            TicketState::Pending
                        }
                        RetryPhase::Conversion | RetryPhase::Placement => {
                            // Go back to Pending for now
                            // TODO: In a future enhancement, resume from download complete
                            TicketState::Pending
                        }
                    };

                    let from_state = ticket.state.state_type().to_string();
                    let to_state = new_state.state_type();

                    update_and_notify_static(ticket_store, on_update, &ticket.id, new_state)?;

                    // Emit audit event
                    if let Some(ref audit_handle) = audit {
                        audit_handle
                            .emit(AuditEvent::TicketStateChanged {
                                ticket_id: ticket.id.clone(),
                                from_state,
                                to_state: to_state.to_string(),
                                reason: Some(format!(
                                    "Retry attempt {} after error: {}",
                                    retry_attempt, error
                                )),
                            })
                            .await;
                    }

                    info!(
                        "Ticket {} transitioned from pending_retry to {} for retry attempt {}",
                        ticket.id, to_state, retry_attempt
                    );
                }
            }
        }

        Ok(())
    }

    /// Process one pending ticket (acquisition).
    async fn process_one_pending(
        ticket_store: &Arc<dyn TicketStore>,
        searcher: &Arc<dyn Searcher>,
        catalog: &Arc<dyn TorrentCatalog>,
        config: &OrchestratorConfig,
        textbrain_config: &TextBrainConfig,
        audit: &Option<AuditHandle>,
        on_update: &Option<TicketUpdateCallback>,
    ) -> Result<(), OrchestratorError> {
        // Get highest priority pending ticket
        let filter = TicketFilter::new().with_state("pending").with_limit(1);
        let tickets = ticket_store.list(&filter)?;

        let Some(ticket) = tickets.first() else {
            return Ok(()); // Nothing to do
        };

        debug!("Processing pending ticket: {}", ticket.id);

        // Transition to Acquiring
        let started_at = Utc::now();
        update_and_notify_static(
            ticket_store,
            on_update,
            &ticket.id,
            TicketState::Acquiring {
                started_at,
                queries_tried: vec![],
                candidates_found: 0,
                phase: AcquisitionPhase::QueryBuilding,
            },
        )?;

        // Emit state change event
        if let Some(ref audit_handle) = audit {
            audit_handle
                .emit(AuditEvent::TicketStateChanged {
                    ticket_id: ticket.id.clone(),
                    from_state: "pending".to_string(),
                    to_state: "acquiring".to_string(),
                    reason: Some("Starting acquisition".to_string()),
                })
                .await;
        }

        // Build TextBrain with configured implementations
        let textbrain = Self::build_textbrain(textbrain_config, Arc::clone(catalog));

        // Create state updater for persisting acquisition progress
        let state_updater: Arc<dyn AcquisitionStateUpdater> = Arc::new(TicketStateUpdater {
            ticket_id: ticket.id.clone(),
            started_at,
            ticket_store: Arc::clone(ticket_store),
            on_update: on_update.clone(),
        });

        // Execute acquisition with fallback (tries discography if album search fails)
        let result = if let Some(ref audit_handle) = audit {
            // Use acquire_with_fallback_and_audit for detailed real-time events
            let audit_ctx = AcquisitionAuditContext {
                ticket_id: ticket.id.clone(),
                audit: audit_handle.clone(),
                started_at,
                state_updater: Some(state_updater),
            };
            textbrain
                .acquire_with_fallback_and_audit(
                    &ticket.query_context,
                    searcher.as_ref(),
                    &audit_ctx,
                )
                .await
        } else {
            // No audit configured, use regular acquire with fallback
            textbrain
                .acquire_with_fallback(&ticket.query_context, searcher.as_ref())
                .await
        };

        match result {
            Ok(acq) => {
                // Record acquisition metrics
                let duration_secs = acq.duration_ms as f64 / 1000.0;
                metrics::QUERIES_GENERATED
                    .with_label_values(&[])
                    .observe(acq.queries_tried.len() as f64);
                metrics::CANDIDATES_FOUND
                    .with_label_values(&[])
                    .observe(acq.candidates_evaluated as f64);
                if let Some(ref best) = acq.best_candidate {
                    metrics::MATCH_CONFIDENCE
                        .with_label_values(&[])
                        .observe(best.score as f64);
                }

                // Emit summary audit events for the acquisition (QueriesGenerated, CandidatesScored)
                // These are kept for backward compatibility and training data collection
                if let Some(ref audit_handle) = audit {
                    // QueriesGenerated event (summary)
                    let queries_event = AuditEvent::QueriesGenerated {
                        ticket_id: ticket.id.clone(),
                        queries: acq.queries_tried.clone(),
                        method: acq.query_method.clone(),
                        llm_input_tokens: acq.llm_usage.as_ref().map(|u| u.input_tokens),
                        llm_output_tokens: acq.llm_usage.as_ref().map(|u| u.output_tokens),
                        duration_ms: acq.duration_ms,
                    };
                    audit_handle.emit(queries_event).await;

                    // CandidatesScored event (summary)
                    let scored_event = AuditEvent::CandidatesScored {
                        ticket_id: ticket.id.clone(),
                        candidates_count: acq.candidates_evaluated,
                        top_candidate_hash: acq
                            .best_candidate
                            .as_ref()
                            .map(|c| c.candidate.info_hash.clone()),
                        top_candidate_score: acq
                            .best_candidate
                            .as_ref()
                            .map(|c| (c.score * 100.0) as u32),
                        method: acq.score_method.clone(),
                        llm_input_tokens: acq.llm_usage.as_ref().map(|u| u.input_tokens),
                        llm_output_tokens: acq.llm_usage.as_ref().map(|u| u.output_tokens),
                        duration_ms: acq.duration_ms,
                    };
                    audit_handle.emit(scored_event).await;

                    // Training events (for LLM fine-tuning data)
                    let training_events =
                        create_acquisition_training_events(&ticket.id, &ticket.query_context, &acq);
                    for event in training_events {
                        audit_handle.emit(event).await;
                    }
                }

                if acq.auto_approved {
                    if let Some(ref candidate) = acq.best_candidate {
                        // Auto-approved - high confidence match
                        // Build all candidates for failover (up to max_failover_candidates)
                        let candidates: Vec<SelectedCandidate> = acq
                            .all_candidates
                            .iter()
                            .take(config.max_failover_candidates)
                            .map(Self::build_selected_candidate)
                            .collect();

                        let selected = Self::build_selected_candidate(candidate);
                        update_and_notify_static(
                            &ticket_store,
                            &on_update,
                            &ticket.id,
                            TicketState::AutoApproved {
                                selected,
                                candidates,
                                confidence: candidate.score,
                                approved_at: Utc::now(),
                            },
                        )?;

                        // Emit state change event
                        if let Some(ref audit_handle) = audit {
                            audit_handle
                                .emit(AuditEvent::TicketStateChanged {
                                    ticket_id: ticket.id.clone(),
                                    from_state: "acquiring".to_string(),
                                    to_state: "auto_approved".to_string(),
                                    reason: Some(format!(
                                        "Auto-approved with score {:.2}",
                                        candidate.score
                                    )),
                                })
                                .await;
                        }

                        // Record acquisition success metrics
                        metrics::ACQUISITION_ATTEMPTS
                            .with_label_values(&["auto_approved"])
                            .inc();
                        metrics::ACQUISITION_DURATION
                            .with_label_values(&["auto_approved"])
                            .observe(duration_secs);

                        info!(
                            "Ticket {} auto-approved with score {:.2}",
                            ticket.id, candidate.score
                        );
                    } else {
                        // No candidate found - record failure
                        metrics::ACQUISITION_ATTEMPTS
                            .with_label_values(&["failed"])
                            .inc();
                        metrics::ACQUISITION_DURATION
                            .with_label_values(&["failed"])
                            .observe(duration_secs);
                        update_and_notify_static(
                            &ticket_store,
                            &on_update,
                            &ticket.id,
                            TicketState::AcquisitionFailed {
                                queries_tried: acq.queries_tried.clone(),
                                candidates_seen: acq.candidates_evaluated,
                                reason: "No candidates found".to_string(),
                                failed_at: Utc::now(),
                            },
                        )?;

                        // Emit state change event
                        if let Some(ref audit_handle) = audit {
                            audit_handle
                                .emit(AuditEvent::TicketStateChanged {
                                    ticket_id: ticket.id.clone(),
                                    from_state: "acquiring".to_string(),
                                    to_state: "acquisition_failed".to_string(),
                                    reason: Some("No candidates found".to_string()),
                                })
                                .await;
                        }
                    }
                } else if let Some(ref candidate) = acq.best_candidate {
                    // Needs manual approval - below threshold
                    let summaries: Vec<ScoredCandidateSummary> = acq
                        .all_candidates
                        .iter()
                        .take(5) // Top 5 candidates for review
                        .map(ScoredCandidateSummary::from)
                        .collect();

                    update_and_notify_static(
                        &ticket_store,
                        &on_update,
                        &ticket.id,
                        TicketState::NeedsApproval {
                            candidates: summaries,
                            recommended_idx: 0,
                            confidence: candidate.score,
                            waiting_since: Utc::now(),
                        },
                    )?;

                    // Emit state change event
                    if let Some(ref audit_handle) = audit {
                        audit_handle
                            .emit(AuditEvent::TicketStateChanged {
                                ticket_id: ticket.id.clone(),
                                from_state: "acquiring".to_string(),
                                to_state: "needs_approval".to_string(),
                                reason: Some(format!(
                                    "Best score {:.2} below threshold",
                                    candidate.score
                                )),
                            })
                            .await;
                    }

                    // Record needs_approval metrics
                    metrics::ACQUISITION_ATTEMPTS
                        .with_label_values(&["needs_approval"])
                        .inc();
                    metrics::ACQUISITION_DURATION
                        .with_label_values(&["needs_approval"])
                        .observe(duration_secs);

                    info!(
                        "Ticket {} needs approval, best score {:.2} < threshold {:.2}",
                        ticket.id, candidate.score, config.auto_approve_threshold
                    );
                } else {
                    // No candidate found - record failure
                    metrics::ACQUISITION_ATTEMPTS
                        .with_label_values(&["failed"])
                        .inc();
                    metrics::ACQUISITION_DURATION
                        .with_label_values(&["failed"])
                        .observe(duration_secs);

                    update_and_notify_static(
                        &ticket_store,
                        &on_update,
                        &ticket.id,
                        TicketState::AcquisitionFailed {
                            queries_tried: acq.queries_tried.clone(),
                            candidates_seen: acq.candidates_evaluated,
                            reason: "No suitable candidates found".to_string(),
                            failed_at: Utc::now(),
                        },
                    )?;

                    // Emit state change event
                    if let Some(ref audit_handle) = audit {
                        audit_handle
                            .emit(AuditEvent::TicketStateChanged {
                                ticket_id: ticket.id.clone(),
                                from_state: "acquiring".to_string(),
                                to_state: "acquisition_failed".to_string(),
                                reason: Some("No suitable candidates found".to_string()),
                            })
                            .await;
                    }
                }
            }
            Err(e) => {
                let error_reason = e.to_string();

                // Check if this is a retryable error (transient failures)
                // Network errors, API timeouts, etc. are retryable
                // "No candidates found" type errors are not (those go to AcquisitionFailed)
                let is_retryable = Self::is_retryable_error(&error_reason);

                if is_retryable {
                    // Try to schedule a retry using the ticket's retry_count
                    let retry_scheduled = schedule_retry(
                        ticket_store,
                        on_update,
                        &ticket.id,
                        &error_reason,
                        ticket.retry_count,
                        RetryPhase::Acquisition,
                        &config.retry,
                    )?;

                    if retry_scheduled {
                        // Record retry metric
                        metrics::RETRY_ATTEMPTS
                            .with_label_values(&["acquisition"])
                            .inc();

                        // Emit state change event
                        if let Some(ref audit_handle) = audit {
                            audit_handle
                                .emit(AuditEvent::TicketStateChanged {
                                    ticket_id: ticket.id.clone(),
                                    from_state: "acquiring".to_string(),
                                    to_state: "pending_retry".to_string(),
                                    reason: Some(format!(
                                        "Transient error, scheduling retry: {}",
                                        error_reason
                                    )),
                                })
                                .await;
                        }

                        warn!(
                            "Acquisition failed for ticket {} (transient, will retry): {}",
                            ticket.id, error_reason
                        );
                        return Ok(());
                    }
                }

                // Record acquisition failure metric
                metrics::ACQUISITION_ATTEMPTS
                    .with_label_values(&["failed"])
                    .inc();

                // Either not retryable or max retries exceeded - go to AcquisitionFailed
                update_and_notify_static(
                    ticket_store,
                    on_update,
                    &ticket.id,
                    TicketState::AcquisitionFailed {
                        queries_tried: vec![],
                        candidates_seen: 0,
                        reason: error_reason.clone(),
                        failed_at: Utc::now(),
                    },
                )?;

                // Emit state change event
                if let Some(ref audit_handle) = audit {
                    audit_handle
                        .emit(AuditEvent::TicketStateChanged {
                            ticket_id: ticket.id.clone(),
                            from_state: "acquiring".to_string(),
                            to_state: "acquisition_failed".to_string(),
                            reason: Some(error_reason.clone()),
                        })
                        .await;
                }

                warn!(
                    "Acquisition failed for ticket {}: {}",
                    ticket.id, error_reason
                );
            }
        }

        Ok(())
    }

    /// Start downloads for newly approved tickets.
    async fn start_approved_downloads(
        ticket_store: &Arc<dyn TicketStore>,
        torrent_client: &Arc<dyn TorrentClient>,
        active_downloads: &Arc<RwLock<HashMap<String, ActiveDownload>>>,
        config: &OrchestratorConfig,
        audit: &Option<AuditHandle>,
        on_update: &Option<TicketUpdateCallback>,
    ) -> Result<(), OrchestratorError> {
        // Process both auto_approved and manually approved tickets
        for state_type in ["auto_approved", "approved"] {
            let filter = TicketFilter::new().with_state(state_type).with_limit(10);

            let tickets = ticket_store.list(&filter)?;

            for ticket in tickets {
                // Skip if already tracking this download
                {
                    let downloads = active_downloads.read().await;
                    if downloads.contains_key(&ticket.id) {
                        continue;
                    }
                }

                // Check max concurrent limit
                if config.max_concurrent_downloads > 0 {
                    let downloads = active_downloads.read().await;
                    if downloads.len() >= config.max_concurrent_downloads {
                        debug!("Max concurrent downloads reached, waiting...");
                        break;
                    }
                }

                // Extract all candidates from state for failover
                let (_selected, candidates) = Self::extract_candidates(&ticket)?;

                // Try each candidate until one works
                let mut last_error = String::new();
                let mut success = false;

                for (candidate_idx, candidate) in candidates.iter().enumerate() {
                    debug!(
                        "Trying candidate {} of {} for ticket {}: {}",
                        candidate_idx + 1,
                        candidates.len(),
                        ticket.id,
                        candidate.title
                    );

                    let add_result =
                        Self::add_torrent_from_candidate(torrent_client.as_ref(), candidate).await;

                    match add_result {
                        Ok(result) => {
                            // Track active download with failover context
                            let now = Utc::now();
                            {
                                let mut downloads = active_downloads.write().await;
                                downloads.insert(
                                    ticket.id.clone(),
                                    ActiveDownload {
                                        ticket_id: ticket.id.clone(),
                                        info_hash: result.hash.clone(),
                                        started_at: now,
                                        candidate_idx,
                                        failover_round: 1,
                                        last_progress_pct: 0.0,
                                        last_progress_at: now,
                                    },
                                );
                            }

                            // Update ticket state with failover fields
                            update_and_notify_static(
                                &ticket_store,
                                &on_update,
                                &ticket.id,
                                TicketState::Downloading {
                                    info_hash: result.hash.clone(),
                                    progress_pct: 0.0,
                                    speed_bps: 0,
                                    eta_secs: None,
                                    started_at: now,
                                    candidate_idx,
                                    failover_round: 1,
                                    last_progress_pct: 0.0,
                                    last_progress_at: now,
                                    candidates: candidates.clone(),
                                },
                            )?;

                            // Emit state change event
                            if let Some(ref audit_handle) = audit {
                                audit_handle
                                    .emit(AuditEvent::TicketStateChanged {
                                        ticket_id: ticket.id.clone(),
                                        from_state: state_type.to_string(),
                                        to_state: "downloading".to_string(),
                                        reason: Some(format!("Started download: {}", result.hash)),
                                    })
                                    .await;
                            }

                            // Record download started metric
                            metrics::DOWNLOADS_STARTED.inc();

                            info!(
                                "Started download for ticket {} (candidate {}): {}",
                                ticket.id,
                                candidate_idx + 1,
                                result.hash
                            );
                            success = true;
                            break;
                        }
                        Err(e) => {
                            warn!(
                                "Failed to add candidate {} for ticket {}: {}",
                                candidate_idx + 1,
                                ticket.id,
                                e
                            );
                            last_error = format!("{}", e);
                            // Continue to next candidate
                        }
                    }
                }

                // If all candidates failed, try to schedule a retry
                if !success {
                    let error_msg = format!(
                        "All {} candidates failed to add. Last error: {}",
                        candidates.len(),
                        last_error
                    );
                    error!(
                        "All {} candidates failed for ticket {}",
                        candidates.len(),
                        ticket.id
                    );

                    // Check if error is retryable
                    let is_retryable = Self::is_retryable_error(&last_error);

                    if is_retryable {
                        let retry_scheduled = schedule_retry(
                            ticket_store,
                            on_update,
                            &ticket.id,
                            &error_msg,
                            ticket.retry_count,
                            RetryPhase::Download,
                            &config.retry,
                        )?;

                        if retry_scheduled {
                            // Record retry metric
                            metrics::RETRY_ATTEMPTS
                                .with_label_values(&["download"])
                                .inc();

                            // Emit state change event
                            if let Some(ref audit_handle) = audit {
                                audit_handle
                                    .emit(AuditEvent::TicketStateChanged {
                                        ticket_id: ticket.id.clone(),
                                        from_state: state_type.to_string(),
                                        to_state: "pending_retry".to_string(),
                                        reason: Some(format!(
                                            "Transient error, scheduling retry: {}",
                                            error_msg
                                        )),
                                    })
                                    .await;
                            }
                            continue; // Skip to next ticket
                        }
                    }

                    // Record download failure
                    metrics::DOWNLOADS_FAILED.inc();

                    // Not retryable or max retries exceeded - mark as failed
                    update_and_notify_static(
                        ticket_store,
                        on_update,
                        &ticket.id,
                        TicketState::Failed {
                            error: error_msg.clone(),
                            retryable: true,
                            retry_count: 0,
                            failed_at: Utc::now(),
                        },
                    )?;

                    // Emit state change event
                    if let Some(ref audit_handle) = audit {
                        audit_handle
                            .emit(AuditEvent::TicketStateChanged {
                                ticket_id: ticket.id.clone(),
                                from_state: state_type.to_string(),
                                to_state: "failed".to_string(),
                                reason: Some(error_msg),
                            })
                            .await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check progress of active downloads.
    async fn check_download_progress<C2, P2>(
        ticket_store: &Arc<dyn TicketStore>,
        torrent_client: &Arc<dyn TorrentClient>,
        pipeline: &Arc<PipelineProcessor<C2, P2>>,
        active_downloads: &Arc<RwLock<HashMap<String, ActiveDownload>>>,
        config: &OrchestratorConfig,
        audit: &Option<AuditHandle>,
        on_update: &Option<TicketUpdateCallback>,
    ) -> Result<(), OrchestratorError>
    where
        C2: crate::converter::Converter + 'static,
        P2: crate::placer::Placer + 'static,
    {
        // Collect downloads to check (avoid holding lock during API calls)
        let downloads: Vec<ActiveDownload> = {
            let downloads = active_downloads.read().await;
            downloads.values().cloned().collect()
        };

        for download in downloads {
            let info = match torrent_client.get_torrent(&download.info_hash).await {
                Ok(info) => info,
                Err(e) => {
                    // Check if this is a "torrent not found" error (torrent was deleted externally)
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("not found")
                        || error_str.contains("404")
                        || error_str.contains("no such")
                    {
                        warn!(
                            "Torrent {} was deleted externally for ticket {}, triggering failover",
                            download.info_hash, download.ticket_id
                        );
                        // Handle this like a stall - try the next candidate
                        if let Err(failover_err) = Self::handle_stall(
                            ticket_store,
                            torrent_client,
                            active_downloads,
                            &download,
                            config,
                            audit,
                            on_update,
                        )
                        .await
                        {
                            warn!(
                                "Failed to handle deleted torrent for ticket {}: {}",
                                download.ticket_id, failover_err
                            );
                        }
                    } else {
                        warn!(
                            "Failed to get torrent {} for ticket {}: {}",
                            download.info_hash, download.ticket_id, e
                        );
                    }
                    continue;
                }
            };

            if info.progress >= 1.0 || info.state == TorrentState::Seeding {
                // Download complete!
                let download_duration = Utc::now().signed_duration_since(download.started_at);
                metrics::DOWNLOADS_COMPLETED.inc();
                metrics::DOWNLOAD_DURATION
                    .with_label_values(&["success"])
                    .observe(download_duration.num_seconds() as f64);

                info!("Download complete for ticket {}", download.ticket_id);

                // Remove from tracking
                {
                    let mut downloads = active_downloads.write().await;
                    downloads.remove(&download.ticket_id);
                }

                // Trigger pipeline
                if let Err(e) =
                    Self::trigger_pipeline(ticket_store, pipeline, &download.ticket_id, &info).await
                {
                    warn!(
                        "Failed to trigger pipeline for ticket {}: {}",
                        download.ticket_id, e
                    );
                    // Update ticket to failed state
                    let error_msg = format!("Failed to start pipeline: {}", e);
                    let _ = update_and_notify_static(
                        &ticket_store,
                        &on_update,
                        &download.ticket_id,
                        TicketState::Failed {
                            error: error_msg.clone(),
                            retryable: true,
                            retry_count: 0,
                            failed_at: Utc::now(),
                        },
                    );

                    // Emit state change event
                    if let Some(ref audit_handle) = audit {
                        audit_handle
                            .emit(AuditEvent::TicketStateChanged {
                                ticket_id: download.ticket_id.clone(),
                                from_state: "downloading".to_string(),
                                to_state: "failed".to_string(),
                                reason: Some(error_msg),
                            })
                            .await;
                    }
                }
            } else {
                let now = Utc::now();
                let current_progress = (info.progress * 100.0) as f32;

                // Check if progress changed
                let (new_last_pct, new_last_at) = if current_progress > download.last_progress_pct {
                    (current_progress, now) // Progress! Reset timer
                } else {
                    (download.last_progress_pct, download.last_progress_at) // No change
                };

                // Check for stall
                let timeout = Self::get_stall_timeout(config, download.failover_round);
                let stall_duration = now.signed_duration_since(new_last_at);

                if stall_duration > chrono::Duration::seconds(timeout as i64) {
                    // STALLED - trigger failover
                    metrics::STALL_DETECTIONS.inc();

                    if let Err(e) = Self::handle_stall(
                        ticket_store,
                        torrent_client,
                        active_downloads,
                        &download,
                        config,
                        audit,
                        on_update,
                    )
                    .await
                    {
                        warn!(
                            "Failed to handle stall for ticket {}: {}",
                            download.ticket_id, e
                        );
                    }
                } else {
                    // Update progress tracking in active downloads
                    {
                        let mut downloads = active_downloads.write().await;
                        if let Some(d) = downloads.get_mut(&download.ticket_id) {
                            d.last_progress_pct = new_last_pct;
                            d.last_progress_at = new_last_at;
                        }
                    }

                    // Get ticket to preserve candidates
                    if let Ok(Some(ticket)) = ticket_store.get(&download.ticket_id) {
                        let candidates = match &ticket.state {
                            TicketState::Downloading { candidates, .. } => candidates.clone(),
                            _ => vec![],
                        };

                        // Update ticket state with new progress
                        let _ = update_and_notify_static(
                            &ticket_store,
                            &on_update,
                            &download.ticket_id,
                            TicketState::Downloading {
                                info_hash: download.info_hash.clone(),
                                progress_pct: current_progress,
                                speed_bps: info.download_speed,
                                eta_secs: info.eta_secs.map(|e| e as u32),
                                started_at: download.started_at,
                                candidate_idx: download.candidate_idx,
                                failover_round: download.failover_round,
                                last_progress_pct: new_last_pct,
                                last_progress_at: new_last_at,
                                candidates,
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Trigger the pipeline for a completed download.
    async fn trigger_pipeline<C2, P2>(
        ticket_store: &Arc<dyn TicketStore>,
        pipeline: &Arc<PipelineProcessor<C2, P2>>,
        ticket_id: &str,
        torrent_info: &TorrentInfo,
    ) -> Result<(), OrchestratorError>
    where
        C2: crate::converter::Converter + 'static,
        P2: crate::placer::Placer + 'static,
    {
        let ticket = ticket_store
            .get(ticket_id)?
            .ok_or_else(|| OrchestratorError::TicketNotFound(ticket_id.to_string()))?;

        // Get file mappings from the selected candidate in ticket state
        let selected = Self::extract_selected_candidate(&ticket)?;

        // Get save path from torrent info
        let save_path = torrent_info
            .save_path
            .as_ref()
            .ok_or_else(|| OrchestratorError::MissingData("save_path not available".to_string()))?;

        // Build source files from download
        // For now, we assume single file or we use the torrent name as directory
        let source_files = vec![SourceFile {
            path: PathBuf::from(save_path).join(&torrent_info.name),
            item_id: "main".to_string(),
            dest_filename: format!("{}.converted", torrent_info.name),
        }];

        // Build pipeline job with file mappings from acquisition
        let job = PipelineJob {
            ticket_id: ticket.id.clone(),
            source_files,
            file_mappings: selected.file_mappings,
            constraints: ticket
                .output_constraints
                .as_ref()
                .and_then(|c| c.to_conversion_constraints()),
            dest_dir: PathBuf::from(&ticket.dest_path),
            metadata: None,
        };

        // Submit to pipeline (non-blocking)
        pipeline.process(job, None).await?;

        info!("Pipeline triggered for ticket {}", ticket_id);

        Ok(())
    }

    /// Add a torrent from a SelectedCandidate, handling both magnet URIs and .torrent URLs.
    async fn add_torrent_from_candidate(
        torrent_client: &dyn TorrentClient,
        candidate: &SelectedCandidate,
    ) -> Result<crate::torrent_client::AddTorrentResult, crate::torrent_client::TorrentClientError>
    {
        // First, try the magnet URI if it looks valid
        if candidate.magnet_uri.starts_with("magnet:") {
            let request = AddTorrentRequest::magnet(&candidate.magnet_uri);
            return torrent_client.add_torrent(request).await;
        }

        // If magnet_uri doesn't start with "magnet:", it might be a .torrent URL
        // Try to download it
        if candidate.magnet_uri.starts_with("http://")
            || candidate.magnet_uri.starts_with("https://")
        {
            debug!(
                "magnet_uri is actually a URL, downloading .torrent file: {}",
                candidate.magnet_uri
            );
            match Self::download_torrent_file(&candidate.magnet_uri).await {
                Ok(data) => {
                    let request = AddTorrentRequest::torrent_file(data);
                    return torrent_client.add_torrent(request).await;
                }
                Err(e) => {
                    warn!("Failed to download .torrent file from magnet_uri: {}", e);
                }
            }
        }

        // Try the dedicated torrent_url if available
        if let Some(ref torrent_url) = candidate.torrent_url {
            debug!("Trying torrent_url: {}", torrent_url);
            match Self::download_torrent_file(torrent_url).await {
                Ok(data) => {
                    let request = AddTorrentRequest::torrent_file(data);
                    return torrent_client.add_torrent(request).await;
                }
                Err(e) => {
                    warn!("Failed to download .torrent file from torrent_url: {}", e);
                }
            }
        }

        // Last resort: construct magnet from info_hash
        if !candidate.info_hash.is_empty() {
            debug!(
                "Constructing magnet URI from info_hash: {}",
                candidate.info_hash
            );
            let constructed_magnet = format!(
                "magnet:?xt=urn:btih:{}&dn={}",
                candidate.info_hash,
                urlencoding::encode(&candidate.title)
            );
            let request = AddTorrentRequest::magnet(&constructed_magnet);
            return torrent_client.add_torrent(request).await;
        }

        Err(crate::torrent_client::TorrentClientError::ApiError(
            "No valid magnet URI, torrent URL, or info hash available".to_string(),
        ))
    }

    /// Download a .torrent file from a URL.
    async fn download_torrent_file(url: &str) -> Result<Vec<u8>, OrchestratorError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| {
                OrchestratorError::MissingData(format!("Failed to create HTTP client: {}", e))
            })?;

        let response = client.get(url).send().await.map_err(|e| {
            OrchestratorError::MissingData(format!("Failed to download .torrent: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(OrchestratorError::MissingData(format!(
                "Failed to download .torrent: HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            OrchestratorError::MissingData(format!("Failed to read .torrent body: {}", e))
        })?;

        Ok(bytes.to_vec())
    }

    /// Build SelectedCandidate from ScoredCandidate.
    fn build_selected_candidate(candidate: &ScoredCandidate) -> SelectedCandidate {
        // Get magnet URI from first source that has one
        let magnet_uri = candidate
            .candidate
            .sources
            .iter()
            .find_map(|s| s.magnet_uri.clone())
            .unwrap_or_else(|| {
                // Build magnet URI from info hash
                format!(
                    "magnet:?xt=urn:btih:{}&dn={}",
                    candidate.candidate.info_hash, candidate.candidate.title
                )
            });

        // Get torrent URL from first source that has one (fallback for when magnet is unavailable)
        let torrent_url = candidate
            .candidate
            .sources
            .iter()
            .find_map(|s| s.torrent_url.clone());

        SelectedCandidate {
            title: candidate.candidate.title.clone(),
            info_hash: candidate.candidate.info_hash.clone(),
            magnet_uri,
            torrent_url,
            size_bytes: candidate.candidate.size_bytes,
            score: candidate.score,
            file_mappings: candidate.file_mappings.clone(),
        }
    }

    /// Extract SelectedCandidate from ticket state.
    fn extract_selected_candidate(ticket: &Ticket) -> Result<SelectedCandidate, OrchestratorError> {
        match &ticket.state {
            TicketState::AutoApproved { selected, .. } => Ok(selected.clone()),
            TicketState::Approved { selected, .. } => Ok(selected.clone()),
            TicketState::Downloading {
                candidates,
                candidate_idx,
                ..
            } => {
                // When triggering pipeline after download, use the current candidate
                candidates.get(*candidate_idx).cloned().ok_or_else(|| {
                    OrchestratorError::MissingData(
                        "No candidate at current index in Downloading state".to_string(),
                    )
                })
            }
            _ => Err(OrchestratorError::InvalidState {
                expected: "AutoApproved, Approved, or Downloading".to_string(),
                actual: ticket.state.state_type().to_string(),
            }),
        }
    }

    /// Extract selected candidate and all candidates for failover from ticket state.
    fn extract_candidates(
        ticket: &Ticket,
    ) -> Result<(SelectedCandidate, Vec<SelectedCandidate>), OrchestratorError> {
        match &ticket.state {
            TicketState::AutoApproved {
                selected,
                candidates,
                ..
            } => {
                // Use stored candidates, or fallback to just the selected one
                let all_candidates = if candidates.is_empty() {
                    vec![selected.clone()]
                } else {
                    candidates.clone()
                };
                Ok((selected.clone(), all_candidates))
            }
            TicketState::Approved {
                selected,
                candidates,
                ..
            } => {
                let all_candidates = if candidates.is_empty() {
                    vec![selected.clone()]
                } else {
                    candidates.clone()
                };
                Ok((selected.clone(), all_candidates))
            }
            _ => Err(OrchestratorError::InvalidState {
                expected: "AutoApproved or Approved".to_string(),
                actual: ticket.state.state_type().to_string(),
            }),
        }
    }

    /// Get stall timeout for the given failover round.
    fn get_stall_timeout(config: &OrchestratorConfig, round: u8) -> u64 {
        match round {
            1 => config.stall_timeout_round1_secs,
            2 => config.stall_timeout_round2_secs,
            _ => config.stall_timeout_round3_secs,
        }
    }

    /// Handle a stalled download by trying the next candidate or failing.
    async fn handle_stall(
        ticket_store: &Arc<dyn TicketStore>,
        torrent_client: &Arc<dyn TorrentClient>,
        active_downloads: &Arc<RwLock<HashMap<String, ActiveDownload>>>,
        download: &ActiveDownload,
        config: &OrchestratorConfig,
        audit: &Option<AuditHandle>,
        on_update: &Option<TicketUpdateCallback>,
    ) -> Result<(), OrchestratorError> {
        // Get ticket to access candidates
        let ticket = ticket_store
            .get(&download.ticket_id)?
            .ok_or_else(|| OrchestratorError::TicketNotFound(download.ticket_id.clone()))?;

        let candidates = match &ticket.state {
            TicketState::Downloading { candidates, .. } => candidates.clone(),
            _ => {
                return Err(OrchestratorError::InvalidState {
                    expected: "Downloading".to_string(),
                    actual: ticket.state.state_type().to_string(),
                });
            }
        };

        let num_candidates = candidates.len();
        if num_candidates == 0 {
            // No candidates to failover to - fail immediately
            let error_msg = "Download stalled: no candidates available".to_string();
            active_downloads.write().await.remove(&download.ticket_id);
            let _ = torrent_client
                .remove_torrent(&download.info_hash, false)
                .await;
            update_and_notify_static(
                &ticket_store,
                &on_update,
                &download.ticket_id,
                TicketState::Failed {
                    error: error_msg.clone(),
                    retryable: false,
                    retry_count: 0,
                    failed_at: Utc::now(),
                },
            )?;

            // Emit state change event
            if let Some(ref audit_handle) = audit {
                audit_handle
                    .emit(AuditEvent::TicketStateChanged {
                        ticket_id: download.ticket_id.clone(),
                        from_state: "downloading".to_string(),
                        to_state: "failed".to_string(),
                        reason: Some(error_msg),
                    })
                    .await;
            }

            return Ok(());
        }

        // Calculate next candidate and round
        let mut next_idx = download.candidate_idx + 1;
        let mut next_round = download.failover_round;

        if next_idx >= num_candidates {
            next_idx = 0;
            next_round += 1;
        }

        // Check if we've exhausted all rounds
        if next_round > 3 {
            // All candidates tried in all rounds - fail permanently
            let error_msg = format!(
                "Download stalled: tried {} candidates over 3 rounds (~{} hours)",
                num_candidates,
                (config.stall_timeout_round1_secs * num_candidates as u64
                    + config.stall_timeout_round2_secs * num_candidates as u64
                    + config.stall_timeout_round3_secs * num_candidates as u64)
                    / 3600
            );
            active_downloads.write().await.remove(&download.ticket_id);
            let _ = torrent_client
                .remove_torrent(&download.info_hash, false)
                .await;

            update_and_notify_static(
                &ticket_store,
                &on_update,
                &download.ticket_id,
                TicketState::Failed {
                    error: error_msg.clone(),
                    retryable: false,
                    retry_count: 0,
                    failed_at: Utc::now(),
                },
            )?;

            // Emit state change event
            if let Some(ref audit_handle) = audit {
                audit_handle
                    .emit(AuditEvent::TicketStateChanged {
                        ticket_id: download.ticket_id.clone(),
                        from_state: "downloading".to_string(),
                        to_state: "failed".to_string(),
                        reason: Some(error_msg),
                    })
                    .await;
            }

            info!(
                "Ticket {} failed after exhausting all failover attempts",
                download.ticket_id
            );
            return Ok(());
        }

        // Remove current stalled torrent
        let _ = torrent_client
            .remove_torrent(&download.info_hash, false)
            .await;

        // Try next candidate
        metrics::FAILOVER_ATTEMPTS.inc();

        let next_candidate = &candidates[next_idx];
        info!(
            "Ticket {}: stall detected (round {}), failing over to candidate {} of {}",
            download.ticket_id,
            download.failover_round,
            next_idx + 1,
            num_candidates
        );

        // Add new torrent
        let request = AddTorrentRequest::magnet(&next_candidate.magnet_uri);
        match torrent_client.add_torrent(request).await {
            Ok(result) => {
                let now = Utc::now();

                // Update tracking
                {
                    let mut downloads = active_downloads.write().await;
                    downloads.insert(
                        download.ticket_id.clone(),
                        ActiveDownload {
                            ticket_id: download.ticket_id.clone(),
                            info_hash: result.hash.clone(),
                            started_at: now,
                            candidate_idx: next_idx,
                            failover_round: next_round,
                            last_progress_pct: 0.0,
                            last_progress_at: now,
                        },
                    );
                }

                // Update ticket state
                update_and_notify_static(
                    &ticket_store,
                    &on_update,
                    &download.ticket_id,
                    TicketState::Downloading {
                        info_hash: result.hash,
                        progress_pct: 0.0,
                        speed_bps: 0,
                        eta_secs: None,
                        started_at: now,
                        candidate_idx: next_idx,
                        failover_round: next_round,
                        last_progress_pct: 0.0,
                        last_progress_at: now,
                        candidates,
                    },
                )?;
            }
            Err(e) => {
                // Failed to add the next candidate - try the one after that
                warn!(
                    "Failed to add failover torrent for candidate {}: {}",
                    next_idx, e
                );
                // Update download tracking to skip this candidate
                {
                    let mut downloads = active_downloads.write().await;
                    if let Some(d) = downloads.get_mut(&download.ticket_id) {
                        d.candidate_idx = next_idx;
                        d.failover_round = next_round;
                        // Reset stall timer to trigger immediate retry of next candidate
                        d.last_progress_at = Utc::now()
                            - chrono::Duration::seconds(
                                Self::get_stall_timeout(config, next_round) as i64 + 1,
                            );
                    }
                }
            }
        }

        Ok(())
    }

    /// Build a TextBrain instance with appropriate query builder and matcher
    /// based on the configuration.
    fn build_textbrain(config: &TextBrainConfig, catalog: Arc<dyn TorrentCatalog>) -> TextBrain {
        let mut textbrain = TextBrain::new(config.clone());

        // Always add dumb implementations (used as fallback in most modes)
        if config.mode.can_use_dumb() {
            textbrain = textbrain
                .with_dumb_query_builder(Arc::new(DumbQueryBuilder::new()))
                .with_dumb_matcher(Arc::new(DumbMatcher::new()));
        }

        // Add LLM implementations if configured and mode can use them
        if config.mode.can_use_llm() {
            if let Some(ref llm_config) = config.llm {
                match llm_config.provider {
                    LlmProvider::Anthropic => {
                        if let Some(ref api_key) = llm_config.api_key {
                            let mut client =
                                AnthropicClient::new(api_key.clone(), llm_config.model.clone());
                            if let Some(ref api_base) = llm_config.api_base {
                                client = client.with_api_base(api_base.clone());
                            }
                            let client = Arc::new(client);
                            textbrain = textbrain
                                .with_llm_query_builder(Arc::new(LlmQueryBuilder::new(
                                    client.clone(),
                                )))
                                .with_llm_matcher(Arc::new(LlmMatcher::new(client)));
                            info!(
                                "LLM integration enabled with Anthropic ({})",
                                llm_config.model
                            );
                        } else {
                            warn!("Anthropic provider configured but no API key provided");
                        }
                    }
                    LlmProvider::Ollama => {
                        let mut client = OllamaClient::new(llm_config.model.clone());
                        if let Some(ref api_base) = llm_config.api_base {
                            client = client.with_api_base(api_base.clone());
                        }
                        let client = Arc::new(client);
                        textbrain = textbrain
                            .with_llm_query_builder(Arc::new(LlmQueryBuilder::new(client.clone())))
                            .with_llm_matcher(Arc::new(LlmMatcher::new(client)));
                        info!("LLM integration enabled with Ollama ({})", llm_config.model);
                    }
                    LlmProvider::OpenAi | LlmProvider::Custom => {
                        // TODO: Implement OpenAI and custom providers
                        warn!(
                            "LLM provider {:?} is not yet implemented, falling back to heuristics",
                            llm_config.provider
                        );
                    }
                }
            } else if config.mode.requires_llm() {
                warn!("LLM mode requires LLM configuration but none provided");
            }
        }

        // Add file enricher if enabled
        if config.file_enrichment.enabled {
            let enricher = FileEnricher::new(catalog, config.file_enrichment.clone());
            textbrain = textbrain.with_file_enricher(Arc::new(enricher));
            info!(
                "File enrichment enabled (max_candidates={}, min_score={})",
                config.file_enrichment.max_candidates, config.file_enrichment.min_score_threshold
            );
        }

        textbrain
    }

    /// Check if an error is retryable (transient) or permanent.
    ///
    /// Transient errors that can be retried:
    /// - Network/connection errors
    /// - Timeouts
    /// - Rate limiting (429)
    /// - Server errors (5xx)
    /// - API unavailable
    ///
    /// Permanent errors that should not be retried:
    /// - "No candidates found" type errors
    /// - Invalid configuration
    /// - Authentication failures
    fn is_retryable_error(error: &str) -> bool {
        let error_lower = error.to_lowercase();

        // Patterns indicating transient/retryable errors
        let retryable_patterns = [
            "timeout",
            "timed out",
            "connection refused",
            "connection reset",
            "connection closed",
            "network",
            "dns",
            "temporary",
            "unavailable",
            "service unavailable",
            "too many requests",
            "rate limit",
            "429",
            "500",
            "502",
            "503",
            "504",
            "internal server error",
            "bad gateway",
            "gateway timeout",
            "econnrefused",
            "econnreset",
            "etimedout",
            "ehostunreach",
            "enetunreach",
        ];

        // Patterns indicating permanent errors (don't retry)
        let permanent_patterns = [
            "no candidates",
            "no suitable",
            "not found",
            "invalid",
            "unauthorized",
            "forbidden",
            "authentication",
            "401",
            "403",
            "404",
        ];

        // Check for permanent errors first
        for pattern in permanent_patterns {
            if error_lower.contains(pattern) {
                return false;
            }
        }

        // Check for retryable patterns
        for pattern in retryable_patterns {
            if error_lower.contains(pattern) {
                return true;
            }
        }

        // Default: don't retry unknown errors
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockConverter, MockPlacer};

    type TestOrchestrator = TicketOrchestrator<MockConverter, MockPlacer>;

    #[test]
    fn test_orchestrator_status_default() {
        let status = OrchestratorStatus::default();
        assert!(!status.running);
        assert_eq!(status.active_downloads, 0);
    }

    // ========================================================================
    // is_retryable_error tests
    // ========================================================================

    #[test]
    fn test_retryable_network_errors() {
        assert!(TestOrchestrator::is_retryable_error("Connection refused"));
        assert!(TestOrchestrator::is_retryable_error(
            "connection reset by peer"
        ));
        assert!(TestOrchestrator::is_retryable_error("Network unreachable"));
        assert!(TestOrchestrator::is_retryable_error("DNS lookup failed"));
        assert!(TestOrchestrator::is_retryable_error("Request timeout"));
        assert!(TestOrchestrator::is_retryable_error("Operation timed out"));
    }

    #[test]
    fn test_retryable_server_errors() {
        assert!(TestOrchestrator::is_retryable_error(
            "HTTP 500 Internal Server Error"
        ));
        assert!(TestOrchestrator::is_retryable_error("502 Bad Gateway"));
        assert!(TestOrchestrator::is_retryable_error(
            "503 Service Unavailable"
        ));
        assert!(TestOrchestrator::is_retryable_error("504 Gateway Timeout"));
        assert!(TestOrchestrator::is_retryable_error(
            "Service temporarily unavailable"
        ));
    }

    #[test]
    fn test_retryable_rate_limit_errors() {
        assert!(TestOrchestrator::is_retryable_error(
            "429 Too Many Requests"
        ));
        assert!(TestOrchestrator::is_retryable_error("Rate limit exceeded"));
        assert!(TestOrchestrator::is_retryable_error(
            "Too many requests, please slow down"
        ));
    }

    #[test]
    fn test_permanent_not_found_errors() {
        assert!(!TestOrchestrator::is_retryable_error("No candidates found"));
        assert!(!TestOrchestrator::is_retryable_error(
            "No suitable candidates found"
        ));
        assert!(!TestOrchestrator::is_retryable_error("404 Not Found"));
    }

    #[test]
    fn test_permanent_auth_errors() {
        assert!(!TestOrchestrator::is_retryable_error("401 Unauthorized"));
        assert!(!TestOrchestrator::is_retryable_error("403 Forbidden"));
        assert!(!TestOrchestrator::is_retryable_error(
            "Authentication failed"
        ));
    }

    #[test]
    fn test_permanent_invalid_errors() {
        assert!(!TestOrchestrator::is_retryable_error(
            "Invalid torrent file"
        ));
        assert!(!TestOrchestrator::is_retryable_error(
            "Invalid configuration"
        ));
    }

    #[test]
    fn test_unknown_errors_not_retryable() {
        assert!(!TestOrchestrator::is_retryable_error("Some unknown error"));
        assert!(!TestOrchestrator::is_retryable_error("Unexpected failure"));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(TestOrchestrator::is_retryable_error("CONNECTION REFUSED"));
        assert!(TestOrchestrator::is_retryable_error("Timeout"));
        assert!(TestOrchestrator::is_retryable_error("ECONNRESET"));
    }
}
