use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use super::AuditEvent;

/// Envelope wrapping an audit event with metadata
#[derive(Debug, Clone)]
pub struct AuditEventEnvelope {
    pub timestamp: DateTime<Utc>,
    pub event: AuditEvent,
}

/// Handle for emitting audit events
///
/// This is cheaply cloneable and can be shared across tasks.
/// Events are sent through an async channel to be written by the AuditWriter.
#[derive(Clone)]
pub struct AuditHandle {
    tx: mpsc::Sender<AuditEventEnvelope>,
}

impl AuditHandle {
    /// Create a new audit handle from a channel sender
    pub fn new(tx: mpsc::Sender<AuditEventEnvelope>) -> Self {
        Self { tx }
    }

    /// Emit an audit event asynchronously
    ///
    /// This is non-blocking. If the channel is full or closed, the error is logged
    /// but the caller is not blocked or failed.
    pub async fn emit(&self, event: AuditEvent) {
        let envelope = AuditEventEnvelope {
            timestamp: Utc::now(),
            event,
        };
        if let Err(e) = self.tx.send(envelope).await {
            tracing::error!("Failed to emit audit event: {}", e);
        }
    }

    /// Emit an audit event synchronously (blocking)
    ///
    /// Use this in contexts where async isn't available.
    /// If the channel is full or closed, the error is logged but the caller is not failed.
    pub fn emit_blocking(&self, event: AuditEvent) {
        let envelope = AuditEventEnvelope {
            timestamp: Utc::now(),
            event,
        };
        if let Err(e) = self.tx.blocking_send(envelope) {
            tracing::error!("Failed to emit audit event: {}", e);
        }
    }

    /// Try to emit an audit event without blocking
    ///
    /// Returns true if the event was sent successfully, false otherwise.
    pub fn try_emit(&self, event: AuditEvent) -> bool {
        let envelope = AuditEventEnvelope {
            timestamp: Utc::now(),
            event,
        };
        match self.tx.try_send(envelope) {
            Ok(()) => true,
            Err(e) => {
                tracing::error!("Failed to emit audit event: {}", e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_emit_event() {
        let (tx, mut rx) = mpsc::channel(10);
        let handle = AuditHandle::new(tx);

        handle
            .emit(AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc123".to_string(),
            })
            .await;

        let envelope = rx.recv().await.expect("Should receive event");
        assert!(matches!(envelope.event, AuditEvent::ServiceStarted { .. }));
    }

    #[tokio::test]
    async fn test_multiple_handles_same_channel() {
        let (tx, mut rx) = mpsc::channel(10);
        let handle1 = AuditHandle::new(tx.clone());
        let handle2 = AuditHandle::new(tx);

        handle1
            .emit(AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc".to_string(),
            })
            .await;

        handle2
            .emit(AuditEvent::ServiceStopped {
                reason: "test".to_string(),
            })
            .await;

        let e1 = rx.recv().await.expect("Should receive first event");
        let e2 = rx.recv().await.expect("Should receive second event");

        assert!(matches!(e1.event, AuditEvent::ServiceStarted { .. }));
        assert!(matches!(e2.event, AuditEvent::ServiceStopped { .. }));
    }

    #[test]
    fn test_try_emit() {
        let (tx, mut rx) = mpsc::channel(10);
        let handle = AuditHandle::new(tx);

        let result = handle.try_emit(AuditEvent::ServiceStarted {
            version: "0.1.0".to_string(),
            config_hash: "abc123".to_string(),
        });

        assert!(result);

        let envelope = rx.try_recv().expect("Should receive event");
        assert!(matches!(envelope.event, AuditEvent::ServiceStarted { .. }));
    }

    #[test]
    fn test_try_emit_full_channel() {
        let (tx, _rx) = mpsc::channel(1);
        let handle = AuditHandle::new(tx);

        // First should succeed
        let result1 = handle.try_emit(AuditEvent::ServiceStarted {
            version: "0.1.0".to_string(),
            config_hash: "abc".to_string(),
        });
        assert!(result1);

        // Second should fail (channel full)
        let result2 = handle.try_emit(AuditEvent::ServiceStopped {
            reason: "test".to_string(),
        });
        assert!(!result2);
    }

    #[tokio::test]
    async fn test_emit_closed_channel() {
        let (tx, rx) = mpsc::channel::<AuditEventEnvelope>(10);
        let handle = AuditHandle::new(tx);

        // Drop the receiver to close the channel
        drop(rx);

        // This should not panic, just log an error
        handle
            .emit(AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc123".to_string(),
            })
            .await;
    }

    #[test]
    fn test_envelope_has_timestamp() {
        let (tx, mut rx) = mpsc::channel(10);
        let handle = AuditHandle::new(tx);

        let before = Utc::now();
        handle.try_emit(AuditEvent::ServiceStarted {
            version: "0.1.0".to_string(),
            config_hash: "abc123".to_string(),
        });
        let after = Utc::now();

        let envelope = rx.try_recv().expect("Should receive event");
        assert!(envelope.timestamp >= before);
        assert!(envelope.timestamp <= after);
    }
}
