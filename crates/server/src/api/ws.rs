//! WebSocket support for real-time dashboard updates.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::metrics::{WS_CONNECTIONS_ACTIVE, WS_CONNECTIONS_TOTAL, WS_LAG_EVENTS, WS_MESSAGES_SENT};
use crate::state::AppState;

/// WebSocket message sent to clients for real-time updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// A ticket was created or updated.
    TicketUpdate {
        ticket_id: String,
        /// The new state type (e.g., "pending", "downloading", "completed")
        state: String,
    },
    /// A ticket was deleted.
    TicketDeleted { ticket_id: String },
    /// Torrent progress update (sent periodically for active downloads).
    TorrentProgress {
        ticket_id: String,
        info_hash: String,
        progress_pct: f32,
        speed_bps: u64,
        eta_secs: Option<u64>,
    },
    /// Pipeline progress update (conversion/placement).
    PipelineProgress {
        ticket_id: String,
        phase: String, // "converting" or "placing"
        current: usize,
        total: usize,
        current_name: String,
        /// FFmpeg conversion percentage (0.0 - 100.0) for current file
        percent: f32,
    },
    /// Orchestrator status changed.
    OrchestratorStatus { running: bool },
    /// Server heartbeat (sent periodically to keep connection alive).
    Heartbeat { timestamp: i64 },
}

/// Broadcaster for WebSocket messages using tokio broadcast channel.
#[derive(Debug, Clone)]
pub struct WsBroadcaster {
    sender: broadcast::Sender<WsMessage>,
}

impl WsBroadcaster {
    /// Create a new broadcaster with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast a message to all connected clients.
    pub fn broadcast(&self, msg: WsMessage) {
        // Ignore send errors - they just mean no one is listening
        let _ = self.sender.send(msg);
    }

    /// Subscribe to receive messages.
    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.sender.subscribe()
    }

    /// Convenience method to broadcast a ticket update.
    pub fn ticket_updated(&self, ticket_id: &str, state: &str) {
        self.broadcast(WsMessage::TicketUpdate {
            ticket_id: ticket_id.to_string(),
            state: state.to_string(),
        });
    }

    /// Convenience method to broadcast a ticket deletion.
    pub fn ticket_deleted(&self, ticket_id: &str) {
        self.broadcast(WsMessage::TicketDeleted {
            ticket_id: ticket_id.to_string(),
        });
    }

    /// Convenience method to broadcast torrent progress.
    pub fn torrent_progress(
        &self,
        ticket_id: &str,
        info_hash: &str,
        progress_pct: f32,
        speed_bps: u64,
        eta_secs: Option<u64>,
    ) {
        self.broadcast(WsMessage::TorrentProgress {
            ticket_id: ticket_id.to_string(),
            info_hash: info_hash.to_string(),
            progress_pct,
            speed_bps,
            eta_secs,
        });
    }

    /// Convenience method to broadcast pipeline progress.
    pub fn pipeline_progress(
        &self,
        ticket_id: &str,
        phase: &str,
        current: usize,
        total: usize,
        current_name: &str,
        percent: f32,
    ) {
        self.broadcast(WsMessage::PipelineProgress {
            ticket_id: ticket_id.to_string(),
            phase: phase.to_string(),
            current,
            total,
            current_name: current_name.to_string(),
            percent,
        });
    }

    /// Convenience method to broadcast orchestrator status.
    pub fn orchestrator_status(&self, running: bool) {
        self.broadcast(WsMessage::OrchestratorStatus { running });
    }
}

impl Default for WsBroadcaster {
    fn default() -> Self {
        Self::new(256)
    }
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast messages
    let mut rx = state.ws_broadcaster().subscribe();

    // Track connection metrics
    WS_CONNECTIONS_TOTAL.inc();
    WS_CONNECTIONS_ACTIVE.inc();

    info!("WebSocket client connected");

    // Spawn task to forward broadcast messages to this client
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Forward broadcast messages to client
                result = rx.recv() => {
                    match result {
                        Ok(msg) => {
                            // Track message by type
                            let msg_type = match &msg {
                                WsMessage::TicketUpdate { .. } => "ticket_update",
                                WsMessage::TicketDeleted { .. } => "ticket_deleted",
                                WsMessage::TorrentProgress { .. } => "torrent_progress",
                                WsMessage::PipelineProgress { .. } => "pipeline_progress",
                                WsMessage::OrchestratorStatus { .. } => "orchestrator_status",
                                WsMessage::Heartbeat { .. } => "heartbeat",
                            };
                            WS_MESSAGES_SENT.with_label_values(&[msg_type]).inc();

                            match serde_json::to_string(&msg) {
                                Ok(json) => {
                                    if sender.send(Message::Text(json.into())).await.is_err() {
                                        debug!("WebSocket send failed, client disconnected");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize WsMessage: {}", e);
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("WebSocket client lagged, skipped {} messages", n);
                            WS_LAG_EVENTS.inc();
                            // Continue receiving - the client will catch up
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            debug!("Broadcast channel closed");
                            break;
                        }
                    }
                }
            }
        }
    });

    // Handle incoming messages from client (ping/pong, close)
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Close(_)) => {
                debug!("WebSocket client requested close");
                break;
            }
            Ok(Message::Ping(data)) => {
                // Pong is handled automatically by axum
                debug!("Received ping: {:?}", data);
            }
            Ok(Message::Text(text)) => {
                // We don't expect any client messages, but log them
                debug!("Received text message: {}", text);
            }
            Ok(_) => {
                // Ignore other message types
            }
            Err(e) => {
                warn!("WebSocket receive error: {}", e);
                break;
            }
        }
    }

    // Clean up
    send_task.abort();
    WS_CONNECTIONS_ACTIVE.dec();
    info!("WebSocket client disconnected");
}
