use std::sync::Arc;

use tokio::sync::mpsc;

use super::{AuditEventEnvelope, AuditHandle, AuditRecord, AuditStore};

/// Background task that receives audit events and writes them to storage
pub struct AuditWriter {
    rx: mpsc::Receiver<AuditEventEnvelope>,
    store: Arc<dyn AuditStore>,
}

impl AuditWriter {
    /// Create a new audit writer
    pub fn new(rx: mpsc::Receiver<AuditEventEnvelope>, store: Arc<dyn AuditStore>) -> Self {
        Self { rx, store }
    }

    /// Run the writer, consuming events until the channel is closed
    ///
    /// This should be spawned as a background task.
    pub async fn run(mut self) {
        tracing::info!("Audit writer started");

        while let Some(envelope) = self.rx.recv().await {
            let record = AuditRecord {
                id: 0, // Will be set by database
                timestamp: envelope.timestamp,
                event_type: envelope.event.event_type().to_string(),
                ticket_id: envelope.event.ticket_id().map(String::from),
                user_id: envelope.event.user_id().map(String::from),
                data: envelope.event,
            };

            if let Err(e) = self.store.insert(&record) {
                tracing::error!("Failed to write audit event: {}", e);
            }
        }

        tracing::info!("Audit writer shutting down");
    }
}

/// Create a complete audit system
///
/// Returns:
/// - `AuditHandle` - for emitting events (clone this to share across tasks)
/// - `AuditWriter` - spawn this as a background task with `tokio::spawn(writer.run())`
///
/// # Arguments
/// * `store` - The audit store to write events to
/// * `buffer_size` - Size of the channel buffer (events will block if full)
pub fn create_audit_system(
    store: Arc<dyn AuditStore>,
    buffer_size: usize,
) -> (AuditHandle, AuditWriter) {
    let (tx, rx) = mpsc::channel(buffer_size);
    let handle = AuditHandle::new(tx);
    let writer = AuditWriter::new(rx, store);
    (handle, writer)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::audit::{AuditError, AuditEvent, AuditFilter};

    /// Mock store that records insert calls
    struct MockStore {
        records: Mutex<Vec<AuditRecord>>,
        should_fail: bool,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                records: Mutex::new(Vec::new()),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                records: Mutex::new(Vec::new()),
                should_fail: true,
            }
        }

        fn get_records(&self) -> Vec<AuditRecord> {
            self.records.lock().unwrap().clone()
        }
    }

    impl AuditStore for MockStore {
        fn insert(&self, record: &AuditRecord) -> Result<i64, AuditError> {
            if self.should_fail {
                return Err(AuditError::Database("Mock failure".to_string()));
            }
            let mut records = self.records.lock().unwrap();
            let id = records.len() as i64 + 1;
            let mut stored = record.clone();
            stored.id = id;
            records.push(stored);
            Ok(id)
        }

        fn query(&self, _filter: &AuditFilter) -> Result<Vec<AuditRecord>, AuditError> {
            Ok(self.records.lock().unwrap().clone())
        }

        fn count(&self, _filter: &AuditFilter) -> Result<i64, AuditError> {
            Ok(self.records.lock().unwrap().len() as i64)
        }
    }

    #[tokio::test]
    async fn test_writer_receives_and_stores_events() {
        let store = Arc::new(MockStore::new());
        let store_dyn: Arc<dyn AuditStore> = Arc::clone(&store) as Arc<dyn AuditStore>;
        let (handle, writer) = create_audit_system(store_dyn, 10);

        // Spawn writer
        let writer_handle = tokio::spawn(writer.run());

        // Emit an event
        handle
            .emit(AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc123".to_string(),
            })
            .await;

        // Give writer time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Drop handle to close channel
        drop(handle);

        // Wait for writer to finish
        writer_handle.await.unwrap();

        // Verify event was stored
        let records = store.get_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_type, "service_started");
    }

    #[tokio::test]
    async fn test_writer_handles_multiple_events() {
        let store = Arc::new(MockStore::new());
        let store_dyn: Arc<dyn AuditStore> = Arc::clone(&store) as Arc<dyn AuditStore>;
        let (handle, writer) = create_audit_system(store_dyn, 10);

        let writer_handle = tokio::spawn(writer.run());

        // Emit multiple events
        for i in 0..5 {
            handle
                .emit(AuditEvent::TicketCreated {
                    ticket_id: format!("t-{}", i),
                    requested_by: "user".to_string(),
                    priority: 100,
                    tags: vec!["test".to_string()],
                    description: "test".to_string(),
                    dest_path: "/test".to_string(),
                })
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        drop(handle);
        writer_handle.await.unwrap();

        let records = store.get_records();
        assert_eq!(records.len(), 5);
    }

    #[tokio::test]
    async fn test_writer_continues_on_insert_failure() {
        let store = Arc::new(MockStore::failing());
        let store_dyn: Arc<dyn AuditStore> = Arc::clone(&store) as Arc<dyn AuditStore>;
        let (handle, writer) = create_audit_system(store_dyn, 10);

        let writer_handle = tokio::spawn(writer.run());

        // This should not cause the writer to crash
        handle
            .emit(AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc123".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        drop(handle);

        // Writer should complete normally
        writer_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_writer_extracts_ticket_and_user_ids() {
        let store = Arc::new(MockStore::new());
        let store_dyn: Arc<dyn AuditStore> = Arc::clone(&store) as Arc<dyn AuditStore>;
        let (handle, writer) = create_audit_system(store_dyn, 10);

        let writer_handle = tokio::spawn(writer.run());

        handle
            .emit(AuditEvent::TicketCreated {
                ticket_id: "ticket-123".to_string(),
                requested_by: "user-456".to_string(),
                priority: 100,
                tags: vec!["test".to_string()],
                description: "test".to_string(),
                dest_path: "/test".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        drop(handle);
        writer_handle.await.unwrap();

        let records = store.get_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].ticket_id, Some("ticket-123".to_string()));
        assert_eq!(records[0].user_id, Some("user-456".to_string()));
    }

    #[tokio::test]
    async fn test_cloned_handles_share_writer() {
        let store = Arc::new(MockStore::new());
        let store_dyn: Arc<dyn AuditStore> = Arc::clone(&store) as Arc<dyn AuditStore>;
        let (handle1, writer) = create_audit_system(store_dyn, 10);
        let handle2 = handle1.clone();

        let writer_handle = tokio::spawn(writer.run());

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

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        drop(handle1);
        drop(handle2);
        writer_handle.await.unwrap();

        let records = store.get_records();
        assert_eq!(records.len(), 2);
    }
}
