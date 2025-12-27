# Phase 2c: Torrent Catalog (Search Result Cache)

## Overview

The **Torrent Catalog** is a local cache of torrent metadata from previous searches. When a search is performed via Jackett, the results are stored in the catalog. Future searches can check the catalog first, avoiding redundant external API calls.

This is **NOT** a registry of downloaded files - it's a cache of **known torrents** (metadata only).

## Purpose

1. **Avoid redundant searches**: If we've seen a torrent before, we have it cached
2. **Faster lookups**: Local SQLite query vs external Jackett API call
3. **Offline capability**: Can search cached torrents even if Jackett is down
4. **Matcher optimization** (Phase 4): Matcher can check cache first before hitting external indexers

## How It Works

```
User searches "Radiohead OK Computer"
            │
            ▼
    ┌───────────────────┐
    │  Check Catalog    │  ◄── Do we have matching torrents from previous searches?
    │  (local SQLite)   │
    └───────────────────┘
            │
            ▼
    ┌───────────────────┐
    │  Search Jackett   │  ◄── Get fresh results from external indexers
    │  (external)       │
    └───────────────────┘
            │
            ▼
    ┌───────────────────┐
    │  Store in Catalog │  ◄── Save these results for future searches
    └───────────────────┘
            │
            ▼
      Show combined results
      (deduplicated by info_hash)
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage | SQLite (same DB) | Consistent with tickets/audit, fast local queries |
| What's stored | TorrentCandidate data | Same structure as search results |
| Deduplication | By info_hash | Same torrent = same hash, merge sources |
| Cache invalidation | None (for now) | Old data still valid, seeders may be stale but torrent still exists |
| File list storage | Store if available | Enables file-level search in cache |
| Search matching | LIKE on title + file paths | Simple, good enough for now |

## Scope

### In Scope
- SQLite schema for cached torrents + files
- Store search results in catalog after Jackett search
- Search catalog by query (title/file matching)
- Deduplicate results (catalog + external) by info_hash
- Dashboard toggle: Cache only / External only / Both
- `GET /api/v1/catalog` - list/search cached torrents
- `GET /api/v1/catalog/stats` - cache statistics
- Persist search results automatically on external search

### Out of Scope
- Content parsing (artist/album/track extraction) - Phase 4
- Coverage queries ("do we have this artist?") - Phase 4
- Cache expiration/cleanup - Future
- Downloaded files registry - separate concern (query torrent client)

## SQLite Schema

```sql
-- Cached torrent metadata (one row per unique info_hash)
CREATE TABLE IF NOT EXISTS torrent_cache (
    -- Info hash (lowercase hex) - primary key
    info_hash TEXT PRIMARY KEY,
    -- Torrent title (from first source that provided it)
    title TEXT NOT NULL,
    -- Total size in bytes
    size_bytes INTEGER NOT NULL,
    -- Category (e.g., "Audio", "Music/Lossless")
    category TEXT,
    -- When first cached
    first_seen_at TEXT NOT NULL,
    -- When last seen in a search result
    last_seen_at TEXT NOT NULL,
    -- Number of times this torrent appeared in search results
    seen_count INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_torrent_cache_title ON torrent_cache(title);
CREATE INDEX IF NOT EXISTS idx_torrent_cache_last_seen ON torrent_cache(last_seen_at);

-- Sources for each cached torrent (one torrent can have multiple sources/indexers)
CREATE TABLE IF NOT EXISTS torrent_cache_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    info_hash TEXT NOT NULL REFERENCES torrent_cache(info_hash) ON DELETE CASCADE,
    -- Indexer name
    indexer TEXT NOT NULL,
    -- Magnet URI (if available)
    magnet_uri TEXT,
    -- .torrent download URL (if available)
    torrent_url TEXT,
    -- Seeders (last known)
    seeders INTEGER NOT NULL DEFAULT 0,
    -- Leechers (last known)
    leechers INTEGER NOT NULL DEFAULT 0,
    -- Details page URL
    details_url TEXT,
    -- When this source was last updated
    updated_at TEXT NOT NULL,
    -- Unique: one entry per indexer per torrent
    UNIQUE(info_hash, indexer)
);

CREATE INDEX IF NOT EXISTS idx_torrent_cache_sources_hash ON torrent_cache_sources(info_hash);

-- Files within cached torrents (if file list was available)
CREATE TABLE IF NOT EXISTS torrent_cache_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    info_hash TEXT NOT NULL REFERENCES torrent_cache(info_hash) ON DELETE CASCADE,
    -- File path within torrent
    path TEXT NOT NULL,
    -- File size in bytes
    size_bytes INTEGER NOT NULL,
    UNIQUE(info_hash, path)
);

CREATE INDEX IF NOT EXISTS idx_torrent_cache_files_hash ON torrent_cache_files(info_hash);
CREATE INDEX IF NOT EXISTS idx_torrent_cache_files_path ON torrent_cache_files(path);
```

## Rust Types

```rust
// crates/core/src/catalog/mod.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A cached torrent entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTorrent {
    pub info_hash: String,
    pub title: String,
    pub size_bytes: u64,
    pub category: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub seen_count: u32,
    /// All known sources for this torrent
    pub sources: Vec<CachedTorrentSource>,
    /// File list (if available)
    pub files: Option<Vec<CachedTorrentFile>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTorrentSource {
    pub indexer: String,
    pub magnet_uri: Option<String>,
    pub torrent_url: Option<String>,
    pub seeders: u32,
    pub leechers: u32,
    pub details_url: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTorrentFile {
    pub path: String,
    pub size_bytes: u64,
}

/// Search mode for combined catalog + external search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// Search only the local cache
    CacheOnly,
    /// Search only external indexers (Jackett)
    ExternalOnly,
    /// Search both, combine results
    #[default]
    Both,
}

/// Query for searching the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSearchQuery {
    /// Search text (matched against title and file paths)
    pub query: String,
    /// Maximum results
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 { 100 }

/// Catalog statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogStats {
    /// Total cached torrents
    pub total_torrents: u64,
    /// Total cached files
    pub total_files: u64,
    /// Total size of all cached torrents
    pub total_size_bytes: u64,
    /// Number of unique indexers
    pub unique_indexers: u32,
    /// Oldest entry
    pub oldest_entry: Option<DateTime<Utc>>,
    /// Most recent entry
    pub newest_entry: Option<DateTime<Utc>>,
}

/// Errors for catalog operations.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),
}
```

## Trait Definition

```rust
// crates/core/src/catalog/mod.rs

use async_trait::async_trait;

#[async_trait]
pub trait TorrentCatalog: Send + Sync {
    /// Store search results in the catalog.
    /// Deduplicates by info_hash, merges sources.
    async fn store(&self, candidates: &[TorrentCandidate]) -> Result<u32, CatalogError>;

    /// Search the catalog.
    async fn search(&self, query: &CatalogSearchQuery) -> Result<Vec<CachedTorrent>, CatalogError>;

    /// Get a specific torrent by info_hash.
    async fn get(&self, info_hash: &str) -> Result<CachedTorrent, CatalogError>;

    /// Get catalog statistics.
    async fn stats(&self) -> Result<CatalogStats, CatalogError>;

    /// Check if a torrent exists in the catalog.
    async fn exists(&self, info_hash: &str) -> Result<bool, CatalogError>;

    /// Remove a torrent from the catalog.
    async fn remove(&self, info_hash: &str) -> Result<(), CatalogError>;

    /// Clear all cached data.
    async fn clear(&self) -> Result<(), CatalogError>;
}
```

## Integration with Search Flow

The existing search endpoint needs to be modified:

```rust
// Modified search flow

pub async fn search(
    query: &SearchQuery,
    mode: SearchMode,
    catalog: &dyn TorrentCatalog,
    searcher: &dyn Searcher,
) -> Result<CombinedSearchResult, Error> {
    let mut cached_results = vec![];
    let mut external_results = vec![];

    // 1. Search catalog (if mode allows)
    if mode != SearchMode::ExternalOnly {
        cached_results = catalog.search(&CatalogSearchQuery {
            query: query.query.clone(),
            limit: query.limit.unwrap_or(100),
        }).await?;
    }

    // 2. Search external (if mode allows)
    if mode != SearchMode::CacheOnly {
        let result = searcher.search(query).await?;
        external_results = result.candidates;

        // 3. Store external results in catalog for future
        catalog.store(&external_results).await?;
    }

    // 4. Combine and deduplicate by info_hash
    let combined = merge_results(cached_results, external_results);

    Ok(CombinedSearchResult {
        candidates: combined,
        cached_count: cached_results.len(),
        external_count: external_results.len(),
    })
}
```

## API Changes

### Modified Search Endpoint

```
POST /api/v1/search
Content-Type: application/json

{
  "query": "Radiohead OK Computer FLAC",
  "indexers": ["rutracker"],           // optional
  "categories": ["music"],              // optional
  "limit": 50,                          // optional
  "mode": "both"                        // NEW: "cache_only" | "external_only" | "both"
}

Response 200:
{
  "query": { ... },
  "candidates": [
    {
      "title": "Radiohead - OK Computer (1997) [FLAC]",
      "info_hash": "abc123...",
      "size_bytes": 524288000,
      "seeders": 45,
      "leechers": 3,
      "sources": [ ... ],
      "from_cache": true                // NEW: was this result from cache?
    },
    {
      "title": "Radiohead - OK Computer FLAC 24bit",
      "info_hash": "def456...",
      ...
      "from_cache": false               // This one came from external search
    }
  ],
  "duration_ms": 1234,
  "indexer_errors": {},
  "cache_hits": 15,                     // NEW: how many came from cache
  "external_hits": 32                   // NEW: how many came from external
}
```

### New Catalog Endpoints

```
GET /api/v1/catalog?query=radiohead&limit=50
→ Search the catalog directly

GET /api/v1/catalog/stats
→ Get cache statistics

GET /api/v1/catalog/{info_hash}
→ Get specific cached torrent

DELETE /api/v1/catalog/{info_hash}
→ Remove from cache

DELETE /api/v1/catalog
→ Clear entire cache
```

## Dashboard Changes

### Search Page Updates

Add a toggle/dropdown for search mode and show source per result:

```
┌──────────────────────────────────────────────────────────────────┐
│  Search                                                          │
│  ┌───────────────────────────────────┐                           │
│  │ Radiohead OK Computer             │  [Search]                 │
│  └───────────────────────────────────┘                           │
│                                                                  │
│  Mode: [Both ▼]     ← NEW dropdown                               │
│         • Both (cache + external)                                │
│         • Cache only                                             │
│         • External only                                          │
│                                                                  │
│  Results: 47 total (15 from cache, 32 from Jackett)              │
│  ────────────────────────────────────────────────────────────    │
│  │ Src │ Title                          │ Size   │ Seeders │ ... │
│  ├─────┼────────────────────────────────┼────────┼─────────┼─────│
│  │ 💾  │ Radiohead - OK Computer [FLAC] │ 450 MB │ 45      │     │  ← cached
│  │ 💾  │ Radiohead - OK Computer 24bit  │ 1.2 GB │ 12      │     │  ← cached
│  │ 🌐  │ Radiohead OK Computer V0       │ 120 MB │ 8       │     │  ← external
│  │ 🌐  │ Radiohead - OKC Deluxe         │ 890 MB │ 23      │     │  ← external
└──────────────────────────────────────────────────────────────────┘

Legend:
  💾 = From cache (previous search)
  🌐 = From external (Jackett)
```

Alternative: Use a small badge/chip instead of emoji:
- `[CACHED]` in muted color
- `[LIVE]` or no badge for external results

### New Catalog Stats (Optional)

Small stats display somewhere (sidebar or settings):

```
Cache: 1,234 torrents | 45,678 files | 2.3 TB total
```

## Implementation Tasks

### Task 1: Core Types & Trait
**Files**: `crates/core/src/catalog/mod.rs`, `crates/core/src/catalog/types.rs`

- [ ] Create catalog module
- [ ] Define `CachedTorrent`, `CachedTorrentSource`, `CachedTorrentFile`
- [ ] Define `SearchMode`, `CatalogSearchQuery`, `CatalogStats`
- [ ] Define `CatalogError`
- [ ] Define `TorrentCatalog` trait
- [ ] Export from lib.rs

### Task 2: SQLite Implementation
**Files**: `crates/core/src/catalog/sqlite.rs`

- [ ] Create schema (torrent_cache, torrent_cache_sources, torrent_cache_files)
- [ ] Implement `SqliteCatalog`
- [ ] Implement `store()` - upsert logic with source merging
- [ ] Implement `search()` - LIKE matching on title + files
- [ ] Implement `get()`, `exists()`, `remove()`, `clear()`
- [ ] Implement `stats()`
- [ ] Write unit tests

### Task 3: Integrate with Search Flow
**Files**: `crates/core/src/searcher/mod.rs`, `crates/server/src/api/searcher.rs`

- [ ] Add `SearchMode` to `SearchQuery`
- [ ] Modify search handler to check catalog first
- [ ] Store external results in catalog after search
- [ ] Return cache/external hit counts in response
- [ ] Update response types

### Task 4: Catalog API Endpoints
**Files**: `crates/server/src/api/catalog.rs`

- [ ] `GET /api/v1/catalog` - search catalog
- [ ] `GET /api/v1/catalog/stats` - get stats
- [ ] `GET /api/v1/catalog/{hash}` - get single entry
- [ ] `DELETE /api/v1/catalog/{hash}` - remove entry
- [ ] `DELETE /api/v1/catalog` - clear cache
- [ ] Register routes

### Task 5: Dashboard - Search Mode Toggle & Source Indicator
**Files**: `crates/dashboard/src/components/search/*`, `crates/dashboard/src/api/types.ts`

- [ ] Add `SearchMode` type
- [ ] Add `from_cache` field to result type
- [ ] Add mode dropdown to SearchForm
- [ ] Update search API call to include mode
- [ ] Display cache/external hit counts in results header
- [ ] Show source icon/badge per result row (💾 cached, 🌐 external)

### Task 6: Dashboard - Catalog Stats (Optional)
**Files**: `crates/dashboard/src/components/catalog/*`

- [ ] Add catalog stats API client
- [ ] Display stats in UI (sidebar or settings page)

### Task 7: Testing
- [ ] Unit tests for catalog store/search
- [ ] Integration test: search populates cache
- [ ] Integration test: cache-only search returns cached results
- [ ] Dashboard manual testing

## Success Criteria

- [ ] External search results are stored in catalog
- [ ] Subsequent search for same query hits cache
- [ ] Search mode toggle works (cache only / external only / both)
- [ ] Results show cache vs external hit counts
- [ ] `cargo test` passes
- [ ] `npm run build` succeeds
- [ ] Dashboard search works with mode toggle

## Open Questions (Resolved)

| Question | Decision |
|----------|----------|
| What is the catalog? | Cache of torrent metadata from searches, NOT downloaded files |
| When is it populated? | Automatically when external search is performed |
| How is it searched? | LIKE matching on title + file paths |
| Dashboard integration? | Mode toggle on search page |
| Downloaded files tracking? | Separate concern - query torrent client directly |

## Future Enhancements

- **Cache expiration**: Remove old entries after N days
- **Content parsing**: Extract artist/album/track from titles (Phase 4)
- **Coverage queries**: "Do we have this artist cached?" (Phase 4)
- **Seeder refresh**: Periodically update seeder counts for cached torrents
