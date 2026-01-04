# Quentin Torrentino

## Overview

**Quentin Torrentino** is a **content-agnostic** media acquisition system that can be used as:
- **Standalone service**: HTTP API for receiving download tickets
- **Rust library**: Embeddable in other applications

It uses a pluggable torrent search backend (Jackett, Prowlarr, etc.) for torrent search, optional LLM for intelligent matching, qBittorrent for downloading, and ffmpeg for conversion.

### Supported Content Types

| Content Type | Example Use Case | Matching Strategy |
|--------------|------------------|-------------------|
| Music | Pezzottify | Artist/Album/Track mapping |
| Movies | Pezzottflix | Title/Year/Quality matching |
| TV Series | Pezzottflix | Show/Season/Episode mapping |
| Software | - | Name/Version matching |
| Ebooks | - | Title/Author matching |

The system is **content-agnostic** - the ticket structure hints at content type, and TextBrain adapts its query building and matching strategies accordingly.

> **Note:** Music support is implemented first. Other content types follow the same patterns.

### Architecture: Library + Service

```
┌──────────────────────────────────────────────────────────────────────┐
│                    quentin-torrentino-core (library)                  │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │                         TextBrain                                │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────────┐ │ │
│  │  │ DumbQueryBuilder│  │   DumbMatcher   │  │ LlmClient (opt)  │ │ │
│  │  └─────────────────┘  └─────────────────┘  │ - Anthropic      │ │ │
│  │                                            │ - OpenAI         │ │ │
│  │  Modes: dumb-only | dumb-first |           │ - Ollama         │ │ │
│  │         llm-first | llm-only               │ - Custom HTTP    │ │ │
│  └─────────────────────────────────────────────┴──────────────────┘ │
│                                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────────┐ │
│  │TorrentCatalog│  │   Searcher   │  │      TorrentClient         │ │
│  │ (cache)      │  │  (Jackett)   │  │  (librqbit / qBittorrent)  │ │
│  └──────────────┘  └──────────────┘  └────────────────────────────┘ │
│                                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────────┐ │
│  │ QueueManager │  │  Converter   │  │         Placer             │ │
│  └──────────────┘  └──────────────┘  └────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘
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

### Orchestrator Architecture (Phase 5a)

The `TicketOrchestrator` is a background service that drives tickets through the state machine automatically:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          TICKET ORCHESTRATOR                                 │
│                                                                              │
│  ┌────────────────────┐     ┌────────────────────┐     ┌─────────────────┐  │
│  │  Acquisition       │     │  Download          │     │  Pipeline       │  │
│  │  Worker            │     │  Worker            │     │  Trigger        │  │
│  │                    │     │                    │     │                 │  │
│  │ polls: Pending     │     │ polls: Approved    │     │ watches:        │  │
│  │ calls: TextBrain   │     │ calls: TorrentClient│    │ Downloading     │  │
│  │ outputs:           │     │ outputs:           │     │ complete        │  │
│  │  - AutoApproved    │────▶│  - Downloading     │────▶│                 │  │
│  │  - NeedsApproval   │     │  - download done   │     │ submits to:     │  │
│  │  - AcqFailed       │     │  - Failed          │     │ PipelineProcessor│ │
│  └────────────────────┘     └────────────────────┘     └─────────────────┘  │
│           │                          │                          │           │
│           │ ┌────────────────────────┴──────────────────────────┘           │
│           │ │                                                               │
│           ▼ ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Ticket Store (SQLite)                        │   │
│  │  Pending → Acquiring → AutoApproved → Downloading → Converting → ... │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘

State Flow:
  Pending ──[AcquisitionWorker]──▶ Acquiring ──▶ NeedsApproval ──[user]──▶ Approved ─┐
                                       │                                              │
                                       └──▶ AutoApproved ─────────────────────────────┤
                                                                                      │
  ┌───────────────────────────────────────────────────────────────────────────────────┘
  │
  └──[DownloadWorker]──▶ Downloading ──▶ [PipelineTrigger] ──▶ Converting ──▶ Placing ──▶ Completed
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

## TextBrain: Query Building + Matching

TextBrain is the central intelligence component that handles search query generation and result matching. It coordinates between fast heuristic-based ("dumb") methods and optional LLM-powered intelligence.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          TextBrain                               │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Core Components                        │   │
│  │  ┌─────────────────┐        ┌─────────────────┐          │   │
│  │  │ DumbQueryBuilder│        │   DumbMatcher   │          │   │
│  │  │ (always avail)  │        │ (always avail)  │          │   │
│  │  └─────────────────┘        └─────────────────┘          │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                 LlmClient (optional)                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐  │   │
│  │  │Anthropic │ │  OpenAI  │ │  Ollama  │ │ Custom HTTP │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └─────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Coordination Modes:                                             │
│  • dumb-only:  Heuristics only, no LLM                          │
│  • dumb-first: Heuristics, then LLM if low confidence           │
│  • llm-first:  LLM preferred, heuristics as fallback            │
│  • llm-only:   Require LLM, fail if unavailable                 │
└─────────────────────────────────────────────────────────────────┘
```

### Pipeline

```
Ticket
   │
   ▼
┌─────────────────────┐
│ 1. Query Building   │  Generate search queries
│    (Dumb and/or LLM)│  from ticket metadata
└──────────┬──────────┘
           ▼
┌─────────────────────┐
│ 2. Search           │  Query Jackett + cache
│    (Searcher)       │
└──────────┬──────────┘
           ▼
┌─────────────────────┐
│ 3. Candidate Scoring│  Score results against
│    (Dumb and/or LLM)│  ticket requirements
└──────────┬──────────┘
           ▼
┌─────────────────────┐
│ 4. File Mapping     │  Map torrent files to
│    (Dumb and/or LLM)│  ticket items
└──────────┬──────────┘
           ▼
┌─────────────────────┐
│ 5. Selection        │  Auto-select if high
│                     │  confidence, else user
└─────────────────────┘
```

### Dumb Components (Always Available)

**DumbQueryBuilder** - Template-based query generation:
- `"{artist} {album}"`
- `"{artist} discography FLAC"`
- Format variations, common misspellings

**DumbMatcher** - Heuristic scoring:
- Fuzzy string matching (artist, album, track names)
- Format detection (FLAC, 320, V0) from torrent titles
- Red flag detection (karaoke, cover, tribute, compilation)
- Seeder count and size as tiebreakers
- Filename-based track mapping

Works for ~70-80% of mainstream content without any LLM.

### LLM Enhancement (Optional)

When configured, LLM provides:
- Intelligent query variations (transliterations, alternate names)
- Semantic title understanding ("The White Album" → "The Beatles")
- Edge case handling (deluxe editions, remasters, regional variants)
- Accurate track-to-file mapping for complex structures
- Detailed reasoning (logged for training data)

```toml
[textbrain]
mode = "dumb-first"  # or "dumb-only", "llm-first", "llm-only"
auto_approve_threshold = 0.85

[textbrain.llm]
provider = "anthropic"  # or "openai", "ollama", "custom"
model = "claude-3-haiku-20240307"
api_key = "your-api-key"  # or use environment variable
api_base = "https://api.anthropic.com"  # optional, for custom endpoints

# For Ollama (local)
# provider = "ollama"
# model = "llama2"
# api_base = "http://localhost:11434"

# For custom HTTP endpoint
# provider = "custom"
# api_base = "https://my-llm-proxy.example.com/v1"
```

### Training Data Collection

All TextBrain decisions are logged with full context:
- Input ticket
- Generated queries
- Candidate scores with reasoning
- File mappings
- Which method (dumb/LLM) produced each result
- User corrections (when manual selection differs from auto)

This data enables fine-tuning smaller models for local inference, reducing latency and API costs.

## Content-Specific Logic

Content-specific behavior for query building, scoring, and post-processing is dispatched based on the `ExpectedContent` type in the ticket. No plugin system - just organized code with match-based dispatch.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Orchestrator                            │
│                                                              │
│  ticket.query_context.expected = ExpectedContent::Album     │
│                           │                                  │
│                           ▼                                  │
│                   match expected {                           │
│                     Album/Track => content::music::*         │
│                     Movie/TvEpisode => content::video::*     │
│                     _ => content::generic::*                 │
│                   }                                          │
└─────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
│    music.rs         │  │    video.rs         │  │   generic.rs        │
│                     │  │                     │  │                     │
│ handles:            │  │ handles:            │  │ handles:            │
│  - Album            │  │  - Movie            │  │  - everything else  │
│  - Track            │  │  - TvEpisode        │  │  - fallback         │
│                     │  │                     │  │                     │
│ features:           │  │ features:           │  │ features:           │
│  - MusicBrainz      │  │  - TMDB lookup      │  │  - basic fuzzy      │
│  - Cover art        │  │  - Subtitle fetch   │  │    matching         │
│  - Audio metadata   │  │  - Release groups   │  │  - generic queries  │
│  - Format detection │  │  - Resolution/codec │  │                     │
└─────────────────────┘  └─────────────────────┘  └─────────────────────┘
```

### Dispatch Functions

The `content` module provides dispatch functions that route to the appropriate content-specific logic:

```rust
// content/mod.rs - dispatch based on ExpectedContent
pub fn build_queries(context: &QueryContext, config: &TextBrainConfig) -> QueryBuildResult {
    match &context.expected {
        Some(ExpectedContent::Album { .. }) |
        Some(ExpectedContent::Track { .. }) => music::build_queries(context, config),

        Some(ExpectedContent::Movie { .. }) |
        Some(ExpectedContent::TvEpisode { .. }) => video::build_queries(context, config),

        _ => generic::build_queries(context, config),
    }
}

pub fn score_candidate(context: &QueryContext, candidate: &TorrentCandidate, config: &TextBrainConfig) -> ScoredCandidate {
    match &context.expected {
        Some(ExpectedContent::Album { .. }) |
        Some(ExpectedContent::Track { .. }) => music::score_candidate(context, candidate, config),
        // ... same pattern
    }
}

pub async fn post_process(ticket: &Ticket, download_path: &Path) -> Result<PostProcessResult> {
    match &ticket.query_context.expected {
        Some(ExpectedContent::Album { .. }) |
        Some(ExpectedContent::Track { .. }) => music::post_process(ticket, download_path).await,
        // ... same pattern
    }
}
```

### LLM Integration

LLM is configured at the instance level (`[textbrain.llm]`), not per content type. Content-specific code can use LLM when available:

```rust
pub fn build_queries(context: &QueryContext, config: &TextBrainConfig) -> QueryBuildResult {
    // Content-specific heuristics
    let mut queries = build_music_queries(context);

    // Enhance with LLM if configured
    if config.mode.can_use_llm() && config.llm.is_some() {
        queries.extend(llm_enhanced_queries(context, config));
    }

    queries
}
```

### Content Types

#### Music (Album, Track)

| Feature | Description |
|---------|-------------|
| **Query Building** | `"{artist} {album}"`, `"{artist} discography FLAC"`, handles transliterations |
| **Scoring** | Track count validation, duration tolerance (±5s), format detection (FLAC/320/V0) |
| **File Mapping** | Match files to tracks by name, number, duration |
| **Post-Processing** | Fetch cover art (MusicBrainz CAA → Discogs → embedded) |
| **API Routes** | `POST /api/v1/music/album` - lookup album, auto-populate tracks |

#### VideoModule

| Feature | Description |
|---------|-------------|
| **Query Building** | `"{title} {year}"`, `"{series} S{season}"`, release group patterns |
| **Scoring** | Resolution detection (1080p/4K), codec preferences, release group ranking |
| **File Mapping** | Episode number extraction (S01E01), movie file detection |
| **Post-Processing** | Fetch subtitles (OpenSubtitles), extract embedded subs |
| **API Routes** | `POST /api/v1/video/movie`, `POST /api/v1/video/episode` - TMDB lookup |

#### GenericModule

Fallback for unrecognized content types or when `expected` is `None`:
- Basic fuzzy string matching on description
- Generic query patterns from tags
- No post-processing

### Configuration

```toml
[modules]
# Modules are loaded in order, first match wins
enabled = ["music", "video", "generic"]

[modules.music]
musicbrainz_rate_limit_per_sec = 1.0
cover_art_sources = ["musicbrainz", "discogs", "embedded"]
discogs_token = "optional-for-higher-limits"

[modules.video]
tmdb_api_key = "your-tmdb-key"
subtitle_languages = ["en", "es"]
opensubtitles_api_key = "optional"
preferred_resolution = "1080p"
```

### Integration with Orchestrator

```
Ticket Created
      │
      ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Acquisition Worker                            │
│                                                                  │
│  module = registry.get_module(ticket)  ←── dispatch by expected │
│                                                                  │
│  queries = module.build_queries(ticket)                         │
│  candidates = searcher.search(queries)                          │
│  scored = module.score_candidate(ticket, each candidate)        │
│  mapping = module.map_files(ticket, best_candidate.files)       │
│                                                                  │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Download Worker                               │
│                                                                  │
│  ... download completes ...                                     │
│                                                                  │
│  module.post_process(ticket, download_path)  ←── fetch assets   │
│                                                                  │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
                    Pipeline Processor
                    (conversion + placement)
```

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

### Ticket Priority

Tickets have a priority field (`u16`) for queue ordering. Higher value = higher priority.

```rust
struct Ticket {
    // ...
    priority: u16,  // 0 = lowest, u16::MAX = highest
}
```

All processing pools use priority queues, so high-priority tickets are processed first within each stage.

### Ticket JSON

```json
{
  "ticket_id": "uuid",
  "created_at": "2024-12-24T10:00:00Z",
  "requested_by": "user_id_from_auth",
  "priority": 100,
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
│  ACQUIRING  │ ← TextBrain: query building + search + scoring loop
└──────┬──────┘
       │
       ├─────────────────────────────────┐
       │ match found                     │ exhausted, no suitable match
       │                                 ▼
       │                    ┌──────────────────────┐
       │                    │  ACQUISITION_FAILED  │ ← Awaits manual intervention
       │                    └──────────────────────┘
       │                                 │
       │                      force-search / force-magnet
       │                                 │
       ├─────────────────────────────────┘
       │
       ├─────────────────────────────────┐
       │ confidence >= threshold         │ confidence < threshold
       ▼                                 ▼
┌───────────────┐               ┌────────────────────┐
│ AUTO_APPROVED │               │  NEEDS_APPROVAL    │ ← User must review
└───────┬───────┘               └─────────┬──────────┘
        │                                 │
        │                      ┌──────────┴──────────┐
        │                      │                     │
        │                user approves          user rejects
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
          │  DOWNLOADING  │ ← Torrent downloading via torrent client
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
            on error / user cancels
                  ▼
          ┌───────────────┐
          │ FAILED/CANCEL │ (terminal, may be retryable)
          └───────────────┘
```

### State Details

```rust
enum TicketState {
    /// Ticket created, waiting to be processed.
    Pending,

    /// TextBrain is acquiring a torrent (building queries, searching, scoring).
    /// This is a combined phase that handles the full query -> search -> score loop.
    Acquiring {
        started_at: DateTime<Utc>,
        queries_tried: Vec<String>,
        candidates_found: u32,
        phase: AcquisitionPhase,
    },

    /// Acquisition failed - no suitable torrent found after exhausting all strategies.
    AcquisitionFailed {
        queries_tried: Vec<String>,
        candidates_seen: u32,
        reason: String,
        failed_at: DateTime<Utc>,
    },

    /// Candidates found but confidence is below threshold - needs manual approval.
    NeedsApproval {
        candidates: Vec<ScoredCandidateSummary>,
        recommended_idx: usize,
        confidence: f32,
        waiting_since: DateTime<Utc>,
    },

    /// Automatically approved (confidence >= threshold).
    AutoApproved {
        selected: SelectedCandidate,
        confidence: f32,
        approved_at: DateTime<Utc>,
    },

    /// Manually approved by user.
    Approved {
        selected: SelectedCandidate,
        approved_by: String,
        approved_at: DateTime<Utc>,
    },

    /// Torrent is being downloaded.
    Downloading {
        info_hash: String,
        progress_pct: f32,
        speed_bps: u64,
        eta_secs: Option<u32>,
        started_at: DateTime<Utc>,
    },

    /// Converting downloaded files (transcoding, metadata embedding).
    Converting {
        current_idx: usize,
        total: usize,
        current_name: String,
        started_at: DateTime<Utc>,
    },

    /// Placing converted files to their final destinations.
    Placing {
        files_placed: usize,
        total_files: usize,
        started_at: DateTime<Utc>,
    },

    /// Ticket completed successfully (terminal).
    Completed {
        completed_at: DateTime<Utc>,
        stats: CompletionStats,
    },

    /// Rejected by user (terminal).
    Rejected {
        rejected_by: String,
        reason: Option<String>,
        rejected_at: DateTime<Utc>,
    },

    /// Ticket failed (terminal, may be retryable).
    Failed {
        error: String,
        retryable: bool,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    },

    /// Ticket was cancelled by user (terminal).
    Cancelled {
        cancelled_by: String,
        reason: Option<String>,
        cancelled_at: DateTime<Utc>,
    },
}

/// Current phase within the Acquiring state.
enum AcquisitionPhase {
    QueryBuilding,
    Searching { query: String },
    Scoring { candidates_count: u32 },
}

/// Summary of a scored candidate for storage in ticket state.
struct ScoredCandidateSummary {
    title: String,
    info_hash: String,
    size_bytes: u64,
    seeders: u32,
    score: f32,
    reasoning: String,
}

/// Summary of the selected candidate for approved states.
struct SelectedCandidate {
    title: String,
    info_hash: String,
    magnet_uri: String,
    size_bytes: u64,
    score: f32,
}

struct CompletionStats {
    total_download_bytes: u64,
    download_duration_secs: u32,
    conversion_duration_secs: u32,
    final_size_bytes: u64,
    files_placed: u32,
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
WS     /api/v1/ws?include_terminal=false
       → WebSocket for state change notifications
```

#### Connection Protocol

On connect, server sends a snapshot of current tickets for state consistency:

```json
← { "type": "snapshot", "seq": 1043, "tickets": [...active tickets...] }
← { "type": "state_change", "seq": 1044, ... }
← { "type": "state_change", "seq": 1045, ... }
```

Query parameters:
- `include_terminal=false` (default): Snapshot only includes active (non-terminal) tickets
- `include_terminal=true`: Snapshot includes all tickets (may be large)

#### Subscription Filtering (Optional)

By default, clients receive all events (admin/firehose mode). Optionally filter:

```json
→ { "action": "subscribe", "ticket_id": "uuid1" }
← { "type": "subscribed", "ticket_id": "uuid1" }

→ { "action": "subscribe", "filter": { "requested_by": "pezzottify" } }
← { "type": "subscribed", "filter": {...} }

→ { "action": "unsubscribe", "ticket_id": "uuid1" }
```

#### Event Types

```json
{ "type": "state_change", "seq": 1044, "ticket_id": "...", "old_state": "...", "new_state": "...", "details": {...} }
{ "type": "progress", "seq": 1045, "ticket_id": "...", "progress_pct": 45.2 }
{ "type": "needs_approval", "seq": 1046, "ticket_id": "...", "candidates": [...] }
```

The `seq` field is a monotonic sequence number for consistency tracking. If a client reconnects and detects a gap in sequence numbers, it should refetch state.

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

### 3. Shadow Catalog (`shadow_catalog/`)

Background system that proactively builds a torrent catalog mirroring your music library. It lurks in the background, pre-downloading discographies so requests can be served instantly from cache.

```rust
struct ShadowCatalog {
    catalog: Arc<dyn TorrentCatalog>,
    searcher: Arc<dyn TorrentSearcher>,
    matcher: Arc<dyn Matcher>,
    torrent_client: Arc<dyn TorrentClient>,
    config: ShadowCatalogConfig,
}

struct ShadowCatalogConfig {
    enabled: bool,
    scan_interval_hours: u32,
    max_concurrent_downloads: usize,
    storage_budget_bytes: u64,
    min_artist_popularity: u32,
}
```

The Shadow Catalog is implemented early (Phase 2) to prove torrent search and seeding work before building the full processing pipeline.

### 4. Queue Manager (`queue/`)

- SQLite-backed ticket persistence
- State machine enforcement
- Retry logic with exponential backoff
- Priority queue ordering (higher `priority` value = processed first)

### 5. Processing Pools (`processor/`)

Tickets flow through a series of processing pools. Each pool handles one stage of the pipeline and enforces concurrency limits.

```rust
struct ProcessorPools {
    acquisition: Pool<AcquisitionJob>,  // TextBrain: query + search + score loop
    downloader: Pool<DownloadJob>,      // max_parallel_downloads workers
    converter: Pool<ConvertJob>,        // max_parallel_conversions workers
    placer: Pool<PlaceJob>,             // Generous, I/O bound
}
```

The **Acquisition pool** combines what was previously Search + Match into a single coordinated operation. TextBrain owns the full query building → search → scoring loop, iterating until a match is found or all strategies are exhausted.

Each pool is a priority queue - tickets with higher priority are processed first within each stage.

```
┌────────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ Acquisition│───▶│Downloader│───▶│Converter │───▶│  Placer  │
│    Pool    │    │   Pool   │    │   Pool   │    │   Pool   │
└────────────┘    └──────────┘    └──────────┘    └──────────┘
    PENDING        APPROVED/       DOWNLOADING      CONVERTING
       ▼          AUTO_APPROVED     COMPLETE           ▼
   ACQUIRING          ▼               ▼            PLACING
       ▼          DOWNLOADING     CONVERTING          ▼
 NEEDS_APPROVAL/                                   COMPLETED
 AUTO_APPROVED
```

### 6. Searcher (`searcher/`)

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

### 7. Matcher (`matcher/`)

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

### 8. Torrent Client (`torrent_client/`)

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

### 9. Converter (`converter/`)

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

### 10. Placer (`placer/`)

File operations:
- Move converted files to dest_path
- Create directories as needed
- Verify file integrity after move
- Cleanup temp files
- Rollback on partial failure

## Admin Dashboard

The admin dashboard is built with **Vue 3 + TypeScript + Vite** and evolves alongside the backend. Every phase delivers corresponding dashboard functionality.

### Tech Stack

| Component | Choice |
|-----------|--------|
| Framework | Vue 3 (Composition API) |
| Language | TypeScript |
| Build | Vite |
| State | Pinia |
| Styling | UnoCSS (Tailwind-compatible, Vite-native) |
| WebSocket | Native WebSocket + VueUse composables |

### Type Synchronization

TypeScript types are generated from Rust structs using `ts-rs` or `specta`:

```rust
// In Rust
#[derive(Serialize, TS)]
#[ts(export)]
struct Ticket {
    ticket_id: String,
    priority: u16,
    // ...
}
```

```typescript
// Generated in TypeScript
export interface Ticket {
    ticket_id: string;
    priority: number;
    // ...
}
```

### Key Composables

```typescript
// useWebSocket - Real-time updates with snapshot-on-connect
const { tickets, isConnected, subscribe } = useWebSocket()

// useTickets - Ticket management
const { createTicket, approveTicket, rejectTicket } = useTickets()

// useAuth - Authentication state
const { identity, isAuthenticated, logout } = useAuth()

// useShadowCatalog - Shadow catalog browser
const { torrents, coverage, searchTorrent } = useShadowCatalog()
```

### Dashboard Features by Phase

| Phase | Dashboard Features |
|-------|-------------------|
| 1 | Auth flow, config display, basic layout, ticket management, kanban board, text search |
| 2 | Search testing, torrent status, Shadow Catalog browser |
| 3 | Pipeline visualization, pool status, job progress |
| 4 | Ticket creation, matching preview, conversion status |
| 5 | Approval queue, candidate comparison, LLM reasoning |
| 6 | Real-time updates, audit log viewer, system health |
| 7 | Video-specific views |

## Cover Art

Cover art is fetched from multiple sources with fallback:

1. **MusicBrainz Cover Art Archive** (primary) - CC0, no API key needed
2. **Discogs** (fallback) - Requires API key, rate limited
3. **Torrent files** (last resort) - Extract from embedded tags or folder.jpg

## Configuration

See `config.example.toml` for a complete reference with all options documented. Below is a minimal example:

```toml
# ==============================================================================
# AUTHENTICATION (REQUIRED)
# ==============================================================================

[auth]
method = "none"  # "none" or "api_key"
# api_key = "your-secret-key"  # Required when method = "api_key"

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
path = "quentin.db"

# ==============================================================================
# TORRENT SEARCH (required for ticket processing)
# ==============================================================================

[searcher]
backend = "jackett"

[searcher.jackett]
url = "http://localhost:9117"
api_key = "your-jackett-api-key"
timeout_secs = 30

# ==============================================================================
# TORRENT CLIENT (required for downloading)
# ==============================================================================

# Option 1: Embedded librqbit (no external service needed)
[torrent_client]
backend = "librqbit"

[torrent_client.librqbit]
download_path = "/downloads"
enable_dht = true

# Option 2: External qBittorrent
# [torrent_client]
# backend = "qbittorrent"
#
# [torrent_client.qbittorrent]
# url = "http://localhost:8080"
# username = "admin"
# password = "adminadmin"

# ==============================================================================
# TEXTBRAIN (intelligent matching)
# ==============================================================================

[textbrain]
mode = "dumb_only"  # "dumb_only", "dumb_first", "llm_first", "llm_only"
auto_approve_threshold = 0.85

# Optional LLM configuration (for dumb_first, llm_first, llm_only modes)
# [textbrain.llm]
# provider = "ollama"  # "anthropic", "openai", "ollama"
# model = "llama2"
# timeout_secs = 60

# ==============================================================================
# ORCHESTRATOR (automatic ticket processing)
# ==============================================================================

[orchestrator]
enabled = true
acquisition_poll_interval_ms = 5000
download_poll_interval_ms = 3000
auto_approve_threshold = 0.85
max_concurrent_downloads = 3

# ==============================================================================
# EXTERNAL CATALOGS (for ticket wizard)
# ==============================================================================

# MusicBrainz - No API key required
[external_catalogs.musicbrainz]
user_agent = "QuentinTorrentino/0.1.0 ( https://github.com/your-username )"
rate_limit_ms = 1100

# TMDB - Requires free API key from themoviedb.org
# [external_catalogs.tmdb]
# api_key = "your-tmdb-api-key"
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
│   │       ├── shadow_catalog/
│   │       │   ├── mod.rs
│   │       │   └── priority.rs
│   │       ├── processor/
│   │       │   ├── mod.rs
│   │       │   └── pools.rs
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
│   │       ├── testing/              # Mock implementations for E2E tests
│   │       │   ├── mod.rs
│   │       │   ├── mock_torrent_client.rs
│   │       │   ├── mock_searcher.rs
│   │       │   ├── mock_external_catalog.rs
│   │       │   ├── mock_converter.rs
│   │       │   └── mock_placer.rs
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
│   ├── server/                   # torrentino-server (HTTP service)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── api/
│   │       │   ├── mod.rs
│   │       │   ├── routes.rs
│   │       │   ├── handlers.rs
│   │       │   ├── websocket.rs
│   │       │   └── auth_middleware.rs
│   │       └── bin/
│   │           └── quentin.rs
│   │
│   └── dashboard/                # Admin dashboard (Vue 3 + TypeScript + Vite)
│       ├── package.json
│       ├── vite.config.ts
│       ├── tsconfig.json
│       ├── index.html
│       ├── src/
│       │   ├── main.ts
│       │   ├── App.vue
│       │   ├── api/              # API client (generated from OpenAPI or ts-rs)
│       │   ├── composables/      # useWebSocket, useTickets, useAuth, etc.
│       │   ├── components/
│       │   ├── views/
│       │   └── stores/           # Pinia stores
│       └── public/
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

The `torrentino_core::testing` module provides mock implementations of all external service traits:

| Mock | Trait | Features |
|------|-------|----------|
| `MockTorrentClient` | `TorrentClient` | Track added torrents, control progress, simulate failures |
| `MockSearcher` | `Searcher` | Configurable results, query recording, custom filters |
| `MockExternalCatalog` | `ExternalCatalog` | MusicBrainz/TMDB mock responses |
| `MockConverter` | `Converter` | Track conversions, custom probe results, progress |
| `MockPlacer` | `Placer` | Track placements/rollbacks, progress simulation |

Test fixtures are available via `testing::fixtures`:
- `torrent_candidate()`, `audio_candidate()`, `video_candidate()`
- `musicbrainz_release()`, `tmdb_movie()`, `tmdb_series()`, `tmdb_season()`

Example usage:
```rust
use torrentino_core::testing::{MockTorrentClient, MockSearcher, fixtures};

let searcher = MockSearcher::new();
searcher.set_results(vec![
    fixtures::audio_candidate("The Beatles", "Abbey Road", "abc123"),
]).await;

// Inject into AppState for E2E tests...
```

### End-to-End Tests
- Docker compose with real Jackett + qBittorrent
- Test with legal/free torrents (e.g., creative commons music)

## Remaining Work

Phases 1-4 and most of Phase 5 are complete. Below is the remaining work.

### Phase 6: Production Ready
- [ ] Retry logic with exponential backoff (integrate with orchestrator)
- [ ] Metrics/observability (Prometheus metrics for all workers)
- [ ] Docker packaging
- [ ] E2E test suite for server (using mock infrastructure)
- [ ] E2E test suite for dashboard

## Design Decisions

1. **Auth required**: Service exits on startup if no auth configured. Must explicitly opt-in to `method = "none"` for anonymous access.

2. **Seeding policy**: Ethical approach - seed until ratio >= 1.0 before cleanup eligible.

3. **Rate limiting**: Per-indexer limits for private trackers.

4. **LLM optional**: Dumb matcher works for most cases. LLM enhances accuracy when configured.

5. **Audit everything**: All significant events logged. Consumer handles retention and training data extraction.

6. **Fail-safe placement**: If any file fails to place, rollback all placed files for that ticket.

7. **Priority queues**: All processing pools use priority queues. Higher `priority` value = processed first.

8. **Dashboard-first development**: Admin dashboard evolves with each phase. Every feature is immediately testable via UI.

9. **WebSocket consistency**: Snapshot-on-connect with sequence numbers ensures dashboard state is always consistent.

10. **Shadow Catalog deferred**: Proactive torrent caching is complex and requires integration with consumer catalogs. Deferred to a later phase.

## Future Enhancements (Not In Scope Yet)

- Shadow Catalog - proactive search & seeding to pre-cache content
- Notification system (webhooks, email, Telegram with interactive approvals)
- Bulk admin operations
- Fine-tuned local LLM for matching
- Per-user rate limits / quotas
