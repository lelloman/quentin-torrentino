//! Testing utilities and mock implementations for E2E tests.
//!
//! This module provides mock implementations of all external service traits,
//! allowing comprehensive E2E testing without real infrastructure.
//!
//! # Example
//!
//! ```rust,ignore
//! use torrentino_core::testing::{MockTorrentClient, MockSearcher, MockExternalCatalog};
//!
//! let torrent_client = MockTorrentClient::new();
//! let searcher = MockSearcher::new();
//! let external_catalog = MockExternalCatalog::new();
//!
//! // Configure mock responses
//! searcher.set_results(vec![/* candidates */]);
//! torrent_client.set_progress("hash", 0.5);
//!
//! // Use in AppState...
//! ```

mod mock_converter;
mod mock_external_catalog;
mod mock_placer;
mod mock_searcher;
mod mock_torrent_client;

pub use mock_converter::MockConverter;
pub use mock_external_catalog::MockExternalCatalog;
pub use mock_placer::MockPlacer;
pub use mock_searcher::MockSearcher;
pub use mock_torrent_client::MockTorrentClient;

/// Test fixtures and helper functions.
pub mod fixtures {
    use crate::searcher::{TorrentCandidate, TorrentSource};
    use crate::external_catalog::{
        MusicBrainzRelease, MusicBrainzTrack, TmdbMovie, TmdbSeries, TmdbSeason, TmdbEpisode,
    };

    /// Create a test torrent candidate with reasonable defaults.
    pub fn torrent_candidate(title: &str, info_hash: &str) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: info_hash.to_string(),
            size_bytes: 1024 * 1024 * 100, // 100 MB
            seeders: 50,
            leechers: 10,
            category: Some("Music".to_string()),
            publish_date: None,
            files: None,
            sources: vec![TorrentSource {
                indexer: "mock-indexer".to_string(),
                magnet_uri: Some(format!("magnet:?xt=urn:btih:{}", info_hash)),
                torrent_url: None,
                seeders: 50,
                leechers: 10,
                details_url: None,
            }],
            from_cache: false,
        }
    }

    /// Create a test torrent candidate for audio content.
    pub fn audio_candidate(artist: &str, album: &str, info_hash: &str) -> TorrentCandidate {
        torrent_candidate(&format!("{} - {} [FLAC]", artist, album), info_hash)
    }

    /// Create a test torrent candidate for video content.
    pub fn video_candidate(title: &str, year: u32, info_hash: &str) -> TorrentCandidate {
        let mut candidate = torrent_candidate(&format!("{} ({}) 1080p BluRay", title, year), info_hash);
        candidate.category = Some("Movies".to_string());
        candidate.size_bytes = 1024 * 1024 * 1024 * 4; // 4 GB
        candidate
    }

    /// Create a test MusicBrainz release.
    pub fn musicbrainz_release(artist: &str, title: &str, tracks: u32) -> MusicBrainzRelease {
        MusicBrainzRelease {
            mbid: format!("mb-{}", title.to_lowercase().replace(' ', "-")),
            title: title.to_string(),
            artist_credit: artist.to_string(),
            release_date: Some("2024-01-01".to_string()),
            tracks: (1..=tracks)
                .map(|i| MusicBrainzTrack {
                    position: i,
                    title: format!("Track {}", i),
                    duration_ms: Some(180_000 + (i as u64 * 10_000)),
                    disc_number: Some(1),
                    artist_credit: None,
                })
                .collect(),
            cover_art_available: true,
            disambiguation: None,
            country: Some("US".to_string()),
        }
    }

    /// Create a test TMDB movie.
    pub fn tmdb_movie(title: &str, year: u32) -> TmdbMovie {
        TmdbMovie {
            id: (year * 100 + title.len() as u32) % 100000,
            title: title.to_string(),
            original_title: None,
            release_date: Some(format!("{}-06-15", year)),
            runtime_minutes: Some(120),
            overview: Some(format!("A movie about {}.", title.to_lowercase())),
            poster_path: Some("/poster.jpg".to_string()),
            backdrop_path: Some("/backdrop.jpg".to_string()),
            genres: vec!["Drama".to_string(), "Thriller".to_string()],
            vote_average: Some(7.5),
        }
    }

    /// Create a test TMDB TV series.
    pub fn tmdb_series(name: &str, seasons: u32) -> TmdbSeries {
        use crate::external_catalog::TmdbSeasonSummary;

        TmdbSeries {
            id: (name.len() as u32 * 1000) % 100000,
            name: name.to_string(),
            original_name: None,
            first_air_date: Some("2020-01-01".to_string()),
            overview: Some(format!("A TV series about {}.", name.to_lowercase())),
            poster_path: Some("/poster.jpg".to_string()),
            backdrop_path: Some("/backdrop.jpg".to_string()),
            number_of_seasons: seasons,
            number_of_episodes: seasons * 10,
            seasons: (1..=seasons)
                .map(|s| TmdbSeasonSummary {
                    season_number: s,
                    name: Some(format!("Season {}", s)),
                    episode_count: 10,
                    air_date: Some(format!("{}-01-01", 2020 + s - 1)),
                    poster_path: None,
                })
                .collect(),
            genres: vec!["Drama".to_string()],
            vote_average: Some(8.0),
        }
    }

    /// Create a test TMDB season.
    pub fn tmdb_season(season_number: u32, episodes: u32) -> TmdbSeason {
        TmdbSeason {
            season_number,
            name: Some(format!("Season {}", season_number)),
            overview: Some(format!("Season {} of the series.", season_number)),
            episodes: (1..=episodes)
                .map(|e| TmdbEpisode {
                    episode_number: e,
                    name: format!("Episode {}", e),
                    overview: Some(format!("Episode {} description.", e)),
                    runtime_minutes: Some(45),
                    air_date: Some(format!("2020-01-{:02}", e)),
                    still_path: None,
                    vote_average: Some(8.0),
                })
                .collect(),
            poster_path: None,
        }
    }
}
