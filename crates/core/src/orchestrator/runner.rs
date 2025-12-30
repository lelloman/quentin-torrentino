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

use chrono::Utc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use crate::audit::AuditHandle;
use crate::processor::{PipelineJob, PipelineProcessor, SourceFile};
use crate::searcher::Searcher;
use crate::textbrain::{
    DumbMatcher, DumbQueryBuilder, ScoredCandidate, ScoredCandidateSummary, TextBrain,
    TextBrainConfig,
};
use crate::ticket::{
    AcquisitionPhase, SelectedCandidate, Ticket, TicketFilter, TicketState, TicketStore,
};
use crate::torrent_client::{AddTorrentRequest, TorrentClient, TorrentInfo, TorrentState};

use super::config::OrchestratorConfig;
use super::types::{ActiveDownload, OrchestratorError, OrchestratorStatus};

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
    #[allow(dead_code)]
    audit: Option<AuditHandle>,
    textbrain_config: TextBrainConfig,

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
            audit,
            textbrain_config,
            running: Arc::new(AtomicBool::new(false)),
            active_downloads: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
        }
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
        let filter = TicketFilter::new().with_state("downloading").with_limit(100);

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
        let config = self.config.clone();
        let textbrain_config = self.textbrain_config.clone();
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
                            &config,
                            &textbrain_config,
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
                        ).await {
                            warn!("Failed to check downloads: {}", e);
                        }
                    }
                }
            }
            info!("Download monitor loop stopped");
        });
    }

    /// Process one pending ticket (acquisition).
    async fn process_one_pending(
        ticket_store: &Arc<dyn TicketStore>,
        searcher: &Arc<dyn Searcher>,
        config: &OrchestratorConfig,
        textbrain_config: &TextBrainConfig,
    ) -> Result<(), OrchestratorError> {
        // Get highest priority pending ticket
        let filter = TicketFilter::new().with_state("pending").with_limit(1);
        let tickets = ticket_store.list(&filter)?;

        let Some(ticket) = tickets.first() else {
            return Ok(()); // Nothing to do
        };

        debug!("Processing pending ticket: {}", ticket.id);

        // Transition to Acquiring
        ticket_store.update_state(
            &ticket.id,
            TicketState::Acquiring {
                started_at: Utc::now(),
                queries_tried: vec![],
                candidates_found: 0,
                phase: AcquisitionPhase::QueryBuilding,
            },
        )?;

        // Build TextBrain with dumb implementations
        let textbrain = TextBrain::new(textbrain_config.clone())
            .with_dumb_query_builder(Arc::new(DumbQueryBuilder::new()))
            .with_dumb_matcher(Arc::new(DumbMatcher::new()));

        // Execute acquisition
        let result = textbrain
            .acquire(&ticket.query_context, searcher.as_ref())
            .await;

        match result {
            Ok(acq) => {
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
                        ticket_store.update_state(
                            &ticket.id,
                            TicketState::AutoApproved {
                                selected,
                                candidates,
                                confidence: candidate.score,
                                approved_at: Utc::now(),
                            },
                        )?;
                        info!(
                            "Ticket {} auto-approved with score {:.2}",
                            ticket.id, candidate.score
                        );
                    } else {
                        // No candidate found
                        ticket_store.update_state(
                            &ticket.id,
                            TicketState::AcquisitionFailed {
                                queries_tried: acq.queries_tried,
                                candidates_seen: acq.candidates_evaluated,
                                reason: "No candidates found".to_string(),
                                failed_at: Utc::now(),
                            },
                        )?;
                    }
                } else if let Some(ref candidate) = acq.best_candidate {
                    // Needs manual approval - below threshold
                    let summaries: Vec<ScoredCandidateSummary> = acq
                        .all_candidates
                        .iter()
                        .take(5) // Top 5 candidates for review
                        .map(ScoredCandidateSummary::from)
                        .collect();

                    ticket_store.update_state(
                        &ticket.id,
                        TicketState::NeedsApproval {
                            candidates: summaries,
                            recommended_idx: 0,
                            confidence: candidate.score,
                            waiting_since: Utc::now(),
                        },
                    )?;
                    info!(
                        "Ticket {} needs approval, best score {:.2} < threshold {:.2}",
                        ticket.id, candidate.score, config.auto_approve_threshold
                    );
                } else {
                    // No candidate found
                    ticket_store.update_state(
                        &ticket.id,
                        TicketState::AcquisitionFailed {
                            queries_tried: acq.queries_tried,
                            candidates_seen: acq.candidates_evaluated,
                            reason: "No suitable candidates found".to_string(),
                            failed_at: Utc::now(),
                        },
                    )?;
                }
            }
            Err(e) => {
                ticket_store.update_state(
                    &ticket.id,
                    TicketState::AcquisitionFailed {
                        queries_tried: vec![],
                        candidates_seen: 0,
                        reason: e.to_string(),
                        failed_at: Utc::now(),
                    },
                )?;
                warn!("Acquisition failed for ticket {}: {}", ticket.id, e);
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
                let (selected, candidates) = Self::extract_candidates(&ticket)?;

                // Add to torrent client
                let request = AddTorrentRequest::magnet(&selected.magnet_uri);
                match torrent_client.add_torrent(request).await {
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
                                    candidate_idx: 0,
                                    failover_round: 1,
                                    last_progress_pct: 0.0,
                                    last_progress_at: now,
                                },
                            );
                        }

                        // Update ticket state with failover fields
                        ticket_store.update_state(
                            &ticket.id,
                            TicketState::Downloading {
                                info_hash: result.hash.clone(),
                                progress_pct: 0.0,
                                speed_bps: 0,
                                eta_secs: None,
                                started_at: now,
                                candidate_idx: 0,
                                failover_round: 1,
                                last_progress_pct: 0.0,
                                last_progress_at: now,
                                candidates,
                            },
                        )?;

                        info!(
                            "Started download for ticket {}: {}",
                            ticket.id, result.hash
                        );
                    }
                    Err(e) => {
                        warn!("Failed to add torrent for ticket {}: {}", ticket.id, e);
                        ticket_store.update_state(
                            &ticket.id,
                            TicketState::Failed {
                                error: format!("Failed to add torrent: {}", e),
                                retryable: true,
                                retry_count: 0,
                                failed_at: Utc::now(),
                            },
                        )?;
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
                    warn!(
                        "Failed to get torrent {} for ticket {}: {}",
                        download.info_hash, download.ticket_id, e
                    );
                    continue;
                }
            };

            if info.progress >= 1.0 || info.state == TorrentState::Seeding {
                // Download complete!
                info!("Download complete for ticket {}", download.ticket_id);

                // Remove from tracking
                {
                    let mut downloads = active_downloads.write().await;
                    downloads.remove(&download.ticket_id);
                }

                // Trigger pipeline
                if let Err(e) = Self::trigger_pipeline(
                    ticket_store,
                    pipeline,
                    &download.ticket_id,
                    &info,
                )
                .await
                {
                    warn!(
                        "Failed to trigger pipeline for ticket {}: {}",
                        download.ticket_id, e
                    );
                    // Update ticket to failed state
                    let _ = ticket_store.update_state(
                        &download.ticket_id,
                        TicketState::Failed {
                            error: format!("Failed to start pipeline: {}", e),
                            retryable: true,
                            retry_count: 0,
                            failed_at: Utc::now(),
                        },
                    );
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
                    if let Err(e) = Self::handle_stall(
                        ticket_store,
                        torrent_client,
                        active_downloads,
                        &download,
                        config,
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
                        let _ = ticket_store.update_state(
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

    /// Build SelectedCandidate from ScoredCandidate.
    fn build_selected_candidate(candidate: &ScoredCandidate) -> SelectedCandidate {
        // Get magnet URI from first source
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

        SelectedCandidate {
            title: candidate.candidate.title.clone(),
            info_hash: candidate.candidate.info_hash.clone(),
            magnet_uri,
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
            _ => Err(OrchestratorError::InvalidState {
                expected: "AutoApproved or Approved".to_string(),
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
            active_downloads.write().await.remove(&download.ticket_id);
            let _ = torrent_client
                .remove_torrent(&download.info_hash, false)
                .await;
            ticket_store.update_state(
                &download.ticket_id,
                TicketState::Failed {
                    error: "Download stalled: no candidates available".to_string(),
                    retryable: false,
                    retry_count: 0,
                    failed_at: Utc::now(),
                },
            )?;
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
            active_downloads.write().await.remove(&download.ticket_id);
            let _ = torrent_client
                .remove_torrent(&download.info_hash, false)
                .await;

            ticket_store.update_state(
                &download.ticket_id,
                TicketState::Failed {
                    error: format!(
                        "Download stalled: tried {} candidates over 3 rounds (~{} hours)",
                        num_candidates,
                        (config.stall_timeout_round1_secs * num_candidates as u64
                            + config.stall_timeout_round2_secs * num_candidates as u64
                            + config.stall_timeout_round3_secs * num_candidates as u64)
                            / 3600
                    ),
                    retryable: false,
                    retry_count: 0,
                    failed_at: Utc::now(),
                },
            )?;

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
                ticket_store.update_state(
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_status_default() {
        let status = OrchestratorStatus::default();
        assert!(!status.running);
        assert_eq!(status.active_downloads, 0);
    }
}
