# Phase 2: Ticket System Core

## Overview

This phase implements the foundational ticket system - the data model, state machine, persistence, and CRUD API. By the end, you'll be able to create, query, update, and cancel tickets via HTTP API, with all operations recorded in the audit log.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Query context | `QueryContext { tags, description }` | Structured tags for routing + freeform text for LLM matching |
| State storage | Current state only in tickets table | Audit log provides full history reconstruction |
| DELETE behavior | Transition to `cancelled` state | Maintains audit trail, fits state machine model |
| State machine | Simple enum with transition methods | No external library needed |

## Scope

### In Scope
- Ticket data model with `QueryContext`
- State machine (subset of states - no processing states yet)
- SQLite persistence for tickets
- CRUD API endpoints
- Audit events for ticket lifecycle
- Manual testing via curl/httpie

### Out of Scope (Future Phases)
- Processing pools and workers
- External service integration (Jackett, qBittorrent)
- WebSocket real-time updates
- Matcher/Converter implementations

## State Machine (Phase 2 Subset)

For this phase, we implement a minimal state machine. Processing states will be added when we build the processing pipeline.

```
┌─────────────┐
│   PENDING   │ ← Ticket created, awaiting processing
└──────┬──────┘
       │
       │ (future: picked up by processor)
       ▼
   [PROCESSING STATES - Phase 3+]
       │
       ▼
┌─────────────┐
│  COMPLETED  │ (terminal)
└─────────────┘

┌─────────────┐
│  CANCELLED  │ (terminal) ← Via DELETE endpoint
└─────────────┘

┌─────────────┐
│   FAILED    │ (terminal)
└─────────────┘
```

### Phase 2 States

```rust
pub enum TicketState {
    /// Ticket created, waiting to be processed
    Pending,

    /// Ticket was cancelled by user/admin
    Cancelled {
        cancelled_by: String,
        reason: Option<String>,
        cancelled_at: DateTime<Utc>,
    },

    /// Ticket completed successfully (for testing - manual transition)
    Completed {
        completed_at: DateTime<Utc>,
    },

    /// Ticket failed (for testing - manual transition)
    Failed {
        error: String,
        failed_at: DateTime<Utc>,
    },
}
```

## Data Model

### Ticket

```rust
pub struct Ticket {
    /// Unique identifier (UUID)
    pub id: String,

    /// When the ticket was created
    pub created_at: DateTime<Utc>,

    /// User who created the ticket (from auth identity)
    pub created_by: String,

    /// Current state
    pub state: TicketState,

    /// Priority for queue ordering (higher = more urgent)
    pub priority: u16,

    /// Query context for search/matching
    pub query_context: QueryContext,

    /// Destination path for final output
    pub dest_path: String,
}
```

### QueryContext

```rust
pub struct QueryContext {
    /// Structured tags for categorization and routing
    /// e.g., ["music", "flac", "album"] or ["movie", "1080p"]
    pub tags: Vec<String>,

    /// Freeform description for LLM-based matching
    /// e.g., "Looking for the 2019 remaster of Abbey Road by The Beatles"
    pub description: String,
}
```

## API Endpoints

### Create Ticket
```
POST /api/v1/tickets
Content-Type: application/json

{
    "priority": 100,
    "query_context": {
        "tags": ["music", "flac", "album"],
        "description": "Abbey Road by The Beatles, prefer 2019 remaster"
    },
    "dest_path": "/media/music/beatles/abbey-road"
}

Response: 201 Created
{
    "id": "uuid",
    "created_at": "2024-12-25T10:00:00Z",
    "created_by": "anonymous",
    "state": "pending",
    "priority": 100,
    "query_context": { ... },
    "dest_path": "/media/music/beatles/abbey-road"
}
```

### Get Ticket
```
GET /api/v1/tickets/{id}

Response: 200 OK
{
    "id": "uuid",
    "created_at": "2024-12-25T10:00:00Z",
    "created_by": "anonymous",
    "state": "pending",
    ...
}
```

### List Tickets
```
GET /api/v1/tickets?state=pending&limit=50&offset=0

Response: 200 OK
{
    "tickets": [...],
    "total": 123,
    "limit": 50,
    "offset": 0
}
```

### Cancel Ticket (DELETE)
```
DELETE /api/v1/tickets/{id}
Content-Type: application/json

{
    "reason": "No longer needed"  // optional
}

Response: 200 OK
{
    "id": "uuid",
    "state": {
        "cancelled": {
            "cancelled_by": "anonymous",
            "reason": "No longer needed",
            "cancelled_at": "2024-12-25T11:00:00Z"
        }
    },
    ...
}
```

### Error Responses
```
404 Not Found - Ticket doesn't exist
409 Conflict - Cannot cancel (already in terminal state)
400 Bad Request - Invalid input
```

## Audit Events

New audit events for ticket lifecycle:

```rust
pub enum AuditEventType {
    // Existing
    ServiceStarted,

    // New for Phase 2
    TicketCreated,
    TicketCancelled,
    TicketStateChanged,  // For future state transitions
}
```

### Event Data

```rust
// TicketCreated
{
    "ticket_id": "uuid",
    "created_by": "user_id",
    "priority": 100,
    "query_context": { "tags": [...], "description": "..." },
    "dest_path": "/path/to/dest"
}

// TicketCancelled
{
    "ticket_id": "uuid",
    "cancelled_by": "user_id",
    "reason": "optional reason",
    "previous_state": "pending"
}
```

## Database Schema

### tickets table

```sql
CREATE TABLE tickets (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,           -- ISO 8601
    created_by TEXT NOT NULL,
    state TEXT NOT NULL,                -- JSON-encoded TicketState
    priority INTEGER NOT NULL DEFAULT 0,
    query_context TEXT NOT NULL,        -- JSON-encoded QueryContext
    dest_path TEXT NOT NULL,
    updated_at TEXT NOT NULL            -- ISO 8601, for optimistic locking
);

CREATE INDEX idx_tickets_state ON tickets(
    json_extract(state, '$.type')       -- For filtering by state type
);

CREATE INDEX idx_tickets_created_by ON tickets(created_by);

CREATE INDEX idx_tickets_priority ON tickets(priority DESC);
```

## Implementation Tasks

### Task 1: Core Data Types
**File**: `crates/core/src/ticket/mod.rs` (new)

- [ ] Define `TicketState` enum with serde serialization
- [ ] Define `QueryContext` struct
- [ ] Define `Ticket` struct
- [ ] Implement `TicketState::is_terminal()` method
- [ ] Implement `TicketState::can_cancel()` method
- [ ] Unit tests for state logic

### Task 2: Ticket Store Trait
**File**: `crates/core/src/ticket/store.rs` (new)

- [ ] Define `TicketStore` trait
- [ ] Define `TicketFilter` for queries
- [ ] Define `CreateTicketRequest` struct
- [ ] Methods: `create`, `get`, `list`, `update_state`, `count`

```rust
#[async_trait]
pub trait TicketStore: Send + Sync {
    fn create(&self, request: CreateTicketRequest) -> Result<Ticket, TicketError>;
    fn get(&self, id: &str) -> Result<Option<Ticket>, TicketError>;
    fn list(&self, filter: &TicketFilter) -> Result<Vec<Ticket>, TicketError>;
    fn count(&self, filter: &TicketFilter) -> Result<i64, TicketError>;
    fn update_state(&self, id: &str, new_state: TicketState) -> Result<Ticket, TicketError>;
}
```

### Task 3: SQLite Ticket Store Implementation
**File**: `crates/core/src/ticket/sqlite_store.rs` (new)

- [ ] Implement `SqliteTicketStore`
- [ ] Create tickets table migration
- [ ] Implement all trait methods
- [ ] JSON serialization for state and query_context
- [ ] Integration tests with real SQLite

### Task 4: Ticket Audit Events
**File**: `crates/core/src/audit/events.rs` (modify)

- [ ] Add `TicketCreated` event type
- [ ] Add `TicketCancelled` event type
- [ ] Add `TicketStateChanged` event type (for future use)
- [ ] Update `AuditEventData` enum

### Task 5: Update Core Exports
**File**: `crates/core/src/lib.rs` (modify)

- [ ] Add `pub mod ticket`
- [ ] Export ticket types in prelude

### Task 6: Ticket API Handlers
**File**: `crates/server/src/api/tickets.rs` (new)

- [ ] `POST /api/v1/tickets` - create ticket
- [ ] `GET /api/v1/tickets/{id}` - get ticket
- [ ] `GET /api/v1/tickets` - list tickets with filters
- [ ] `DELETE /api/v1/tickets/{id}` - cancel ticket
- [ ] Request/response types with serde
- [ ] Error handling and status codes

### Task 7: Wire Up Routes
**File**: `crates/server/src/api/mod.rs` (modify)

- [ ] Add tickets module
- [ ] Register routes in router

### Task 8: Update AppState
**File**: `crates/server/src/state.rs` (modify)

- [ ] Add `TicketStore` to AppState
- [ ] Add constructor parameter

### Task 9: Update Server Startup
**File**: `crates/server/src/main.rs` (modify)

- [ ] Create `SqliteTicketStore` instance
- [ ] Run migrations for tickets table
- [ ] Pass to AppState

### Task 10: Integration Tests
**File**: `crates/server/tests/ticket_integration.rs` (new)

- [ ] Test create ticket
- [ ] Test get ticket
- [ ] Test list tickets with filters
- [ ] Test cancel ticket
- [ ] Test cancel already-cancelled ticket (409)
- [ ] Test get non-existent ticket (404)
- [ ] Verify audit events are created

## Manual Testing Guide

After implementation, test with these commands:

### 1. Start the server
```bash
cargo run -p torrentino-server
```

### 2. Create a ticket
```bash
curl -X POST http://localhost:8080/api/v1/tickets \
  -H "Content-Type: application/json" \
  -d '{
    "priority": 100,
    "query_context": {
      "tags": ["music", "flac"],
      "description": "Abbey Road by The Beatles"
    },
    "dest_path": "/media/music/beatles"
  }'
```

### 3. Get the ticket
```bash
curl http://localhost:8080/api/v1/tickets/{id}
```

### 4. List all tickets
```bash
curl "http://localhost:8080/api/v1/tickets?limit=10"
```

### 5. List pending tickets only
```bash
curl "http://localhost:8080/api/v1/tickets?state=pending"
```

### 6. Cancel the ticket
```bash
curl -X DELETE http://localhost:8080/api/v1/tickets/{id} \
  -H "Content-Type: application/json" \
  -d '{"reason": "Testing cancellation"}'
```

### 7. Verify audit log
```bash
curl "http://localhost:8080/api/v1/audit?event_type=ticket_created"
curl "http://localhost:8080/api/v1/audit?event_type=ticket_cancelled"
```

### 8. Try to cancel again (should fail with 409)
```bash
curl -X DELETE http://localhost:8080/api/v1/tickets/{id}
# Expected: 409 Conflict
```

## Success Criteria

- [ ] All 10 tasks implemented
- [ ] `cargo test` passes (unit + integration tests)
- [ ] `cargo clippy` has no warnings
- [ ] Manual testing guide works end-to-end
- [ ] Audit events recorded for all ticket operations
- [ ] State transitions enforced (can't cancel terminal tickets)

## Files Changed/Created

### New Files
- `crates/core/src/ticket/mod.rs`
- `crates/core/src/ticket/store.rs`
- `crates/core/src/ticket/sqlite_store.rs`
- `crates/server/src/api/tickets.rs`
- `crates/server/tests/ticket_integration.rs`

### Modified Files
- `crates/core/src/lib.rs`
- `crates/core/src/audit/events.rs`
- `crates/server/src/api/mod.rs`
- `crates/server/src/state.rs`
- `crates/server/src/main.rs`

## Estimated Complexity

| Task | Complexity | Notes |
|------|------------|-------|
| 1. Core Data Types | Low | Structs + enums |
| 2. Ticket Store Trait | Low | Trait definition |
| 3. SQLite Implementation | Medium | SQL + JSON handling |
| 4. Audit Events | Low | Add variants |
| 5. Core Exports | Low | Module exports |
| 6. API Handlers | Medium | HTTP handling |
| 7. Wire Routes | Low | Router config |
| 8. Update AppState | Low | Add field |
| 9. Server Startup | Low | Wiring |
| 10. Integration Tests | Medium | E2E tests |
