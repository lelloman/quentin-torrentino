# Quentin Torrentino

## Overview

**Quentin Torrentino** is a **content-agnostic** media acquisition system that can be used as:
- **Standalone service**: HTTP API for receiving download tickets
- **Rust library**: Embeddable in other applications

It uses a pluggable torrent search backend (Jackett, Prowlarr, etc.) for torrent search, optional LLM for intelligent matching, qBittorrent for downloading, and ffmpeg for conversion.

### Supported Content Types

| Content Type | Catalog App | Matching Strategy | Conversion |
|--------------|-------------|-------------------|------------|
| Music | Pezzottify | Artist/Album/Track mapping | Audio transcode + metadata |
| Movies | Pezzottflix | Title/Year/Quality matching | Video transcode (optional) |
| TV Series | Pezzottflix | Show/Season/Episode mapping | Video transcode (optional) |

> **Note:** Music support is implemented first. Video support will follow, reusing patterns from the music module.

### Architecture: Library + Service

```
┌─────────────────────────────────────────────────────────────────┐
│                  quentin-torrentino-core (library)               │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │ TorrentCatalog│  │   Searcher   │  │    TorrentClient       │ │
│  │              │  │  (abstract)  │  │    (qBittorrent)       │ │
│  └──────────────┘  └──────────────┘  └────────────────────────┘ │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │ QueueManager │  │ CacheWarmer  │  │    Placer              │ │
│  └──────────────┘  └──────────────┘  └────────────────────────┘ │
│                                                                  │
│  Traits (implemented per content type):                          │
│  ┌──────────────┐  ┌──────────────┐                             │
│  │ Matcher<T>   │  │ Converter<T> │                             │
│  └──────────────┘  └──────────────┘                             │
└─────────────────────────────────────────────────────────────────┘
           │                                    │
           ▼                                    ▼
┌─────────────────────┐              ┌─────────────────────┐
│ torrentino-music    │              │ torrentino-video    │
│                     │              │                     │
│ - AudioMatcher      │              │ - MovieMatcher      │
│ - AudioConverter    │              │ - TvMatcher         │
│ - MusicTicket       │              │ - VideoConverter    │
│                     │              │ - MovieTicket       │
│                     │              │ - TvTicket          │
└─────────────────────┘              └─────────────────────┘
           │                                    │
           ▼                                    ▼
┌─────────────────────┐              ┌─────────────────────┐
│ torrentino-server   │              │ pezzottflix-server  │
│ (for Pezzottify)    │              │ (for Pezzottflix)   │
└─────────────────────┘              └─────────────────────┘
```

## Runtime Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        QUENTIN TORRENTINO                               │
│                                                                         │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────────┐│
│  │   HTTP API   │────▶│    Queue     │────▶│      Processor           ││
│  │              │     │   Manager    │     │                          ││
│  │ - tickets    │     │              │     │  ┌────────────────────┐  ││
│  │ - status     │     │ - SQLite     │     │  │  1. Searcher       │  ││
│  │ - admin      │     │ - state      │     │  │     (pluggable)    │  ││
│  │ - websocket  │     │   machine    │     │  ├────────────────────┤  ││
│  └──────────────┘     └──────────────┘     │  │  2. Matcher        │  ││
│                                            │  │   (dumb or LLM)    │  ││
│  ┌──────────────┐                          │  ├────────────────────┤  ││
│  │    Auth      │                          │  │  3. Downloader     │  ││
│  │  (pluggable) │                          │  │     (qBittorrent)  │  ││
│  └──────────────┘                          │  ├────────────────────┤  ││
│                                            │  │  4. Converter      │  ││
│  ┌──────────────┐                          │  │     (ffmpeg)       │  ││
│  │  Audit Log   │                          │  ├────────────────────┤  ││
│  │              │                          │  │  5. Placer         │  ││
│  └──────────────┘                          │  │     (file ops)     │  ││
│                                            │  └────────────────────┘  ││
│                                            └──────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘
         ▲                                              │
         │ Tickets                                      │ Files placed at dest_path
         │                                              ▼
┌─────────────────┐                           ┌─────────────────┐
│    Consumer     │                           │   Media Storage │
│  (Pezzottify,   │                           │                 │
│   Pezzottflix)  │                           │                 │
└─────────────────┘                           └─────────────────┘
```

## Authentication

Authentication is **required** - the service will not start without an explicit auth configuration. This ensures operators are aware of their security posture.

### Pluggable Authenticators

```rust
#[async_trait]
trait Authenticator: Send + Sync {
    async fn authenticate(&self, request: &AuthRequest) -> Result<Identity, AuthError>;
}

struct AuthRequest {
    headers: HeaderMap,
    source_ip: IpAddr,
}

struct Identity {
    user_id: String,
    method: String,  // "oidc", "address", "cert", "plugin", "none"
    claims: HashMap<String, Value>,
}
```

### Available Authenticators

| Method | Use Case |
|--------|----------|
| `none` | Explicit no-auth (anonymous access, user_id = "anonymous") |
| `oidc` | JWT validation via OIDC provider |
| `address` | IP/subnet-based identity mapping |
| `cert` | Client certificate validation |
| `plugin` | External script/binary for custom auth |

### Configuration Examples

```toml
# REQUIRED - service will not start without [auth] section

# Option 1: Explicit no-auth (you must opt-in to anonymous access)
[auth]
method = "none"

# Option 2: OIDC/JWT
[auth]
method = "oidc"

[auth.oidc]
issuer = "https://auth.example.com"
audience = "quentin-torrentino"
jwks_url = "https://auth.example.com/.well-known/jwks.json"
user_id_claim = "sub"  # or "email", "preferred_username"

# Option 3: Address-based
[auth]
method = "address"

[auth.address]
rules = [
    { ip = "192.168.1.0/24", user_id = "homelab" },
    { ip = "10.0.0.5", user_id = "pezzottify-server" },
]

# Option 4: Client certificates
[auth]
method = "cert"

[auth.cert]
ca_cert = "/etc/ssl/ca.pem"
user_id_from = "cn"  # Extract user_id from certificate CN

# Option 5: Custom plugin
[auth]
method = "plugin"

[auth.plugin]
command = "/usr/local/bin/my-auth-plugin"
timeout_ms = 5000
# Plugin receives JSON on stdin: { "headers": {...}, "source_ip": "..." }
# Plugin returns JSON: { "user_id": "...", "claims": {...} } or { "error": "..." }
```

## Matching System

The matching system supports two modes: a deterministic "dumb" matcher and an LLM-powered intelligent matcher.

### Layered Matching Strategy

```
┌─────────────────────────────────────────────────────────┐
│                    Matching Pipeline                     │
│                                                          │
│  1. Search Query Generation                              │
│     ├─ Dumb: Template-based queries                     │
│     └─ LLM: Intelligent query variations                │
│                                                          │
│  2. Candidate Scoring                                    │
│     ├─ Dumb: Fuzzy string match + heuristics            │
│     └─ LLM: Semantic understanding + context            │
│                                                          │
│  3. Track Mapping (torrent files → ticket tracks)       │
│     ├─ Dumb: Filename-based fuzzy matching              │
│     └─ LLM: Intelligent file-to-track mapping           │
│                                                          │
│  4. Confidence Check                                     │
│     ├─ High confidence → Auto-approve                   │
│     └─ Low confidence → Needs manual approval           │
└─────────────────────────────────────────────────────────┘
```

### Dumb Matcher (No LLM Required)

Works for ~70-80% of mainstream music. Uses:
- Fuzzy string matching for artist/album names
- Format detection (FLAC, 320, V0) from torrent titles
- Red flag detection (karaoke, cover, tribute, compilation)
- Seeder count as tiebreaker
- Filename-based track mapping

### LLM Matcher (Optional, Recommended)

When configured, provides:
- Intelligent search query generation (name variations, transliterations)
- Semantic torrent title understanding
- Smart handling of edge cases (deluxe editions, remasters, regional variants)
- Accurate track-to-file mapping
- Detailed reasoning (logged for fine-tuning)

```toml
# Optional LLM configuration
[matcher.llm]
provider = "anthropic"  # or "openai", "deepseek", "ollama"
model = "claude-3-haiku-20240307"
api_key = "your-api-key"  # or use environment variable
api_base = "https://api.anthropic.com"  # optional, for custom endpoints
auto_approve_threshold = 0.85
```

### Future: Fine-Tuned Model

All LLM matching decisions are logged with full context. This data can be used to fine-tune a smaller model (Llama, Mistral, Qwen) for this specific task, enabling local inference.

## Audit Log System

Comprehensive audit logging is a first-class feature. Every significant event is logged for debugging, accountability, and LLM training data collection.

### Audit Events

```rust
enum AuditEvent {
    // Ticket lifecycle
    TicketCreated { ticket_id, requested_by, ticket_snapshot },
    TicketStateChanged { ticket_id, from_state, to_state, reason, actor },
    TicketCancelled { ticket_id, cancelled_by, reason },

    // Search
    SearchExecuted {
        ticket_id,
        searcher_name,  // "jackett", "prowlarr", etc.
        query,
        results_count,
        duration_ms
    },

    // Matching
    MatchingStarted { ticket_id, candidates_count, matcher_type },
    MatchingCompleted {
        ticket_id,
        matcher_type,  // "dumb" or "llm"
        llm_provider,  // if LLM
        llm_model,     // if LLM
        prompt_tokens, // if LLM
        completion_tokens, // if LLM
        candidates_scored: Vec<CandidateScore>,
        selected_idx,
        confidence,
        reasoning,  // Full reasoning for training data
    },

    // Approvals
    ApprovalRequested { ticket_id, reason, candidates },
    ApprovalGranted { ticket_id, approved_by, selected_candidate_idx },
    ApprovalRejected { ticket_id, rejected_by, reason },

    // Downloads
    DownloadStarted { ticket_id, info_hash, torrent_name },
    DownloadProgress { ticket_id, info_hash, percent, speed_bps },
    DownloadCompleted { ticket_id, info_hash, size_bytes, duration_secs },
    DownloadFailed { ticket_id, info_hash, error },

    // Conversion
    ConversionStarted { ticket_id, item_id, input_path },
    ConversionCompleted { ticket_id, item_id, output_path, duration_ms },
    ConversionFailed { ticket_id, item_id, error },

    // Placement
    PlacementStarted { ticket_id, files_count },
    PlacementCompleted { ticket_id, files_placed },
    PlacementFailed { ticket_id, error, rollback_status },

    // System
    ServiceStarted { version, config_hash },
    ServiceStopped { reason },
    ConfigValidationFailed { errors },

    // Admin actions
    AdminForceSearch { ticket_id, admin_id, custom_query },
    AdminForceMagnet { ticket_id, admin_id, magnet_uri },
    AdminRetry { ticket_id, admin_id },
}
```

### Storage

- **SQLite**: Queryable event store for runtime queries
- **Structured logs**: JSON Lines format for external ingestion

The consumer is responsible for log retention and extracting training data. Quentin Torrentino writes events; the operator decides what to keep.

### Query API

```
GET /api/v1/audit?ticket_id=xxx
GET /api/v1/audit?event_type=MatchingCompleted&from=2024-01-01
GET /api/v1/audit/export?format=jsonl
```

## Torrent Search Abstraction

The torrent search backend is abstracted to avoid hard dependencies on specific providers.

```rust
#[async_trait]
trait TorrentSearcher: Send + Sync {
    /// Provider name for audit logging
    fn name(&self) -> &str;

    /// Search for torrents matching the query
    async fn search(&self, query: &SearchQuery) -> Result<Vec<TorrentCandidate>>;

    /// Fetch file list if supported by the provider
    async fn get_files(&self, candidate: &TorrentCandidate) -> Result<Option<Vec<TorrentFile>>>;
}
```

### Supported Backends

| Backend | Status | Notes |
|---------|--------|-------|
| Jackett | Planned (first) | Aggregates multiple indexers |
| Prowlarr | Future | Modern Jackett alternative |
| Direct tracker API | Future | e.g., RED, OPS APIs |

## Ticket Structure

The ticket is the contract between consumers and Quentin Torrentino.

### Request Scope

Users can request:
- **Full album**: All tracks in the album
- **Individual tracks**: Specific tracks within an album (subset download)
- **Dry run**: Preview matching results without downloading

### Dry Run Mode

```json
{
  "dry_run": true,
  // ... rest of ticket
}
```

When `dry_run: true`:
- Runs search and matching normally
- Returns scored candidates with mappings
- Does NOT add to qBittorrent
- Transitions to `DryRunComplete` state (terminal)

Useful for testing matchers, previewing downloads, debugging search queries.

### Smart Torrent Selection

The system maintains a **torrent catalog** - a registry of seeded/cached torrents. When processing a request:

1. **Check torrent catalog first**: Are the requested tracks available in an already-seeded discography/collection?
2. **If yes**: Extract and convert only the needed tracks from the cached torrent (no new download)
3. **If no**: Search for new torrent, preferring discographies/collections over single albums
4. **Cache new torrents**: Add downloaded torrents to catalog for future requests

```
User requests: "Radiohead - Paranoid Android" (single track)

Torrent Catalog lookup:
  ├─ "Radiohead Discography FLAC" (seeded) → Contains track? YES → Extract from cache
  └─ No need to download anything new

User requests: "Aphex Twin - Windowlicker"

Torrent Catalog lookup:
  └─ No Aphex Twin torrents cached → Search → Download "Aphex Twin Discography" → Add to catalog → Extract track
```

### Ticket Deduplication

Multiple users may request the same content:

1. **On new request**: Check if an active ticket exists for overlapping content
2. **If overlap found**: Link the new request to the existing ticket
3. **On completion**: All linked requests are resolved together
4. **Partial overlap**: Requests are merged (tracks 1-5 + 3-8 → 1-8)

### Ticket JSON

```json
{
  "ticket_id": "uuid",
  "created_at": "2024-12-24T10:00:00Z",
  "requested_by": "user_id_from_auth",
  "dry_run": false,
  "linked_tickets": ["uuid2", "uuid3"],
  "request_scope": "full_album",

  "search": {
    "artist": "Radiohead",
    "album": "OK Computer",
    "year": 1997,
    "label": "Parlophone",
    "genres": ["alternative rock", "art rock"]
  },

  "tracks": [
    {
      "catalog_track_id": "t1",
      "disc_number": 1,
      "track_number": 1,
      "name": "Airbag",
      "duration_secs": 284,
      "dest_path": "/media/albums/abc123/d1t01.ogg",
      "requested": true
    }
  ],

  "images": [
    {
      "catalog_image_id": "img1",
      "type": "cover_front",
      "dest_path": "/media/albums/abc123/cover.jpg"
    }
  ],

  "constraints": {
    "format": "ogg_vorbis",
    "bitrate_kbps": 320,
    "sample_rate_hz": 44100,
    "embed_metadata": true,
    "embed_cover": true
  },

  "metadata_to_embed": {
    "artist": "Radiohead",
    "album": "OK Computer",
    "year": 1997,
    "genre": "Alternative Rock"
  }
}
```

## State Machine

```
┌─────────────┐
│   PENDING   │ ← Ticket received, queued for processing
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  SEARCHING  │ ← Querying torrent search backend for candidates
└──────┬──────┘
       │
       ├─────────────────────────────────┐
       │ candidates found               │ no candidates after all strategies
       ▼                                 ▼
┌─────────────┐                  ┌───────────────┐
│  MATCHING   │                  │ SEARCH_FAILED │ ← Awaits manual intervention
└──────┬──────┘                  └───────────────┘
       │                                 │
       │                      force-search / force-magnet
       │                                 │
       │                                 ▼
       │                         (back to SEARCHING or DOWNLOADING)
       │
       ├────────────────────────────────────────────────────────┐
       │                                                        │
       │ (if dry_run)                                          │
       ▼                                                        │
┌──────────────────┐                                           │
│  DRY_RUN_COMPLETE│ (terminal)                                │
└──────────────────┘                                           │
                                                               │
       ├─────────────────────────────────┐                     │
       │ confidence >= threshold         │ confidence < threshold
       ▼                                 ▼
┌───────────────┐               ┌────────────────────┐
│ AUTO_APPROVED │               │  NEEDS_APPROVAL    │ ← Admin must review
└───────┬───────┘               └─────────┬──────────┘
        │                                 │
        │                      ┌──────────┴──────────┐
        │                      │                     │
        │               admin approves          admin rejects
        │                      │                     │
        │                      ▼                     ▼
        │               ┌──────────────┐    ┌────────────┐
        │               │   APPROVED   │    │  REJECTED  │ (terminal)
        │               └──────┬───────┘    └────────────┘
        │                      │
        └──────────┬───────────┘
                   │
                   ▼
          ┌───────────────┐
          │  DOWNLOADING  │ ← Torrent downloading via qBittorrent
          └───────┬───────┘
                  │
                  ▼
          ┌───────────────┐
          │  CONVERTING   │ ← ffmpeg: transcode + embed metadata
          └───────┬───────┘
                  │
                  ▼
          ┌───────────────┐
          │    PLACING    │ ← Moving files to dest_path
          └───────┬───────┘
                  │
                  ▼
          ┌───────────────┐
          │   COMPLETED   │ (terminal)
          └───────────────┘

          (any non-terminal state)
                  │
               on error
                  ▼
          ┌───────────────┐
          │    FAILED     │ (terminal, may be retryable)
          └───────────────┘
```

### State Details

```rust
enum TicketState {
    Pending,

    Searching {
        strategy_idx: usize,
        strategies_tried: Vec<String>,
        started_at: DateTime<Utc>,
    },

    SearchFailed {
        strategies_tried: Vec<String>,
        failed_at: DateTime<Utc>,
    },

    Matching {
        candidates: Vec<TorrentCandidate>,
        started_at: DateTime<Utc>,
    },

    DryRunComplete {
        candidates: Vec<ScoredCandidate>,
        recommended_idx: usize,
        completed_at: DateTime<Utc>,
    },

    NeedsApproval {
        candidates: Vec<ScoredCandidate>,
        recommended_idx: usize,
        confidence: f32,
        reason: ApprovalReason,
        waiting_since: DateTime<Utc>,
    },

    AutoApproved {
        selected: ScoredCandidate,
        confidence: f32,
    },

    Approved {
        selected: ScoredCandidate,
        approved_by: String,
        approved_at: DateTime<Utc>,
    },

    Downloading {
        torrent_hash: String,
        progress_pct: f32,
        download_speed_bps: u64,
        eta_secs: Option<u32>,
        started_at: DateTime<Utc>,
    },

    Converting {
        current_track_idx: usize,
        total_tracks: usize,
        current_track_name: String,
        started_at: DateTime<Utc>,
    },

    Placing {
        files_placed: usize,
        total_files: usize,
        started_at: DateTime<Utc>,
    },

    Completed {
        completed_at: DateTime<Utc>,
        stats: CompletionStats,
    },

    Rejected {
        rejected_by: String,
        reason: Option<String>,
        rejected_at: DateTime<Utc>,
    },

    Failed {
        failed_at_state: String,
        error: String,
        retryable: bool,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    },
}

enum ApprovalReason {
    LowConfidence { score: f32, threshold: f32 },
    TrackCountMismatch { expected: usize, found: usize },
    DurationMismatch { expected_secs: u32, found_secs: u32 },
    NameSimilarityLow { similarity: f32 },
    MultipleGoodMatches { top_scores: Vec<f32> },
    NoExactMatch,
}

struct ScoredCandidate {
    torrent: TorrentCandidate,
    score: f32,
    track_mapping: Vec<TrackMapping>,
    reasoning: String,
}

struct TrackMapping {
    catalog_track_id: String,
    torrent_file_path: String,
    confidence: f32,
}

struct CompletionStats {
    total_download_bytes: u64,
    download_duration_secs: u32,
    conversion_duration_secs: u32,
    final_size_bytes: u64,
}
```

## API Endpoints

### Ticket Management

```
POST   /api/v1/ticket
       Body: Ticket JSON
       → Creates new ticket, returns ticket_id

GET    /api/v1/ticket/{ticket_id}
       → Returns full ticket state and history

GET    /api/v1/tickets
       Query params: ?state=needs_approval&limit=50&offset=0
       → Lists tickets with filtering

DELETE /api/v1/ticket/{ticket_id}
       → Cancels ticket (if not terminal)
```

### Admin Actions

```
POST   /api/v1/ticket/{ticket_id}/approve
       Body: { "candidate_idx": 0 }  (optional, uses recommended if omitted)
       → Approves ticket with selected candidate

POST   /api/v1/ticket/{ticket_id}/reject
       Body: { "reason": "Wrong album" }
       → Rejects ticket

POST   /api/v1/ticket/{ticket_id}/retry
       → Retries failed ticket from last safe state

POST   /api/v1/ticket/{ticket_id}/force-search
       Body: { "query": "custom search query" }
       → Manual search override (restarts from SEARCHING)

POST   /api/v1/ticket/{ticket_id}/force-magnet
       Body: { "magnet_uri": "magnet:?xt=..." }
       → Skip search entirely, go directly to DOWNLOADING
```

### Audit

```
GET    /api/v1/audit
       Query params: ?ticket_id=xxx&event_type=MatchingCompleted&from=2024-01-01
       → Query audit events

GET    /api/v1/audit/export
       Query params: ?format=jsonl
       → Export audit log for training data
```

### Status & Health

```
GET    /api/v1/health
       → Service health check

GET    /api/v1/stats
       → Queue stats, processing rates, etc.

GET    /api/v1/config
       → Current configuration (admin only)
```

### Real-time Updates

```
WS     /api/v1/ws
       → WebSocket for state change notifications

       Messages:
       - { "type": "state_change", "ticket_id": "...", "old_state": "...", "new_state": "...", "details": {...} }
       - { "type": "progress", "ticket_id": "...", "progress_pct": 45.2 }
       - { "type": "needs_approval", "ticket_id": "...", "candidates": [...] }
```

## Components

### 1. HTTP API (`api/`)

Axum-based HTTP server:
- Ticket CRUD endpoints
- Admin action endpoints
- WebSocket for real-time updates
- Pluggable authentication

### 2. Torrent Catalog (`torrent_catalog/`)

Database of seeded/cached torrents and their contents.

```rust
struct CachedTorrent {
    info_hash: String,
    title: String,
    indexer: String,
    download_path: PathBuf,
    size_bytes: u64,
    added_at: DateTime<Utc>,
    last_accessed: DateTime<Utc>,
    seed_ratio: f32,
    status: CachedTorrentStatus,
}

struct TorrentContent {
    info_hash: String,
    file_path: String,
    artist: Option<String>,
    album: Option<String>,
    track_name: Option<String>,
    duration_secs: Option<u32>,
    catalog_track_id: Option<String>,
}

#[async_trait]
trait TorrentCatalog {
    async fn find_coverage(&self, tracks: &[TrackRequest]) -> Vec<CoverageMatch>;
    async fn register_torrent(&self, torrent: CachedTorrent, contents: Vec<TorrentContent>) -> Result<()>;
    async fn get_uncovered_artists(&self, limit: usize) -> Vec<ArtistCoverageGap>;
    async fn touch(&self, info_hash: &str) -> Result<()>;
    async fn get_cleanup_candidates(&self, max_size_bytes: u64) -> Vec<CachedTorrent>;
}
```

### 3. Cache Warmer (`cache_warmer/`)

Background job that proactively builds the torrent catalog.

```rust
struct CacheWarmer {
    catalog: Arc<dyn TorrentCatalog>,
    searcher: Arc<dyn TorrentSearcher>,
    matcher: Arc<dyn Matcher>,
    torrent_client: Arc<dyn TorrentClient>,
    config: CacheWarmerConfig,
}

struct CacheWarmerConfig {
    enabled: bool,
    scan_interval_hours: u32,
    max_concurrent_downloads: usize,
    storage_budget_bytes: u64,
    min_artist_popularity: u32,
}
```

### 4. Queue Manager (`queue/`)

- SQLite-backed ticket persistence
- State machine enforcement
- Retry logic with exponential backoff
- Priority handling

### 5. Searcher (`searcher/`)

Pluggable torrent search backend:

```rust
#[async_trait]
trait TorrentSearcher: Send + Sync {
    fn name(&self) -> &str;
    async fn search(&self, query: &SearchQuery) -> Result<Vec<TorrentCandidate>>;
    async fn get_files(&self, candidate: &TorrentCandidate) -> Result<Option<Vec<TorrentFile>>>;
}

struct TorrentCandidate {
    title: String,
    indexer: String,
    magnet_uri: String,
    info_hash: String,
    size_bytes: u64,
    seeders: u32,
    leechers: u32,
    files: Option<Vec<TorrentFile>>,
    publish_date: DateTime<Utc>,
}
```

### 6. Matcher (`matcher/`)

Layered matching with optional LLM:

```rust
#[async_trait]
trait Matcher<T: Ticket>: Send + Sync {
    async fn score_candidates(
        &self,
        ticket: &T,
        candidates: Vec<TorrentCandidate>,
    ) -> Result<Vec<ScoredCandidate<T::ContentItem>>>;
}

// Implementations:
struct DumbMatcher;  // Fuzzy string matching + heuristics
struct LlmMatcher;   // Intelligent semantic matching
```

### 7. Torrent Client (`torrent_client/`)

qBittorrent Web API integration:

```rust
#[async_trait]
trait TorrentClient {
    async fn add_torrent(&self, magnet: &str, save_path: &Path) -> Result<String>;
    async fn get_progress(&self, hash: &str) -> Result<TorrentProgress>;
    async fn get_files(&self, hash: &str) -> Result<Vec<TorrentFile>>;
    async fn remove_torrent(&self, hash: &str, delete_files: bool) -> Result<()>;
}
```

### 8. Converter (`converter/`)

ffmpeg wrapper:

```rust
struct ConversionJob {
    input_path: PathBuf,
    output_path: PathBuf,
    format: AudioFormat,
    bitrate_kbps: u32,
    sample_rate_hz: u32,
    metadata: HashMap<String, String>,
    cover_art: Option<PathBuf>,
}

#[async_trait]
trait Converter<T: Ticket>: Send + Sync {
    async fn convert(
        &self,
        source: &Path,
        item: &T::ContentItem,
        constraints: &ConversionConstraints,
    ) -> Result<ConversionResult>;

    async fn validate(&self, path: &Path) -> Result<MediaInfo>;
}
```

### 9. Placer (`placer/`)

File operations:
- Move converted files to dest_path
- Create directories as needed
- Verify file integrity after move
- Cleanup temp files
- Rollback on partial failure

## Cover Art

Cover art is fetched from multiple sources with fallback:

1. **MusicBrainz Cover Art Archive** (primary) - CC0, no API key needed
2. **Discogs** (fallback) - Requires API key, rate limited
3. **Torrent files** (last resort) - Extract from embedded tags or folder.jpg

## Configuration

```toml
# ==============================================================================
# AUTHENTICATION (REQUIRED - service will not start without this)
# ==============================================================================

[auth]
method = "none"  # "none", "oidc", "address", "cert", "plugin"

# See Authentication section for method-specific config

# ==============================================================================
# SERVER
# ==============================================================================

[server]
host = "0.0.0.0"
port = 8080

# ==============================================================================
# DATABASE
# ==============================================================================

[database]
path = "/data/quentin.db"

# ==============================================================================
# TORRENT SEARCH
# ==============================================================================

[searcher]
backend = "jackett"  # or "prowlarr" (future)

[searcher.jackett]
url = "http://localhost:9117"
api_key = "your-jackett-api-key"
timeout_secs = 30

[[searcher.jackett.indexers]]
name = "rutracker"
rate_limit_requests_per_min = 10

[[searcher.jackett.indexers]]
name = "redacted"
rate_limit_requests_per_min = 5

# ==============================================================================
# TORRENT CLIENT
# ==============================================================================

[qbittorrent]
url = "http://localhost:8081"
username = "admin"
password = "adminadmin"
download_path = "/downloads/incomplete"
seed_ratio_target = 1.0
seed_time_limit_mins = 0  # 0 = no limit
cleanup_policy = "after_ratio"  # "immediate", "after_ratio", "manual"

# ==============================================================================
# MATCHER
# ==============================================================================

[matcher]
type = "dumb"  # or "llm"
auto_approve_threshold = 0.85

# Optional LLM configuration
[matcher.llm]
provider = "anthropic"  # "openai", "deepseek", "ollama"
model = "claude-3-haiku-20240307"
api_key = "your-api-key"
api_base = "https://api.anthropic.com"  # optional

# ==============================================================================
# CONVERTER
# ==============================================================================

[converter]
ffmpeg_path = "/usr/bin/ffmpeg"
ffprobe_path = "/usr/bin/ffprobe"
temp_dir = "/tmp/quentin"
max_parallel_conversions = 4

# ==============================================================================
# PROCESSING
# ==============================================================================

[processing]
max_parallel_downloads = 2
check_interval_secs = 10
retry_max_attempts = 3
retry_initial_delay_secs = 60
retry_max_delay_secs = 3600

# ==============================================================================
# AUDIT LOG
# ==============================================================================

[audit]
# SQLite storage is automatic (same DB as tickets)
# Structured log output (optional)
log_file = "/var/log/quentin/audit.jsonl"  # optional
```

## Crate Structure (Workspace)

```
quentin-torrentino/
├── Cargo.toml                    # Workspace root
│
├── crates/
│   ├── core/                     # torrentino-core (library)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs
│   │       ├── auth/
│   │       │   ├── mod.rs
│   │       │   ├── oidc.rs
│   │       │   ├── address.rs
│   │       │   ├── cert.rs
│   │       │   └── plugin.rs
│   │       ├── audit/
│   │       │   ├── mod.rs
│   │       │   ├── events.rs
│   │       │   └── store.rs
│   │       ├── torrent_catalog/
│   │       │   ├── mod.rs
│   │       │   ├── store.rs
│   │       │   └── coverage.rs
│   │       ├── cache_warmer/
│   │       │   ├── mod.rs
│   │       │   └── priority.rs
│   │       ├── queue/
│   │       │   ├── mod.rs
│   │       │   ├── manager.rs
│   │       │   ├── state.rs
│   │       │   └── store.rs
│   │       ├── searcher/
│   │       │   ├── mod.rs
│   │       │   ├── traits.rs
│   │       │   ├── jackett.rs
│   │       │   └── rate_limiter.rs
│   │       ├── torrent_client/
│   │       │   ├── mod.rs
│   │       │   └── qbittorrent.rs
│   │       ├── placer/
│   │       │   ├── mod.rs
│   │       │   └── file_ops.rs
│   │       ├── traits/
│   │       │   ├── mod.rs
│   │       │   ├── matcher.rs
│   │       │   ├── converter.rs
│   │       │   └── ticket.rs
│   │       └── models/
│   │           ├── mod.rs
│   │           ├── state.rs
│   │           ├── torrent.rs
│   │           └── catalog.rs
│   │
│   ├── music/                    # torrentino-music (content-specific)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ticket.rs
│   │       ├── matcher/
│   │       │   ├── mod.rs
│   │       │   ├── dumb.rs
│   │       │   └── llm.rs
│   │       ├── converter.rs
│   │       ├── metadata.rs
│   │       └── cover_art.rs
│   │
│   ├── video/                    # torrentino-video (future)
│   │   └── ...
│   │
│   └── server/                   # torrentino-server (HTTP service)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── api/
│           │   ├── mod.rs
│           │   ├── routes.rs
│           │   ├── handlers.rs
│           │   ├── websocket.rs
│           │   └── auth_middleware.rs
│           └── bin/
│               └── quentin.rs
│
├── tests/
│   ├── integration/
│   └── mocks/
│
└── docker/
    ├── Dockerfile
    └── docker-compose.example.yml
```

## Core Traits

```rust
// crates/core/src/traits/ticket.rs

pub trait Ticket: Send + Sync + 'static {
    type ContentItem: ContentItem;

    fn id(&self) -> &str;
    fn requested_by(&self) -> &str;
    fn is_dry_run(&self) -> bool;
    fn search_query(&self) -> SearchQuery;
    fn items(&self) -> &[Self::ContentItem];
    fn constraints(&self) -> &ConversionConstraints;
}

pub trait ContentItem: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn dest_path(&self) -> &Path;
    fn is_requested(&self) -> bool;
    fn expected_duration(&self) -> Option<Duration>;
}

// crates/core/src/traits/matcher.rs

#[async_trait]
pub trait Matcher<T: Ticket>: Send + Sync {
    async fn score_candidates(
        &self,
        ticket: &T,
        candidates: Vec<TorrentCandidate>,
    ) -> Result<Vec<ScoredCandidate<T::ContentItem>>>;
}

// crates/core/src/traits/converter.rs

#[async_trait]
pub trait Converter<T: Ticket>: Send + Sync {
    async fn convert(
        &self,
        source: &Path,
        item: &T::ContentItem,
        constraints: &ConversionConstraints,
    ) -> Result<ConversionResult>;

    async fn validate(&self, path: &Path) -> Result<MediaInfo>;
}
```

## Deployment

### Docker

```dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/quentin /usr/local/bin/
ENTRYPOINT ["quentin"]
```

### Docker Compose Example

```yaml
services:
  quentin-torrentino:
    image: quentin-torrentino:latest
    volumes:
      - ./config.toml:/etc/quentin/config.toml:ro
      - ./data:/data
      - /media:/media
    environment:
      - RUST_LOG=info
    ports:
      - "8080:8080"
    restart: unless-stopped

  # User provides their own qBittorrent, Jackett, etc.
```

## Testing Strategy

### Unit Tests
- State machine transitions
- Ticket validation
- Dumb matcher scoring logic
- Auth validation

### Integration Tests (with mocks)
- API endpoint tests
- Queue processing with mocked external services
- Converter with real ffmpeg but test files
- Auth middleware with test tokens

### End-to-End Tests
- Docker compose with real Jackett + qBittorrent
- Test with legal/free torrents (e.g., creative commons music)

## Implementation Phases

**Phase 1: Core Library (`torrentino-core`)**
- [ ] Workspace setup
- [ ] Core traits: `Ticket`, `ContentItem`, `Matcher`, `Converter`
- [ ] Auth system (all authenticators)
- [ ] Audit log system
- [ ] SQLite schema + migrations
- [ ] State machine implementation
- [ ] Queue manager
- [ ] Configuration loading (with auth validation)

**Phase 2: External Integrations**
- [ ] Jackett client + per-indexer rate limiting
- [ ] qBittorrent client
- [ ] Torrent catalog
- [ ] Searcher trait + Jackett implementation

**Phase 3: Processing Pipeline**
- [ ] ffmpeg wrapper (audio first)
- [ ] Placer with rollback
- [ ] End-to-end processing with dumb matcher

**Phase 4: Music Module (`torrentino-music`)**
- [ ] MusicTicket implementation
- [ ] Dumb matcher for music
- [ ] Audio converter
- [ ] Cover art fetching (MusicBrainz, Discogs, embedded)
- [ ] Metadata embedding

**Phase 5: Smart Matching (Optional)**
- [ ] LLM integration (Anthropic/OpenAI/DeepSeek/Ollama)
- [ ] LLM matcher implementation
- [ ] Approval workflow
- [ ] Training data export

**Phase 6: Production Ready**
- [ ] HTTP server (`torrentino-server`)
- [ ] WebSocket real-time updates
- [ ] Retry logic with exponential backoff
- [ ] Metrics/observability
- [ ] Docker packaging
- [ ] Comprehensive testing

**Phase 7: Video Module (Future)**
- [ ] `torrentino-video` crate
- [ ] Movie/TV matchers
- [ ] Video converter
- [ ] Subtitle handling

## Design Decisions

1. **Auth required**: Service exits on startup if no auth configured. Must explicitly opt-in to `method = "none"` for anonymous access.

2. **Seeding policy**: Ethical approach - seed until ratio >= 1.0 before cleanup eligible.

3. **Rate limiting**: Per-indexer limits for private trackers.

4. **LLM optional**: Dumb matcher works for most cases. LLM enhances accuracy when configured.

5. **Audit everything**: All significant events logged. Consumer handles retention and training data extraction.

6. **Fail-safe placement**: If any file fails to place, rollback all placed files for that ticket.

## Future Enhancements (Not In Scope Yet)

- Notification system (webhooks, email, Telegram with interactive approvals)
- Bulk admin operations
- Priority queues
- Fine-tuned local LLM for matching
- Cache warmer for proactive downloading
