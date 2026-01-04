//! SQLite-backed torrent catalog implementation.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::{
    CachedTorrent, CachedTorrentFile, CachedTorrentSource, CatalogError, CatalogSearchQuery,
    CatalogStats, TorrentCatalog,
};
use crate::searcher::{TorrentCandidate, TorrentFile};

/// SQLite-backed torrent catalog.
pub struct SqliteCatalog {
    conn: Mutex<Connection>,
}

impl SqliteCatalog {
    /// Create a new SQLite catalog, creating the database file and tables if needed.
    pub fn new(path: &Path) -> Result<Self, CatalogError> {
        let conn = Connection::open(path).map_err(|e| CatalogError::Database(e.to_string()))?;
        Self::initialize_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite catalog (useful for testing).
    pub fn in_memory() -> Result<Self, CatalogError> {
        let conn =
            Connection::open_in_memory().map_err(|e| CatalogError::Database(e.to_string()))?;
        Self::initialize_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn initialize_schema(conn: &Connection) -> Result<(), CatalogError> {
        conn.execute_batch(
            r#"
            -- Cached torrent metadata (one row per unique info_hash)
            CREATE TABLE IF NOT EXISTS torrent_cache (
                info_hash TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                category TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                seen_count INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_torrent_cache_title ON torrent_cache(title);
            CREATE INDEX IF NOT EXISTS idx_torrent_cache_last_seen ON torrent_cache(last_seen_at);

            -- Sources for each cached torrent
            CREATE TABLE IF NOT EXISTS torrent_cache_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                info_hash TEXT NOT NULL REFERENCES torrent_cache(info_hash) ON DELETE CASCADE,
                indexer TEXT NOT NULL,
                magnet_uri TEXT,
                torrent_url TEXT,
                seeders INTEGER NOT NULL DEFAULT 0,
                leechers INTEGER NOT NULL DEFAULT 0,
                details_url TEXT,
                updated_at TEXT NOT NULL,
                UNIQUE(info_hash, indexer)
            );

            CREATE INDEX IF NOT EXISTS idx_torrent_cache_sources_hash ON torrent_cache_sources(info_hash);

            -- Files within cached torrents
            CREATE TABLE IF NOT EXISTS torrent_cache_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                info_hash TEXT NOT NULL REFERENCES torrent_cache(info_hash) ON DELETE CASCADE,
                path TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                UNIQUE(info_hash, path)
            );

            CREATE INDEX IF NOT EXISTS idx_torrent_cache_files_hash ON torrent_cache_files(info_hash);
            CREATE INDEX IF NOT EXISTS idx_torrent_cache_files_path ON torrent_cache_files(path);
            "#,
        )
        .map_err(|e| CatalogError::Database(e.to_string()))?;

        Ok(())
    }

    /// Load sources for a torrent.
    fn load_sources(
        conn: &Connection,
        info_hash: &str,
    ) -> Result<Vec<CachedTorrentSource>, CatalogError> {
        let mut stmt = conn
            .prepare(
                "SELECT indexer, magnet_uri, torrent_url, seeders, leechers, details_url, updated_at
                 FROM torrent_cache_sources WHERE info_hash = ?",
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![info_hash], |row| {
                let updated_at_str: String = row.get(6)?;
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(CachedTorrentSource {
                    indexer: row.get(0)?,
                    magnet_uri: row.get(1)?,
                    torrent_url: row.get(2)?,
                    seeders: row.get(3)?,
                    leechers: row.get(4)?,
                    details_url: row.get(5)?,
                    updated_at,
                })
            })
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let mut sources = Vec::new();
        for row in rows {
            sources.push(row.map_err(|e| CatalogError::Database(e.to_string()))?);
        }
        Ok(sources)
    }

    /// Load files for a torrent.
    fn load_files(
        conn: &Connection,
        info_hash: &str,
    ) -> Result<Vec<CachedTorrentFile>, CatalogError> {
        let mut stmt = conn
            .prepare("SELECT path, size_bytes FROM torrent_cache_files WHERE info_hash = ?")
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![info_hash], |row| {
                Ok(CachedTorrentFile {
                    path: row.get(0)?,
                    size_bytes: row.get(1)?,
                })
            })
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row.map_err(|e| CatalogError::Database(e.to_string()))?);
        }
        Ok(files)
    }

    /// Convert a row to CachedTorrent (without sources/files).
    fn row_to_cached_torrent(row: &rusqlite::Row) -> rusqlite::Result<CachedTorrent> {
        let first_seen_str: String = row.get(4)?;
        let last_seen_str: String = row.get(5)?;

        let first_seen_at = DateTime::parse_from_rfc3339(&first_seen_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let last_seen_at = DateTime::parse_from_rfc3339(&last_seen_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(CachedTorrent {
            info_hash: row.get(0)?,
            title: row.get(1)?,
            size_bytes: row.get(2)?,
            category: row.get(3)?,
            first_seen_at,
            last_seen_at,
            seen_count: row.get(6)?,
            sources: Vec::new(), // Will be loaded separately
            files: None,         // Will be loaded separately
        })
    }
}

impl TorrentCatalog for SqliteCatalog {
    fn store(&self, candidates: &[TorrentCandidate]) -> Result<u32, CatalogError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let mut new_count = 0;

        for candidate in candidates {
            // Skip candidates without info_hash
            if candidate.info_hash.is_empty() {
                continue;
            }

            let info_hash = candidate.info_hash.to_lowercase();

            // Check if torrent exists
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM torrent_cache WHERE info_hash = ?",
                    params![&info_hash],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if exists {
                // Update existing entry
                conn.execute(
                    "UPDATE torrent_cache SET last_seen_at = ?, seen_count = seen_count + 1 WHERE info_hash = ?",
                    params![&now_str, &info_hash],
                )
                .map_err(|e| CatalogError::Database(e.to_string()))?;
            } else {
                // Insert new entry
                conn.execute(
                    "INSERT INTO torrent_cache (info_hash, title, size_bytes, category, first_seen_at, last_seen_at, seen_count)
                     VALUES (?, ?, ?, ?, ?, ?, 1)",
                    params![
                        &info_hash,
                        &candidate.title,
                        candidate.size_bytes as i64,
                        &candidate.category,
                        &now_str,
                        &now_str,
                    ],
                )
                .map_err(|e| CatalogError::Database(e.to_string()))?;
                new_count += 1;
            }

            // Upsert sources
            for source in &candidate.sources {
                conn.execute(
                    "INSERT INTO torrent_cache_sources (info_hash, indexer, magnet_uri, torrent_url, seeders, leechers, details_url, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(info_hash, indexer) DO UPDATE SET
                        magnet_uri = COALESCE(excluded.magnet_uri, magnet_uri),
                        torrent_url = COALESCE(excluded.torrent_url, torrent_url),
                        seeders = excluded.seeders,
                        leechers = excluded.leechers,
                        details_url = COALESCE(excluded.details_url, details_url),
                        updated_at = excluded.updated_at",
                    params![
                        &info_hash,
                        &source.indexer,
                        &source.magnet_uri,
                        &source.torrent_url,
                        source.seeders as i32,
                        source.leechers as i32,
                        &source.details_url,
                        &now_str,
                    ],
                )
                .map_err(|e| CatalogError::Database(e.to_string()))?;
            }

            // Insert files (if available and not already present)
            if let Some(ref files) = candidate.files {
                for file in files {
                    conn.execute(
                        "INSERT OR IGNORE INTO torrent_cache_files (info_hash, path, size_bytes)
                         VALUES (?, ?, ?)",
                        params![&info_hash, &file.path, file.size_bytes as i64],
                    )
                    .map_err(|e| CatalogError::Database(e.to_string()))?;
                }
            }
        }

        Ok(new_count)
    }

    fn search(&self, query: &CatalogSearchQuery) -> Result<Vec<CachedTorrent>, CatalogError> {
        let conn = self.conn.lock().unwrap();
        let search_pattern = format!("%{}%", query.query);

        // Search by title OR by file path
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT tc.info_hash, tc.title, tc.size_bytes, tc.category,
                        tc.first_seen_at, tc.last_seen_at, tc.seen_count
                 FROM torrent_cache tc
                 LEFT JOIN torrent_cache_files tcf ON tc.info_hash = tcf.info_hash
                 WHERE tc.title LIKE ?1 OR tcf.path LIKE ?1
                 ORDER BY tc.last_seen_at DESC
                 LIMIT ?2",
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![&search_pattern, query.limit as i32],
                Self::row_to_cached_torrent,
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            let mut torrent = row.map_err(|e| CatalogError::Database(e.to_string()))?;
            // Load sources and files
            torrent.sources = Self::load_sources(&conn, &torrent.info_hash)?;
            let files = Self::load_files(&conn, &torrent.info_hash)?;
            torrent.files = if files.is_empty() { None } else { Some(files) };
            results.push(torrent);
        }

        Ok(results)
    }

    fn get(&self, info_hash: &str) -> Result<CachedTorrent, CatalogError> {
        let conn = self.conn.lock().unwrap();
        let info_hash = info_hash.to_lowercase();

        let mut torrent = conn
            .query_row(
                "SELECT info_hash, title, size_bytes, category, first_seen_at, last_seen_at, seen_count
                 FROM torrent_cache WHERE info_hash = ?",
                params![&info_hash],
                Self::row_to_cached_torrent,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CatalogError::NotFound(info_hash.clone()),
                _ => CatalogError::Database(e.to_string()),
            })?;

        // Load sources and files
        torrent.sources = Self::load_sources(&conn, &info_hash)?;
        let files = Self::load_files(&conn, &info_hash)?;
        torrent.files = if files.is_empty() { None } else { Some(files) };

        Ok(torrent)
    }

    fn store_files(
        &self,
        info_hash: &str,
        title: &str,
        files: &[TorrentFile],
    ) -> Result<(), CatalogError> {
        let conn = self.conn.lock().unwrap();
        let info_hash = info_hash.to_lowercase();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Ensure the torrent exists in the catalog (create minimal entry if not)
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM torrent_cache WHERE info_hash = ?",
                params![&info_hash],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !exists {
            // Calculate total size from files
            let total_size: u64 = files.iter().map(|f| f.size_bytes).sum();

            conn.execute(
                "INSERT INTO torrent_cache (info_hash, title, size_bytes, category, first_seen_at, last_seen_at, seen_count)
                 VALUES (?, ?, ?, NULL, ?, ?, 1)",
                params![
                    &info_hash,
                    title,
                    total_size as i64,
                    &now_str,
                    &now_str,
                ],
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;
        }

        // Delete existing files for this torrent (we're replacing them)
        conn.execute(
            "DELETE FROM torrent_cache_files WHERE info_hash = ?",
            params![&info_hash],
        )
        .map_err(|e| CatalogError::Database(e.to_string()))?;

        // Insert new files
        for file in files {
            conn.execute(
                "INSERT INTO torrent_cache_files (info_hash, path, size_bytes) VALUES (?, ?, ?)",
                params![&info_hash, &file.path, file.size_bytes as i64],
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;
        }

        Ok(())
    }

    fn get_files(&self, info_hash: &str) -> Result<Option<Vec<CachedTorrentFile>>, CatalogError> {
        let conn = self.conn.lock().unwrap();
        let info_hash = info_hash.to_lowercase();

        let files = Self::load_files(&conn, &info_hash)?;

        if files.is_empty() {
            Ok(None)
        } else {
            Ok(Some(files))
        }
    }

    fn stats(&self) -> Result<CatalogStats, CatalogError> {
        let conn = self.conn.lock().unwrap();

        let total_torrents: u64 = conn
            .query_row("SELECT COUNT(*) FROM torrent_cache", [], |row| row.get(0))
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let total_files: u64 = conn
            .query_row("SELECT COUNT(*) FROM torrent_cache_files", [], |row| {
                row.get(0)
            })
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let total_size_bytes: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM torrent_cache",
                [],
                |row| row.get(0),
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let unique_indexers: u32 = conn
            .query_row(
                "SELECT COUNT(DISTINCT indexer) FROM torrent_cache_sources",
                [],
                |row| row.get(0),
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        let oldest_entry: Option<DateTime<Utc>> = conn
            .query_row("SELECT MIN(first_seen_at) FROM torrent_cache", [], |row| {
                let s: Option<String> = row.get(0)?;
                Ok(s)
            })
            .map_err(|e| CatalogError::Database(e.to_string()))?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let newest_entry: Option<DateTime<Utc>> = conn
            .query_row("SELECT MAX(last_seen_at) FROM torrent_cache", [], |row| {
                let s: Option<String> = row.get(0)?;
                Ok(s)
            })
            .map_err(|e| CatalogError::Database(e.to_string()))?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(CatalogStats {
            total_torrents,
            total_files,
            total_size_bytes: total_size_bytes as u64,
            unique_indexers,
            oldest_entry,
            newest_entry,
        })
    }

    fn exists(&self, info_hash: &str) -> Result<bool, CatalogError> {
        let conn = self.conn.lock().unwrap();
        let info_hash = info_hash.to_lowercase();

        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM torrent_cache WHERE info_hash = ?",
                params![&info_hash],
                |_| Ok(true),
            )
            .unwrap_or(false);

        Ok(exists)
    }

    fn remove(&self, info_hash: &str) -> Result<(), CatalogError> {
        let conn = self.conn.lock().unwrap();
        let info_hash = info_hash.to_lowercase();

        // Delete from main table (cascades to sources and files)
        let rows_affected = conn
            .execute(
                "DELETE FROM torrent_cache WHERE info_hash = ?",
                params![&info_hash],
            )
            .map_err(|e| CatalogError::Database(e.to_string()))?;

        if rows_affected == 0 {
            return Err(CatalogError::NotFound(info_hash));
        }

        Ok(())
    }

    fn clear(&self) -> Result<(), CatalogError> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "DELETE FROM torrent_cache_files;
             DELETE FROM torrent_cache_sources;
             DELETE FROM torrent_cache;",
        )
        .map_err(|e| CatalogError::Database(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::{TorrentFile, TorrentSource};

    fn create_test_catalog() -> SqliteCatalog {
        SqliteCatalog::in_memory().unwrap()
    }

    fn create_test_candidate(info_hash: &str, title: &str) -> TorrentCandidate {
        TorrentCandidate {
            info_hash: info_hash.to_string(),
            title: title.to_string(),
            size_bytes: 1024 * 1024 * 100, // 100 MB
            seeders: 10,
            leechers: 2,
            category: Some("Music".to_string()),
            publish_date: None,
            files: Some(vec![
                TorrentFile {
                    path: "Album/01 - Track One.flac".to_string(),
                    size_bytes: 50 * 1024 * 1024,
                },
                TorrentFile {
                    path: "Album/02 - Track Two.flac".to_string(),
                    size_bytes: 50 * 1024 * 1024,
                },
            ]),
            sources: vec![TorrentSource {
                indexer: "rutracker".to_string(),
                magnet_uri: Some(format!("magnet:?xt=urn:btih:{}", info_hash)),
                torrent_url: None,
                seeders: 10,
                leechers: 2,
                details_url: Some("https://rutracker.org/123".to_string()),
            }],
            from_cache: false,
        }
    }

    #[test]
    fn test_store_new_torrent() {
        let catalog = create_test_catalog();
        let candidate = create_test_candidate("abc123", "Test Album");

        let new_count = catalog.store(&[candidate]).unwrap();
        assert_eq!(new_count, 1);

        // Verify it was stored
        assert!(catalog.exists("abc123").unwrap());
    }

    #[test]
    fn test_store_duplicate_updates() {
        let catalog = create_test_catalog();
        let candidate = create_test_candidate("abc123", "Test Album");

        // Store once
        let new_count = catalog.store(std::slice::from_ref(&candidate)).unwrap();
        assert_eq!(new_count, 1);

        // Store again - should update, not add
        let new_count = catalog.store(std::slice::from_ref(&candidate)).unwrap();
        assert_eq!(new_count, 0);

        // Check seen_count increased
        let torrent = catalog.get("abc123").unwrap();
        assert_eq!(torrent.seen_count, 2);
    }

    #[test]
    fn test_store_skips_empty_info_hash() {
        let catalog = create_test_catalog();
        let mut candidate = create_test_candidate("", "No Hash");
        candidate.info_hash = String::new();

        let new_count = catalog.store(&[candidate]).unwrap();
        assert_eq!(new_count, 0);
    }

    #[test]
    fn test_get_torrent() {
        let catalog = create_test_catalog();
        let candidate = create_test_candidate("abc123", "Test Album");
        catalog.store(&[candidate]).unwrap();

        let torrent = catalog.get("abc123").unwrap();
        assert_eq!(torrent.info_hash, "abc123");
        assert_eq!(torrent.title, "Test Album");
        assert_eq!(torrent.sources.len(), 1);
        assert_eq!(torrent.sources[0].indexer, "rutracker");
        assert!(torrent.files.is_some());
        assert_eq!(torrent.files.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_get_nonexistent() {
        let catalog = create_test_catalog();
        let result = catalog.get("nonexistent");
        assert!(matches!(result, Err(CatalogError::NotFound(_))));
    }

    #[test]
    fn test_search_by_title() {
        let catalog = create_test_catalog();
        catalog
            .store(&[
                create_test_candidate("abc123", "Radiohead - OK Computer"),
                create_test_candidate("def456", "Pink Floyd - The Wall"),
            ])
            .unwrap();

        let query = CatalogSearchQuery {
            query: "Radiohead".to_string(),
            limit: 100,
        };
        let results = catalog.search(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Radiohead - OK Computer");
    }

    #[test]
    fn test_search_by_file_path() {
        let catalog = create_test_catalog();
        catalog
            .store(&[create_test_candidate("abc123", "Test Album")])
            .unwrap();

        let query = CatalogSearchQuery {
            query: "Track One".to_string(),
            limit: 100,
        };
        let results = catalog.search(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].info_hash, "abc123");
    }

    #[test]
    fn test_search_case_insensitive() {
        let catalog = create_test_catalog();
        catalog
            .store(&[create_test_candidate("abc123", "Radiohead - OK Computer")])
            .unwrap();

        let query = CatalogSearchQuery {
            query: "radiohead".to_string(),
            limit: 100,
        };
        let results = catalog.search(&query).unwrap();

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_respects_limit() {
        let catalog = create_test_catalog();
        for i in 0..10 {
            catalog
                .store(&[create_test_candidate(
                    &format!("hash{}", i),
                    &format!("Album {}", i),
                )])
                .unwrap();
        }

        let query = CatalogSearchQuery {
            query: "Album".to_string(),
            limit: 3,
        };
        let results = catalog.search(&query).unwrap();

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_stats() {
        let catalog = create_test_catalog();

        // Empty catalog
        let stats = catalog.stats().unwrap();
        assert_eq!(stats.total_torrents, 0);
        assert_eq!(stats.total_files, 0);

        // Add some torrents
        catalog
            .store(&[
                create_test_candidate("abc123", "Album 1"),
                create_test_candidate("def456", "Album 2"),
            ])
            .unwrap();

        let stats = catalog.stats().unwrap();
        assert_eq!(stats.total_torrents, 2);
        assert_eq!(stats.total_files, 4); // 2 files per torrent
        assert_eq!(stats.unique_indexers, 1); // All from rutracker
        assert!(stats.oldest_entry.is_some());
        assert!(stats.newest_entry.is_some());
    }

    #[test]
    fn test_remove() {
        let catalog = create_test_catalog();
        catalog
            .store(&[create_test_candidate("abc123", "Test Album")])
            .unwrap();

        assert!(catalog.exists("abc123").unwrap());

        catalog.remove("abc123").unwrap();

        assert!(!catalog.exists("abc123").unwrap());
    }

    #[test]
    fn test_remove_nonexistent() {
        let catalog = create_test_catalog();
        let result = catalog.remove("nonexistent");
        assert!(matches!(result, Err(CatalogError::NotFound(_))));
    }

    #[test]
    fn test_clear() {
        let catalog = create_test_catalog();
        catalog
            .store(&[
                create_test_candidate("abc123", "Album 1"),
                create_test_candidate("def456", "Album 2"),
            ])
            .unwrap();

        let stats = catalog.stats().unwrap();
        assert_eq!(stats.total_torrents, 2);

        catalog.clear().unwrap();

        let stats = catalog.stats().unwrap();
        assert_eq!(stats.total_torrents, 0);
        assert_eq!(stats.total_files, 0);
    }

    #[test]
    fn test_merge_sources() {
        let catalog = create_test_catalog();

        // First store with rutracker source
        let mut candidate1 = create_test_candidate("abc123", "Test Album");
        candidate1.sources = vec![TorrentSource {
            indexer: "rutracker".to_string(),
            magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
            torrent_url: None,
            seeders: 10,
            leechers: 2,
            details_url: None,
        }];
        catalog.store(&[candidate1]).unwrap();

        // Store again with different source
        let mut candidate2 = create_test_candidate("abc123", "Test Album");
        candidate2.sources = vec![TorrentSource {
            indexer: "1337x".to_string(),
            magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
            torrent_url: Some("https://1337x.to/abc123.torrent".to_string()),
            seeders: 15,
            leechers: 3,
            details_url: None,
        }];
        catalog.store(&[candidate2]).unwrap();

        // Should have both sources
        let torrent = catalog.get("abc123").unwrap();
        assert_eq!(torrent.sources.len(), 2);

        let indexers: Vec<&str> = torrent.sources.iter().map(|s| s.indexer.as_str()).collect();
        assert!(indexers.contains(&"rutracker"));
        assert!(indexers.contains(&"1337x"));
    }

    #[test]
    fn test_info_hash_case_insensitive() {
        let catalog = create_test_catalog();
        catalog
            .store(&[create_test_candidate("ABC123", "Test Album")])
            .unwrap();

        // Should find with lowercase
        assert!(catalog.exists("abc123").unwrap());

        // Should find with original case
        assert!(catalog.exists("ABC123").unwrap());

        // Get should work with any case
        let torrent = catalog.get("ABC123").unwrap();
        assert_eq!(torrent.info_hash, "abc123"); // Stored as lowercase
    }

    #[test]
    fn test_store_files_creates_entry_if_not_exists() {
        let catalog = create_test_catalog();

        // Store files for a torrent that doesn't exist yet
        let files = vec![
            TorrentFile {
                path: "folder/file1.flac".to_string(),
                size_bytes: 100,
            },
            TorrentFile {
                path: "folder/file2.flac".to_string(),
                size_bytes: 200,
            },
        ];

        catalog
            .store_files("newhash", "New Torrent", &files)
            .unwrap();

        // Should create the torrent entry
        assert!(catalog.exists("newhash").unwrap());

        // Should have the files
        let retrieved_files = catalog.get_files("newhash").unwrap();
        assert!(retrieved_files.is_some());
        let retrieved_files = retrieved_files.unwrap();
        assert_eq!(retrieved_files.len(), 2);
        assert_eq!(retrieved_files[0].path, "folder/file1.flac");
        assert_eq!(retrieved_files[0].size_bytes, 100);
    }

    #[test]
    fn test_store_files_replaces_existing() {
        let catalog = create_test_catalog();

        // Store initial files
        let files1 = vec![TorrentFile {
            path: "old/file.flac".to_string(),
            size_bytes: 100,
        }];
        catalog.store_files("hash123", "Test", &files1).unwrap();

        // Store new files - should replace
        let files2 = vec![
            TorrentFile {
                path: "new/file1.flac".to_string(),
                size_bytes: 200,
            },
            TorrentFile {
                path: "new/file2.flac".to_string(),
                size_bytes: 300,
            },
        ];
        catalog.store_files("hash123", "Test", &files2).unwrap();

        // Should have only the new files
        let retrieved = catalog.get_files("hash123").unwrap().unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].path, "new/file1.flac");
    }

    #[test]
    fn test_get_files_returns_none_for_nonexistent() {
        let catalog = create_test_catalog();

        let files = catalog.get_files("nonexistent").unwrap();
        assert!(files.is_none());
    }

    #[test]
    fn test_get_files_returns_none_for_torrent_without_files() {
        let catalog = create_test_catalog();

        // Create a candidate without files
        let mut candidate = create_test_candidate("abc123", "Test");
        candidate.files = None;
        catalog.store(&[candidate]).unwrap();

        // Should return None (no files)
        let files = catalog.get_files("abc123").unwrap();
        assert!(files.is_none());
    }
}
