use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::{AuditError, AuditEvent, AuditFilter, AuditRecord, AuditStore};

/// SQLite-backed audit store
pub struct SqliteAuditStore {
    conn: Mutex<Connection>,
}

impl SqliteAuditStore {
    /// Create a new SQLite audit store, creating the database file and tables if needed
    pub fn new(path: &Path) -> Result<Self, AuditError> {
        let conn = Connection::open(path).map_err(|e| AuditError::Database(e.to_string()))?;

        // Create tables
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                event_type TEXT NOT NULL,
                ticket_id TEXT,
                user_id TEXT,
                data TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_events_ticket_id ON audit_events(ticket_id);
            CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id);
            "#,
        )
        .map_err(|e| AuditError::Database(e.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite audit store (useful for testing)
    pub fn in_memory() -> Result<Self, AuditError> {
        let conn = Connection::open_in_memory().map_err(|e| AuditError::Database(e.to_string()))?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                event_type TEXT NOT NULL,
                ticket_id TEXT,
                user_id TEXT,
                data TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_events_ticket_id ON audit_events(ticket_id);
            CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id);
            "#,
        )
        .map_err(|e| AuditError::Database(e.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn build_where_clause(filter: &AuditFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref ticket_id) = filter.ticket_id {
            conditions.push("ticket_id = ?");
            params.push(Box::new(ticket_id.clone()));
        }

        if let Some(ref event_type) = filter.event_type {
            conditions.push("event_type = ?");
            params.push(Box::new(event_type.clone()));
        }

        if let Some(ref user_id) = filter.user_id {
            conditions.push("user_id = ?");
            params.push(Box::new(user_id.clone()));
        }

        if let Some(ref from) = filter.from {
            conditions.push("timestamp >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }

        if let Some(ref to) = filter.to {
            conditions.push("timestamp <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }
}

impl AuditStore for SqliteAuditStore {
    fn insert(&self, record: &AuditRecord) -> Result<i64, AuditError> {
        let conn = self.conn.lock().unwrap();

        let data_json = serde_json::to_string(&record.data)
            .map_err(|e| AuditError::Serialization(e.to_string()))?;

        conn.execute(
            "INSERT INTO audit_events (timestamp, event_type, ticket_id, user_id, data) VALUES (?, ?, ?, ?, ?)",
            params![
                record.timestamp.to_rfc3339(),
                record.event_type,
                record.ticket_id,
                record.user_id,
                data_json,
            ],
        )
        .map_err(|e| AuditError::Database(e.to_string()))?;

        Ok(conn.last_insert_rowid())
    }

    fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditRecord>, AuditError> {
        let conn = self.conn.lock().unwrap();

        let (where_clause, params) = Self::build_where_clause(filter);

        let sql = format!(
            "SELECT id, timestamp, event_type, ticket_id, user_id, data FROM audit_events {} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
            where_clause
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AuditError::Database(e.to_string()))?;

        // Build parameter slice with limit and offset
        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = params;
        all_params.push(Box::new(filter.limit));
        all_params.push(Box::new(filter.offset));

        let param_refs: Vec<&dyn rusqlite::ToSql> = all_params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let id: i64 = row.get(0)?;
                let timestamp_str: String = row.get(1)?;
                let event_type: String = row.get(2)?;
                let ticket_id: Option<String> = row.get(3)?;
                let user_id: Option<String> = row.get(4)?;
                let data_json: String = row.get(5)?;

                Ok((id, timestamp_str, event_type, ticket_id, user_id, data_json))
            })
            .map_err(|e| AuditError::Database(e.to_string()))?;

        let mut records = Vec::new();
        for row_result in rows {
            let (id, timestamp_str, event_type, ticket_id, user_id, data_json) =
                row_result.map_err(|e| AuditError::Database(e.to_string()))?;

            let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339(&timestamp_str)
                .map_err(|e| AuditError::Database(format!("Invalid timestamp: {}", e)))?
                .into();

            let data: AuditEvent = serde_json::from_str(&data_json)
                .map_err(|e| AuditError::Serialization(e.to_string()))?;

            records.push(AuditRecord {
                id,
                timestamp,
                event_type,
                ticket_id,
                user_id,
                data,
            });
        }

        Ok(records)
    }

    fn count(&self, filter: &AuditFilter) -> Result<i64, AuditError> {
        let conn = self.conn.lock().unwrap();

        let (where_clause, params) = Self::build_where_clause(filter);

        let sql = format!("SELECT COUNT(*) FROM audit_events {}", where_clause);

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| AuditError::Database(e.to_string()))?;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn create_test_store() -> SqliteAuditStore {
        SqliteAuditStore::in_memory().unwrap()
    }

    fn create_service_started_record() -> AuditRecord {
        AuditRecord {
            id: 0,
            timestamp: Utc::now(),
            event_type: "service_started".to_string(),
            ticket_id: None,
            user_id: None,
            data: AuditEvent::ServiceStarted {
                version: "0.1.0".to_string(),
                config_hash: "abc123".to_string(),
            },
        }
    }

    fn create_ticket_created_record(ticket_id: &str, user_id: &str) -> AuditRecord {
        AuditRecord {
            id: 0,
            timestamp: Utc::now(),
            event_type: "ticket_created".to_string(),
            ticket_id: Some(ticket_id.to_string()),
            user_id: Some(user_id.to_string()),
            data: AuditEvent::TicketCreated {
                ticket_id: ticket_id.to_string(),
                requested_by: user_id.to_string(),
            },
        }
    }

    #[test]
    fn test_insert_and_query() {
        let store = create_test_store();
        let record = create_service_started_record();

        let id = store.insert(&record).unwrap();
        assert!(id > 0);

        let results = store.query(&AuditFilter::new()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert_eq!(results[0].event_type, "service_started");
    }

    #[test]
    fn test_query_by_event_type() {
        let store = create_test_store();

        store.insert(&create_service_started_record()).unwrap();
        store
            .insert(&create_ticket_created_record("t-1", "user-1"))
            .unwrap();
        store
            .insert(&create_ticket_created_record("t-2", "user-2"))
            .unwrap();

        let filter = AuditFilter::new().with_event_type("ticket_created");
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 2);

        let filter = AuditFilter::new().with_event_type("service_started");
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_by_ticket_id() {
        let store = create_test_store();

        store
            .insert(&create_ticket_created_record("t-1", "user-1"))
            .unwrap();
        store
            .insert(&create_ticket_created_record("t-2", "user-2"))
            .unwrap();

        let filter = AuditFilter::new().with_ticket_id("t-1");
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ticket_id, Some("t-1".to_string()));
    }

    #[test]
    fn test_query_by_user_id() {
        let store = create_test_store();

        store
            .insert(&create_ticket_created_record("t-1", "user-1"))
            .unwrap();
        store
            .insert(&create_ticket_created_record("t-2", "user-1"))
            .unwrap();
        store
            .insert(&create_ticket_created_record("t-3", "user-2"))
            .unwrap();

        let filter = AuditFilter::new().with_user_id("user-1");
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_with_time_range() {
        let store = create_test_store();

        let now = Utc::now();
        let mut old_record = create_service_started_record();
        old_record.timestamp = now - Duration::hours(2);
        store.insert(&old_record).unwrap();

        let mut new_record = create_service_started_record();
        new_record.timestamp = now;
        store.insert(&new_record).unwrap();

        // Query only recent events
        let filter = AuditFilter::new().with_time_range(Some(now - Duration::hours(1)), None);
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_pagination() {
        let store = create_test_store();

        for i in 0..5 {
            store
                .insert(&create_ticket_created_record(&format!("t-{}", i), "user"))
                .unwrap();
        }

        let filter = AuditFilter::new().with_limit(2).with_offset(0);
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 2);

        let filter = AuditFilter::new().with_limit(2).with_offset(2);
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 2);

        let filter = AuditFilter::new().with_limit(2).with_offset(4);
        let results = store.query(&filter).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_count() {
        let store = create_test_store();

        store.insert(&create_service_started_record()).unwrap();
        store
            .insert(&create_ticket_created_record("t-1", "user-1"))
            .unwrap();
        store
            .insert(&create_ticket_created_record("t-2", "user-2"))
            .unwrap();

        let count = store.count(&AuditFilter::new()).unwrap();
        assert_eq!(count, 3);

        let filter = AuditFilter::new().with_event_type("ticket_created");
        let count = store.count(&filter).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_file_based_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let store = SqliteAuditStore::new(&db_path).unwrap();
        store.insert(&create_service_started_record()).unwrap();

        // Verify file was created
        assert!(db_path.exists());

        let results = store.query(&AuditFilter::new()).unwrap();
        assert_eq!(results.len(), 1);
    }
}
