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
use crate::converter::ConversionConstraints;
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
                        ..
                    } = &ticket.state
                    {
                        downloads.insert(
                            ticket.id.clone(),
                            ActiveDownload {
                                ticket_id: ticket.id.clone(),
                                info_hash: info_hash.clone(),
                                started_at: *started_at,
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
                        let selected = Self::build_selected_candidate(candidate);
                        ticket_store.update_state(
                            &ticket.id,
                            TicketState::AutoApproved {
                                selected,
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

                // Extract magnet URI from state
                let selected = Self::extract_selected_candidate(&ticket)?;

                // Add to torrent client
                let request = AddTorrentRequest::magnet(&selected.magnet_uri);
                match torrent_client.add_torrent(request).await {
                    Ok(result) => {
                        // Track active download
                        let now = Utc::now();
                        {
                            let mut downloads = active_downloads.write().await;
                            downloads.insert(
                                ticket.id.clone(),
                                ActiveDownload {
                                    ticket_id: ticket.id.clone(),
                                    info_hash: result.hash.clone(),
                                    started_at: now,
                                },
                            );
                        }

                        // Update ticket state
                        ticket_store.update_state(
                            &ticket.id,
                            TicketState::Downloading {
                                info_hash: result.hash.clone(),
                                progress_pct: 0.0,
                                speed_bps: 0,
                                eta_secs: None,
                                started_at: now,
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
                // Update progress in ticket state
                let _ = ticket_store.update_state(
                    &download.ticket_id,
                    TicketState::Downloading {
                        info_hash: download.info_hash.clone(),
                        progress_pct: (info.progress * 100.0) as f32,
                        speed_bps: info.download_speed,
                        eta_secs: info.eta_secs.map(|e| e as u32),
                        started_at: download.started_at,
                    },
                );
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
            constraints: ConversionConstraints::default(), // TODO: From config/ticket
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
