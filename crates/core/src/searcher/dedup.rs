//! Deduplication of torrent search results by info_hash.

use std::collections::HashMap;

use super::{RawTorrentResult, TorrentCandidate, TorrentSource};

/// Deduplicate raw torrent results by info_hash.
///
/// Results with the same info_hash are merged into a single `TorrentCandidate`
/// with multiple sources. Results without an info_hash are kept as separate
/// single-source candidates.
///
/// The final list is sorted by total seeders (descending).
pub fn deduplicate_results(raw: Vec<RawTorrentResult>) -> Vec<TorrentCandidate> {
    let mut by_hash: HashMap<String, TorrentCandidate> = HashMap::new();
    let mut no_hash: Vec<TorrentCandidate> = Vec::new();

    for r in raw {
        match r.info_hash {
            Some(hash) if !hash.is_empty() => {
                let hash = hash.to_lowercase();
                if let Some(existing) = by_hash.get_mut(&hash) {
                    // Add as additional source
                    existing.seeders += r.seeders;
                    existing.leechers += r.leechers;
                    existing.sources.push(TorrentSource {
                        indexer: r.indexer,
                        magnet_uri: r.magnet_uri,
                        torrent_url: r.torrent_url,
                        seeders: r.seeders,
                        leechers: r.leechers,
                        details_url: r.details_url,
                    });
                    // Keep earliest publish date
                    if let Some(date) = r.publish_date {
                        existing.publish_date = Some(match existing.publish_date {
                            Some(existing_date) => existing_date.min(date),
                            None => date,
                        });
                    }
                    // Keep files if we didn't have them
                    if existing.files.is_none() && r.files.is_some() {
                        existing.files = r.files;
                    }
                } else {
                    // First occurrence of this hash
                    by_hash.insert(
                        hash.clone(),
                        TorrentCandidate {
                            title: r.title,
                            info_hash: hash,
                            size_bytes: r.size_bytes,
                            seeders: r.seeders,
                            leechers: r.leechers,
                            category: r.category,
                            publish_date: r.publish_date,
                            files: r.files,
                            sources: vec![TorrentSource {
                                indexer: r.indexer,
                                magnet_uri: r.magnet_uri,
                                torrent_url: r.torrent_url,
                                seeders: r.seeders,
                                leechers: r.leechers,
                                details_url: r.details_url,
                            }],
                            from_cache: false,
                        },
                    );
                }
            }
            _ => {
                // No info_hash or empty - can't deduplicate, include as single-source result
                no_hash.push(TorrentCandidate {
                    title: r.title,
                    info_hash: String::new(), // Empty = unknown
                    size_bytes: r.size_bytes,
                    seeders: r.seeders,
                    leechers: r.leechers,
                    category: r.category,
                    publish_date: r.publish_date,
                    files: r.files,
                    sources: vec![TorrentSource {
                        indexer: r.indexer,
                        magnet_uri: r.magnet_uri,
                        torrent_url: r.torrent_url,
                        seeders: r.seeders,
                        leechers: r.leechers,
                        details_url: r.details_url,
                    }],
                    from_cache: false,
                });
            }
        }
    }

    let mut results: Vec<_> = by_hash.into_values().chain(no_hash).collect();
    // Sort by total seeders descending
    results.sort_by(|a, b| b.seeders.cmp(&a.seeders));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::TorrentFile;
    use chrono::{Datelike, TimeZone, Utc};

    fn make_raw(
        title: &str,
        indexer: &str,
        info_hash: Option<&str>,
        seeders: u32,
    ) -> RawTorrentResult {
        RawTorrentResult {
            title: title.to_string(),
            indexer: indexer.to_string(),
            magnet_uri: Some(format!("magnet:?xt=urn:btih:{}", info_hash.unwrap_or("none"))),
            torrent_url: None,
            info_hash: info_hash.map(|s| s.to_string()),
            size_bytes: 1000,
            seeders,
            leechers: 1,
            category: Some("Music".to_string()),
            publish_date: None,
            details_url: None,
            files: None,
        }
    }

    #[test]
    fn test_dedup_single_result() {
        let raw = vec![make_raw("Test", "indexer1", Some("abc123"), 10)];
        let results = deduplicate_results(raw);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test");
        assert_eq!(results[0].info_hash, "abc123");
        assert_eq!(results[0].seeders, 10);
        assert_eq!(results[0].sources.len(), 1);
    }

    #[test]
    fn test_dedup_merges_same_hash() {
        let raw = vec![
            make_raw("Test A", "indexer1", Some("ABC123"), 10), // uppercase hash
            make_raw("Test B", "indexer2", Some("abc123"), 20), // lowercase hash
            make_raw("Test C", "indexer3", Some("ABC123"), 15), // uppercase again
        ];
        let results = deduplicate_results(raw);

        assert_eq!(results.len(), 1);
        // First title is kept
        assert_eq!(results[0].title, "Test A");
        // Hash is normalized to lowercase
        assert_eq!(results[0].info_hash, "abc123");
        // Seeders are summed
        assert_eq!(results[0].seeders, 45);
        // All sources present
        assert_eq!(results[0].sources.len(), 3);
    }

    #[test]
    fn test_dedup_keeps_no_hash_separate() {
        let raw = vec![
            make_raw("With Hash", "indexer1", Some("abc123"), 10),
            make_raw("No Hash 1", "indexer2", None, 20),
            make_raw("No Hash 2", "indexer3", None, 15),
        ];
        let results = deduplicate_results(raw);

        // 3 separate results (1 with hash, 2 without)
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_dedup_empty_hash_treated_as_no_hash() {
        let raw = vec![
            make_raw("Empty Hash 1", "indexer1", Some(""), 10),
            make_raw("Empty Hash 2", "indexer2", Some(""), 20),
        ];
        let results = deduplicate_results(raw);

        // Both kept separate since empty hash can't be used for dedup
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_dedup_sorts_by_seeders() {
        let raw = vec![
            make_raw("Low", "indexer1", Some("hash1"), 5),
            make_raw("High", "indexer2", Some("hash2"), 50),
            make_raw("Medium", "indexer3", Some("hash3"), 20),
        ];
        let results = deduplicate_results(raw);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].seeders, 50); // High first
        assert_eq!(results[1].seeders, 20); // Medium second
        assert_eq!(results[2].seeders, 5); // Low last
    }

    #[test]
    fn test_dedup_keeps_earliest_date() {
        let mut raw1 = make_raw("Test", "indexer1", Some("abc123"), 10);
        raw1.publish_date = Some(Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap());

        let mut raw2 = make_raw("Test", "indexer2", Some("abc123"), 10);
        raw2.publish_date = Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()); // Earlier

        let results = deduplicate_results(vec![raw1, raw2]);

        assert_eq!(results.len(), 1);
        let date = results[0].publish_date.unwrap();
        assert_eq!(date.month(), 1); // January (earlier date kept)
    }

    #[test]
    fn test_dedup_merges_files() {
        let mut raw1 = make_raw("Test", "indexer1", Some("abc123"), 10);
        raw1.files = None;

        let mut raw2 = make_raw("Test", "indexer2", Some("abc123"), 10);
        raw2.files = Some(vec![TorrentFile {
            path: "file.flac".to_string(),
            size_bytes: 1000,
        }]);

        let results = deduplicate_results(vec![raw1, raw2]);

        assert_eq!(results.len(), 1);
        assert!(results[0].files.is_some());
        assert_eq!(results[0].files.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_dedup_per_source_seeders() {
        let raw = vec![
            make_raw("Test", "indexer1", Some("abc123"), 30),
            make_raw("Test", "indexer2", Some("abc123"), 20),
        ];
        let results = deduplicate_results(raw);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].seeders, 50); // Total
        assert_eq!(results[0].sources[0].seeders, 30); // Per-source
        assert_eq!(results[0].sources[1].seeders, 20); // Per-source
    }
}
