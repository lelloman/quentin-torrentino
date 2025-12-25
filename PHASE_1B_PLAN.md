# Phase 1B: SQLite Foundation + Audit Log

## Goal

Add SQLite persistence and a complete audit logging system. Events are submitted via an async channel and written to the database by a background task. A query API exposes audit events.

## Success Criteria

1. `cargo build --workspace` succeeds
2. `cargo test --workspace` passes
3. Server starts and creates SQLite database if it doesn't exist
4. Audit events are emitted for `ServiceStarted` and `ServiceStopped`
5. `GET /api/v1/audit` returns audit events with filtering support
6. Events are written asynchronously (non-blocking for callers)
7. Integration test: start server, query audit log, verify `ServiceStarted` event exists

---

## Database Schema

### File Location

Configured via `config.toml`:

```toml
[database]
path = "/data/quentin.db"  # or "./quentin.db" for local dev
```

### Tables

```sql
-- Audit events table
CREATE TABLE IF NOT EXISTS audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,           -- ISO 8601 format
    event_type TEXT NOT NULL,          -- e.g., "service_started", "ticket_created"
    ticket_id TEXT,                    -- NULL for non-ticket events
    user_id TEXT,                      -- NULL for system events
    data TEXT NOT NULL                 -- JSON blob with event-specific fields
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_events_ticket_id ON audit_events(ticket_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id);
```

---

## Audit Events

### Event Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEvent {
    // System events
    ServiceStarted {
        version: String,
        config_hash: String,
    },
    ServiceStopped {
        reason: String,
    },

    // Ticket lifecycle (stubs for now - will be used in later phases)
    TicketCreated {
        ticket_id: String,
        requested_by: String,
        // ticket_snapshot: Value, // full ticket JSON - add later
    },
    TicketStateChanged {
        ticket_id: String,
        from_state: String,
        to_state: String,
        reason: Option<String>,
    },
    TicketCancelled {
        ticket_id: String,
        cancelled_by: String,
        reason: Option<String>,
    },
}

impl AuditEvent {
    /// Returns the event type as a string for storage
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ServiceStarted { .. } => "service_started",
            Self::ServiceStopped { .. } => "service_stopped",
            Self::TicketCreated { .. } => "ticket_created",
            Self::TicketStateChanged { .. } => "ticket_state_changed",
            Self::TicketCancelled { .. } => "ticket_cancelled",
        }
    }

    /// Extract ticket_id if this event is ticket-related
    pub fn ticket_id(&self) -> Option<&str> {
        match self {
            Self::ServiceStarted { .. } | Self::ServiceStopped { .. } => None,
            Self::TicketCreated { ticket_id, .. }
            | Self::TicketStateChanged { ticket_id, .. }
            | Self::TicketCancelled { ticket_id, .. } => Some(ticket_id),
        }
    }

    /// Extract user_id if this event was triggered by a user action
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::TicketCreated { requested_by, .. } => Some(requested_by),
            Self::TicketCancelled { cancelled_by, .. } => Some(cancelled_by),
            _ => None,
        }
    }
}
```

### Stored Event Record

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub ticket_id: Option<String>,
    pub user_id: Option<String>,
    pub data: AuditEvent,
}
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Server                               │
│                                                              │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐ │
│  │   Handlers   │────▶│  AuditHandle │────▶│   Channel    │ │
│  │              │     │  (tx clone)  │     │    (mpsc)    │ │
│  └──────────────┘     └──────────────┘     └──────┬───────┘ │
│                                                   │         │
│  ┌──────────────┐                                 ▼         │
│  │  Query API   │◀────────────────────┐   ┌──────────────┐ │
│  │ GET /audit   │                     │   │ AuditWriter  │ │
│  └──────────────┘                     │   │   (task)     │ │
│         │                             │   └──────┬───────┘ │
│         │                             │          │         │
│         ▼                             │          ▼         │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                      SQLite                              ││
│  │                   audit_events                           ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Components

1. **AuditHandle** - Cheaply cloneable handle containing `mpsc::Sender<AuditEventEnvelope>`
2. **AuditEventEnvelope** - Wraps event with timestamp and metadata
3. **AuditWriter** - Background task that receives events and writes to SQLite
4. **AuditStore** - Trait for database operations (insert + query)
5. **SqliteAuditStore** - Implementation using rusqlite

---

## Crate Structure Updates

```
crates/core/src/
├── lib.rs
├── config/
│   ├── mod.rs
│   ├── types.rs        # Add DatabaseConfig
│   ├── loader.rs
│   └── validate.rs     # Add database path validation
├── auth/
│   └── ...
└── audit/              # NEW
    ├── mod.rs
    ├── events.rs       # AuditEvent enum
    ├── handle.rs       # AuditHandle (sender wrapper)
    ├── store.rs        # AuditStore trait
    └── sqlite.rs       # SqliteAuditStore implementation

crates/server/src/
├── main.rs             # Initialize audit system, spawn writer task
├── state.rs            # Add AuditHandle to AppState
└── api/
    ├── mod.rs
    ├── routes.rs       # Add audit routes
    ├── handlers.rs
    └── audit.rs        # NEW - audit query handlers
```

---

## Configuration Updates

### Config Struct

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

fn default_db_path() -> PathBuf {
    PathBuf::from("quentin.db")
}
```

### Validation

- Database path's parent directory must exist (or be creatable)
- Path must be writable

---

## API Endpoints

### Query Audit Events

```
GET /api/v1/audit
```

Query parameters:
- `ticket_id` (optional) - Filter by ticket ID
- `event_type` (optional) - Filter by event type
- `user_id` (optional) - Filter by user ID
- `from` (optional) - ISO 8601 timestamp, events after this time
- `to` (optional) - ISO 8601 timestamp, events before this time
- `limit` (optional, default 100, max 1000) - Number of events to return
- `offset` (optional, default 0) - Pagination offset

Response:

```json
{
  "events": [
    {
      "id": 1,
      "timestamp": "2024-12-25T10:00:00Z",
      "event_type": "service_started",
      "ticket_id": null,
      "user_id": null,
      "data": {
        "type": "service_started",
        "version": "0.1.0",
        "config_hash": "abc123"
      }
    }
  ],
  "total": 1,
  "limit": 100,
  "offset": 0
}
```

---

## Detailed Implementation Tasks

### Task 1: Add Dependencies

Update `crates/core/Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...
rusqlite = { version = "0.33", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"  # For config hash
```

Update workspace `Cargo.toml`:

```toml
[workspace.dependencies]
# ... existing deps ...
rusqlite = { version = "0.33", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
```

**Verification:** `cargo check --workspace` succeeds

---

### Task 2: Add DatabaseConfig

**File: `crates/core/src/config/types.rs`**

Add `DatabaseConfig` struct and update `Config` to include it.

**File: `crates/core/src/config/validate.rs`**

Add validation for database path.

**Unit tests:**
- Config with default database path
- Config with custom database path
- Validation passes for valid path

---

### Task 3: Audit Events Enum

**File: `crates/core/src/audit/events.rs`**

Define `AuditEvent` enum with:
- `ServiceStarted { version, config_hash }`
- `ServiceStopped { reason }`
- `TicketCreated { ticket_id, requested_by }`
- `TicketStateChanged { ticket_id, from_state, to_state, reason }`
- `TicketCancelled { ticket_id, cancelled_by, reason }`

Implement helper methods:
- `event_type() -> &'static str`
- `ticket_id() -> Option<&str>`
- `user_id() -> Option<&str>`

**File: `crates/core/src/audit/mod.rs`**

Define `AuditRecord` struct for stored events.

**Unit tests:**
- Serialize/deserialize each event type
- `event_type()` returns correct string
- `ticket_id()` returns correct value
- `user_id()` returns correct value

---

### Task 4: AuditStore Trait + SQLite Implementation

**File: `crates/core/src/audit/store.rs`**

```rust
pub trait AuditStore: Send + Sync {
    fn insert(&self, record: &AuditRecord) -> Result<i64, AuditError>;
    fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditRecord>, AuditError>;
    fn count(&self, filter: &AuditFilter) -> Result<i64, AuditError>;
}

pub struct AuditFilter {
    pub ticket_id: Option<String>,
    pub event_type: Option<String>,
    pub user_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}
```

**File: `crates/core/src/audit/sqlite.rs`**

Implement `SqliteAuditStore`:
- Constructor takes database path, creates tables if needed
- Uses `rusqlite::Connection` wrapped in `Mutex` for thread safety
- Implements `AuditStore` trait

**Unit tests:**
- Create store, insert event, query it back
- Query with filters (ticket_id, event_type, time range)
- Pagination works correctly

---

### Task 5: AuditHandle + Channel

**File: `crates/core/src/audit/handle.rs`**

```rust
pub struct AuditEventEnvelope {
    pub timestamp: DateTime<Utc>,
    pub event: AuditEvent,
}

#[derive(Clone)]
pub struct AuditHandle {
    tx: mpsc::Sender<AuditEventEnvelope>,
}

impl AuditHandle {
    pub fn new(tx: mpsc::Sender<AuditEventEnvelope>) -> Self {
        Self { tx }
    }

    pub async fn emit(&self, event: AuditEvent) {
        let envelope = AuditEventEnvelope {
            timestamp: Utc::now(),
            event,
        };
        // Log error but don't fail if channel is full/closed
        if let Err(e) = self.tx.send(envelope).await {
            tracing::error!("Failed to emit audit event: {}", e);
        }
    }

    /// Synchronous emit for contexts where async isn't available
    pub fn emit_blocking(&self, event: AuditEvent) {
        let envelope = AuditEventEnvelope {
            timestamp: Utc::now(),
            event,
        };
        if let Err(e) = self.tx.blocking_send(envelope) {
            tracing::error!("Failed to emit audit event: {}", e);
        }
    }
}
```

**Unit tests:**
- Create handle, emit event, receive on channel
- Multiple handles can send to same channel

---

### Task 6: AuditWriter Background Task

**File: `crates/core/src/audit/writer.rs`**

```rust
pub struct AuditWriter {
    rx: mpsc::Receiver<AuditEventEnvelope>,
    store: Arc<dyn AuditStore>,
}

impl AuditWriter {
    pub fn new(
        rx: mpsc::Receiver<AuditEventEnvelope>,
        store: Arc<dyn AuditStore>,
    ) -> Self {
        Self { rx, store }
    }

    pub async fn run(mut self) {
        while let Some(envelope) = self.rx.recv().await {
            let record = AuditRecord {
                id: 0, // Will be set by DB
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

/// Create audit system: returns handle for emitting and writer task to spawn
pub fn create_audit_system(
    store: Arc<dyn AuditStore>,
    buffer_size: usize,
) -> (AuditHandle, AuditWriter) {
    let (tx, rx) = mpsc::channel(buffer_size);
    let handle = AuditHandle::new(tx);
    let writer = AuditWriter::new(rx, store);
    (handle, writer)
}
```

**Unit tests:**
- Writer receives events and calls store.insert()
- Writer logs errors but continues on insert failure

---

### Task 7: Update Core Lib Exports

**File: `crates/core/src/lib.rs`**

Add audit module and exports:

```rust
pub mod audit;

pub use audit::{
    AuditEvent, AuditRecord, AuditFilter, AuditStore, SqliteAuditStore,
    AuditHandle, AuditWriter, create_audit_system, AuditError,
};
```

---

### Task 8: Server Integration - Initialize Audit System

**File: `crates/server/src/main.rs`**

Update `run()` to:
1. Create SQLite audit store from config
2. Create audit system (handle + writer)
3. Spawn writer as background task
4. Emit `ServiceStarted` event
5. Add audit handle to AppState
6. Emit `ServiceStopped` on shutdown (graceful shutdown handler)

**File: `crates/server/src/state.rs`**

Add `AuditHandle` to `AppState`:

```rust
pub struct AppState {
    config: Config,
    authenticator: Arc<dyn Authenticator>,
    audit: AuditHandle,
    audit_store: Arc<dyn AuditStore>,  // For queries
}
```

---

### Task 9: Audit Query API

**File: `crates/server/src/api/audit.rs`**

```rust
#[derive(Deserialize)]
pub struct AuditQueryParams {
    pub ticket_id: Option<String>,
    pub event_type: Option<String>,
    pub user_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Serialize)]
pub struct AuditQueryResponse {
    pub events: Vec<AuditRecord>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

pub async fn query_audit(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditQueryParams>,
) -> Result<Json<AuditQueryResponse>, ApiError> {
    // Validate and cap limit
    // Build filter
    // Query store
    // Return response
}
```

**File: `crates/server/src/api/routes.rs`**

Add route: `GET /api/v1/audit`

---

### Task 10: Update Example Config

**File: `config.example.toml`**

Add database section:

```toml
# ==============================================================================
# DATABASE
# ==============================================================================

[database]
# Path to SQLite database file
# Default: "quentin.db" (current directory)
path = "quentin.db"
```

---

### Task 11: Integration Tests

**File: `crates/server/tests/audit_integration.rs`**

Tests:
- Server starts and creates database file
- `GET /api/v1/audit` returns `ServiceStarted` event
- Query with `event_type` filter works
- Query with time range filter works
- Pagination works

---

## Verification Checklist

After implementation, verify:

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` has no warnings
- [ ] `cargo fmt --check` passes
- [ ] Server creates SQLite file on startup
- [ ] `ServiceStarted` event is logged
- [ ] `curl http://localhost:8080/api/v1/audit` returns events
- [ ] Filtering by `event_type` works
- [ ] Database persists across restarts

---

## Out of Scope (Deferred)

- JSON Lines file output (secondary sink)
- Audit event export endpoint (`GET /api/v1/audit/export`)
- Ticket-related events actually being emitted (no tickets yet)
- Audit log retention/cleanup policies
- Batched writes for performance
