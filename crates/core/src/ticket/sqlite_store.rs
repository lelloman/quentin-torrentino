//! SQLite-backed ticket store implementation.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::{
    CreateTicketRequest, OutputConstraints, QueryContext, Ticket, TicketError, TicketFilter,
    TicketState, TicketStore,
};

/// SQLite-backed ticket store.
pub struct SqliteTicketStore {
    conn: Mutex<Connection>,
}

impl SqliteTicketStore {
    /// Create a new SQLite ticket store, creating the database file and tables if needed.
    pub fn new(path: &Path) -> Result<Self, TicketError> {
        let conn = Connection::open(path).map_err(|e| TicketError::Database(e.to_string()))?;
        Self::initialize_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite ticket store (useful for testing).
    pub fn in_memory() -> Result<Self, TicketError> {
        let conn =
            Connection::open_in_memory().map_err(|e| TicketError::Database(e.to_string()))?;
        Self::initialize_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn initialize_schema(conn: &Connection) -> Result<(), TicketError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tickets (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                state TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                query_context TEXT NOT NULL,
                dest_path TEXT NOT NULL,
                output_constraints TEXT,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tickets_created_by ON tickets(created_by);
            CREATE INDEX IF NOT EXISTS idx_tickets_priority ON tickets(priority DESC);
            CREATE INDEX IF NOT EXISTS idx_tickets_updated_at ON tickets(updated_at);
            "#,
        )
        .map_err(|e| TicketError::Database(e.to_string()))?;

        // Migration: add output_constraints column if it doesn't exist
        let _ = conn.execute("ALTER TABLE tickets ADD COLUMN output_constraints TEXT", []);

        // Migration: add retry_count column if it doesn't exist
        let _ = conn.execute(
            "ALTER TABLE tickets ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0",
            [],
        );

        Ok(())
    }

    fn build_where_clause(filter: &TicketFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref state) = filter.state {
            // We need to extract the state type from the JSON
            // Using json_extract to get the "type" field from the state JSON
            conditions.push("json_extract(state, '$.type') = ?");
            params.push(Box::new(state.clone()));
        }

        if let Some(ref created_by) = filter.created_by {
            conditions.push("created_by = ?");
            params.push(Box::new(created_by.clone()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }

    fn row_to_ticket(row: &rusqlite::Row) -> rusqlite::Result<Ticket> {
        let id: String = row.get(0)?;
        let created_at_str: String = row.get(1)?;
        let created_by: String = row.get(2)?;
        let state_json: String = row.get(3)?;
        let priority: u16 = row.get(4)?;
        let query_context_json: String = row.get(5)?;
        let dest_path: String = row.get(6)?;
        let output_constraints_json: Option<String> = row.get(7)?;
        let updated_at_str: String = row.get(8)?;
        let retry_count: u32 = row.get::<_, Option<u32>>(9)?.unwrap_or(0);

        // Parse timestamps - use default if parsing fails (shouldn't happen with valid data)
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        // Parse JSON fields - these should never fail with valid data
        let state: TicketState = serde_json::from_str(&state_json).unwrap_or(TicketState::Pending);

        let query_context: QueryContext = serde_json::from_str(&query_context_json)
            .unwrap_or_else(|_| QueryContext::new(vec![], ""));

        let output_constraints: Option<OutputConstraints> =
            output_constraints_json.and_then(|json| serde_json::from_str(&json).ok());

        Ok(Ticket {
            id,
            created_at,
            created_by,
            state,
            priority,
            query_context,
            dest_path,
            output_constraints,
            retry_count,
            updated_at,
        })
    }
}

impl TicketStore for SqliteTicketStore {
    fn create(&self, request: CreateTicketRequest) -> Result<Ticket, TicketError> {
        let conn = self.conn.lock().unwrap();

        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let state = TicketState::Pending;

        let state_json =
            serde_json::to_string(&state).map_err(|e| TicketError::Database(e.to_string()))?;

        let query_context_json = serde_json::to_string(&request.query_context)
            .map_err(|e| TicketError::Database(e.to_string()))?;

        let output_constraints_json = request
            .output_constraints
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| TicketError::Database(e.to_string()))?;

        conn.execute(
            "INSERT INTO tickets (id, created_at, created_by, state, priority, query_context, dest_path, output_constraints, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                now.to_rfc3339(),
                request.created_by,
                state_json,
                request.priority,
                query_context_json,
                request.dest_path,
                output_constraints_json,
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| TicketError::Database(e.to_string()))?;

        Ok(Ticket {
            id,
            created_at: now,
            created_by: request.created_by,
            state,
            priority: request.priority,
            query_context: request.query_context,
            dest_path: request.dest_path,
            output_constraints: request.output_constraints,
            retry_count: 0,
            updated_at: now,
        })
    }

    fn get(&self, id: &str) -> Result<Option<Ticket>, TicketError> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, created_at, created_by, state, priority, query_context, dest_path, output_constraints, updated_at, retry_count FROM tickets WHERE id = ?",
            params![id],
            Self::row_to_ticket,
        );

        match result {
            Ok(ticket) => Ok(Some(ticket)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(TicketError::Database(e.to_string())),
        }
    }

    fn list(&self, filter: &TicketFilter) -> Result<Vec<Ticket>, TicketError> {
        let conn = self.conn.lock().unwrap();

        let (where_clause, params) = Self::build_where_clause(filter);

        let sql = format!(
            "SELECT id, created_at, created_by, state, priority, query_context, dest_path, output_constraints, updated_at, retry_count FROM tickets {} ORDER BY priority DESC, created_at ASC LIMIT ? OFFSET ?",
            where_clause
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| TicketError::Database(e.to_string()))?;

        // Build parameter slice with limit and offset
        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = params;
        all_params.push(Box::new(filter.limit));
        all_params.push(Box::new(filter.offset));

        let param_refs: Vec<&dyn rusqlite::ToSql> = all_params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_ticket)
            .map_err(|e| TicketError::Database(e.to_string()))?;

        let mut tickets = Vec::new();
        for row_result in rows {
            let ticket = row_result.map_err(|e| TicketError::Database(e.to_string()))?;
            tickets.push(ticket);
        }

        Ok(tickets)
    }

    fn count(&self, filter: &TicketFilter) -> Result<i64, TicketError> {
        let conn = self.conn.lock().unwrap();

        let (where_clause, params) = Self::build_where_clause(filter);

        let sql = format!("SELECT COUNT(*) FROM tickets {}", where_clause);

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| TicketError::Database(e.to_string()))?;

        Ok(count)
    }

    fn update_state(&self, id: &str, new_state: TicketState) -> Result<Ticket, TicketError> {
        let conn = self.conn.lock().unwrap();

        // First, get the current ticket to check state
        let current = conn.query_row(
            "SELECT id, created_at, created_by, state, priority, query_context, dest_path, output_constraints, updated_at, retry_count FROM tickets WHERE id = ?",
            params![id],
            Self::row_to_ticket,
        );

        let current_ticket = match current {
            Ok(ticket) => ticket,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(TicketError::NotFound(id.to_string()));
            }
            Err(e) => return Err(TicketError::Database(e.to_string())),
        };

        // Check if we can transition to cancelled state
        if matches!(new_state, TicketState::Cancelled { .. }) && !current_ticket.state.can_cancel()
        {
            return Err(TicketError::InvalidState {
                ticket_id: id.to_string(),
                current_state: current_ticket.state.state_type().to_string(),
                operation: "cancel".to_string(),
            });
        }

        let now = Utc::now();
        let state_json =
            serde_json::to_string(&new_state).map_err(|e| TicketError::Database(e.to_string()))?;

        conn.execute(
            "UPDATE tickets SET state = ?, updated_at = ? WHERE id = ?",
            params![state_json, now.to_rfc3339(), id],
        )
        .map_err(|e| TicketError::Database(e.to_string()))?;

        Ok(Ticket {
            id: current_ticket.id,
            created_at: current_ticket.created_at,
            created_by: current_ticket.created_by,
            state: new_state,
            priority: current_ticket.priority,
            query_context: current_ticket.query_context,
            dest_path: current_ticket.dest_path,
            output_constraints: current_ticket.output_constraints,
            retry_count: current_ticket.retry_count,
            updated_at: now,
        })
    }

    fn increment_retry_count(&self, id: &str) -> Result<Ticket, TicketError> {
        let conn = self.conn.lock().unwrap();

        // Get current ticket
        let current = conn.query_row(
            "SELECT id, created_at, created_by, state, priority, query_context, dest_path, output_constraints, updated_at, retry_count FROM tickets WHERE id = ?",
            params![id],
            Self::row_to_ticket,
        );

        let current_ticket = match current {
            Ok(ticket) => ticket,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(TicketError::NotFound(id.to_string()));
            }
            Err(e) => return Err(TicketError::Database(e.to_string())),
        };

        let new_retry_count = current_ticket.retry_count + 1;
        let now = Utc::now();

        conn.execute(
            "UPDATE tickets SET retry_count = ?, updated_at = ? WHERE id = ?",
            params![new_retry_count, now.to_rfc3339(), id],
        )
        .map_err(|e| TicketError::Database(e.to_string()))?;

        Ok(Ticket {
            retry_count: new_retry_count,
            updated_at: now,
            ..current_ticket
        })
    }

    fn delete(&self, id: &str) -> Result<Ticket, TicketError> {
        let conn = self.conn.lock().unwrap();

        // First, get the ticket to return it
        let ticket = conn.query_row(
            "SELECT id, created_at, created_by, state, priority, query_context, dest_path, output_constraints, updated_at, retry_count FROM tickets WHERE id = ?",
            params![id],
            Self::row_to_ticket,
        );

        let ticket = match ticket {
            Ok(t) => t,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(TicketError::NotFound(id.to_string()));
            }
            Err(e) => return Err(TicketError::Database(e.to_string())),
        };

        // Delete the ticket
        conn.execute("DELETE FROM tickets WHERE id = ?", params![id])
            .map_err(|e| TicketError::Database(e.to_string()))?;

        Ok(ticket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> SqliteTicketStore {
        SqliteTicketStore::in_memory().unwrap()
    }

    fn create_test_request() -> CreateTicketRequest {
        CreateTicketRequest {
            created_by: "test-user".to_string(),
            priority: 100,
            query_context: QueryContext::new(
                vec!["music".to_string(), "flac".to_string()],
                "Abbey Road by The Beatles",
            ),
            dest_path: "/media/music/beatles".to_string(),
            output_constraints: None, // Keep original format
        }
    }

    #[test]
    fn test_create_ticket() {
        let store = create_test_store();
        let request = create_test_request();

        let ticket = store.create(request.clone()).unwrap();

        assert!(!ticket.id.is_empty());
        assert_eq!(ticket.created_by, request.created_by);
        assert_eq!(ticket.priority, request.priority);
        assert_eq!(ticket.query_context, request.query_context);
        assert_eq!(ticket.dest_path, request.dest_path);
        assert_eq!(ticket.state, TicketState::Pending);
    }

    #[test]
    fn test_get_ticket() {
        let store = create_test_store();
        let request = create_test_request();

        let created = store.create(request).unwrap();
        let fetched = store.get(&created.id).unwrap();

        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.created_by, created.created_by);
    }

    #[test]
    fn test_get_nonexistent_ticket() {
        let store = create_test_store();
        let result = store.get("nonexistent-id").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_tickets() {
        let store = create_test_store();

        // Create 3 tickets
        for i in 0..3 {
            let mut request = create_test_request();
            request.created_by = format!("user-{}", i);
            store.create(request).unwrap();
        }

        let tickets = store.list(&TicketFilter::new()).unwrap();
        assert_eq!(tickets.len(), 3);
    }

    #[test]
    fn test_list_with_state_filter() {
        let store = create_test_store();

        // Create 2 pending tickets
        store.create(create_test_request()).unwrap();
        let ticket2 = store.create(create_test_request()).unwrap();

        // Cancel one
        store
            .update_state(
                &ticket2.id,
                TicketState::Cancelled {
                    cancelled_by: "admin".to_string(),
                    reason: None,
                    cancelled_at: Utc::now(),
                },
            )
            .unwrap();

        // Filter by pending
        let filter = TicketFilter::new().with_state("pending");
        let tickets = store.list(&filter).unwrap();
        assert_eq!(tickets.len(), 1);

        // Filter by cancelled
        let filter = TicketFilter::new().with_state("cancelled");
        let tickets = store.list(&filter).unwrap();
        assert_eq!(tickets.len(), 1);
    }

    #[test]
    fn test_list_with_created_by_filter() {
        let store = create_test_store();

        let mut request1 = create_test_request();
        request1.created_by = "alice".to_string();
        store.create(request1).unwrap();

        let mut request2 = create_test_request();
        request2.created_by = "bob".to_string();
        store.create(request2).unwrap();

        let filter = TicketFilter::new().with_created_by("alice");
        let tickets = store.list(&filter).unwrap();
        assert_eq!(tickets.len(), 1);
        assert_eq!(tickets[0].created_by, "alice");
    }

    #[test]
    fn test_list_pagination() {
        let store = create_test_store();

        // Create 5 tickets
        for _ in 0..5 {
            store.create(create_test_request()).unwrap();
        }

        let filter = TicketFilter::new().with_limit(2).with_offset(0);
        let tickets = store.list(&filter).unwrap();
        assert_eq!(tickets.len(), 2);

        let filter = TicketFilter::new().with_limit(2).with_offset(2);
        let tickets = store.list(&filter).unwrap();
        assert_eq!(tickets.len(), 2);

        let filter = TicketFilter::new().with_limit(2).with_offset(4);
        let tickets = store.list(&filter).unwrap();
        assert_eq!(tickets.len(), 1);
    }

    #[test]
    fn test_list_priority_ordering() {
        let store = create_test_store();

        // Create tickets with different priorities
        let mut low_priority = create_test_request();
        low_priority.priority = 10;
        store.create(low_priority).unwrap();

        let mut high_priority = create_test_request();
        high_priority.priority = 100;
        store.create(high_priority).unwrap();

        let mut medium_priority = create_test_request();
        medium_priority.priority = 50;
        store.create(medium_priority).unwrap();

        let tickets = store.list(&TicketFilter::new()).unwrap();
        assert_eq!(tickets.len(), 3);
        assert_eq!(tickets[0].priority, 100); // Highest first
        assert_eq!(tickets[1].priority, 50);
        assert_eq!(tickets[2].priority, 10);
    }

    #[test]
    fn test_count_tickets() {
        let store = create_test_store();

        for _ in 0..3 {
            store.create(create_test_request()).unwrap();
        }

        let count = store.count(&TicketFilter::new()).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_with_filter() {
        let store = create_test_store();

        // Create 2 tickets, cancel 1
        store.create(create_test_request()).unwrap();
        let ticket2 = store.create(create_test_request()).unwrap();
        store
            .update_state(
                &ticket2.id,
                TicketState::Cancelled {
                    cancelled_by: "admin".to_string(),
                    reason: None,
                    cancelled_at: Utc::now(),
                },
            )
            .unwrap();

        let filter = TicketFilter::new().with_state("pending");
        let count = store.count(&filter).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_update_state_to_cancelled() {
        let store = create_test_store();
        let ticket = store.create(create_test_request()).unwrap();

        let cancelled_at = Utc::now();
        let updated = store
            .update_state(
                &ticket.id,
                TicketState::Cancelled {
                    cancelled_by: "admin".to_string(),
                    reason: Some("testing".to_string()),
                    cancelled_at,
                },
            )
            .unwrap();

        assert!(matches!(updated.state, TicketState::Cancelled { .. }));

        // Verify persistence
        let fetched = store.get(&ticket.id).unwrap().unwrap();
        assert!(matches!(fetched.state, TicketState::Cancelled { .. }));
    }

    #[test]
    fn test_cannot_cancel_already_cancelled() {
        let store = create_test_store();
        let ticket = store.create(create_test_request()).unwrap();

        // Cancel once
        store
            .update_state(
                &ticket.id,
                TicketState::Cancelled {
                    cancelled_by: "admin".to_string(),
                    reason: None,
                    cancelled_at: Utc::now(),
                },
            )
            .unwrap();

        // Try to cancel again
        let result = store.update_state(
            &ticket.id,
            TicketState::Cancelled {
                cancelled_by: "admin".to_string(),
                reason: None,
                cancelled_at: Utc::now(),
            },
        );

        assert!(matches!(result, Err(TicketError::InvalidState { .. })));
    }

    #[test]
    fn test_update_state_nonexistent_ticket() {
        let store = create_test_store();

        let result = store.update_state(
            "nonexistent-id",
            TicketState::Cancelled {
                cancelled_by: "admin".to_string(),
                reason: None,
                cancelled_at: Utc::now(),
            },
        );

        assert!(matches!(result, Err(TicketError::NotFound(_))));
    }

    #[test]
    fn test_update_state_to_completed() {
        use crate::ticket::CompletionStats;

        let store = create_test_store();
        let ticket = store.create(create_test_request()).unwrap();

        let updated = store
            .update_state(
                &ticket.id,
                TicketState::Completed {
                    completed_at: Utc::now(),
                    stats: CompletionStats {
                        total_download_bytes: 1_000_000,
                        download_duration_secs: 60,
                        conversion_duration_secs: 30,
                        final_size_bytes: 500_000,
                        files_placed: 5,
                    },
                },
            )
            .unwrap();

        assert!(matches!(updated.state, TicketState::Completed { .. }));
    }

    #[test]
    fn test_update_state_to_failed() {
        let store = create_test_store();
        let ticket = store.create(create_test_request()).unwrap();

        let updated = store
            .update_state(
                &ticket.id,
                TicketState::Failed {
                    error: "Something went wrong".to_string(),
                    retryable: true,
                    retry_count: 0,
                    failed_at: Utc::now(),
                },
            )
            .unwrap();

        assert!(matches!(updated.state, TicketState::Failed { .. }));
    }

    #[test]
    fn test_file_based_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("tickets.db");

        let store = SqliteTicketStore::new(&db_path).unwrap();
        let ticket = store.create(create_test_request()).unwrap();

        // Verify file was created
        assert!(db_path.exists());

        // Verify we can fetch the ticket
        let fetched = store.get(&ticket.id).unwrap();
        assert!(fetched.is_some());
    }
}
