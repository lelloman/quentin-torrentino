//! External catalog integration for MusicBrainz and TMDB.
//!
//! This module provides clients for querying external metadata catalogs
//! to enrich ticket creation and improve matching accuracy.

mod musicbrainz;
mod tmdb;
mod types;

pub use musicbrainz::{MusicBrainzClient, MusicBrainzConfig};
pub use tmdb::{TmdbClient, TmdbConfig};
pub use types::*;

use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur when interacting with external catalogs.
#[derive(Debug, Error)]
pub enum ExternalCatalogError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded, please wait before retrying")]
    RateLimitExceeded,

    /// Resource not found (404).
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// API returned an error.
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    /// Failed to parse response.
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// Client not configured (missing API key, etc.).
    #[error("Client not configured: {0}")]
    NotConfigured(String),
}

/// Trait for external catalog clients.
///
/// This trait is implemented by both MusicBrainzClient and TmdbClient,
/// allowing them to be used interchangeably where needed.
#[async_trait]
pub trait ExternalCatalog: Send + Sync {
    // MusicBrainz operations

    /// Search for music releases by query.
    async fn search_releases(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<MusicBrainzRelease>, ExternalCatalogError>;

    /// Get a specific release by MusicBrainz ID.
    async fn get_release(&self, mbid: &str) -> Result<MusicBrainzRelease, ExternalCatalogError>;

    // TMDB operations

    /// Search for movies by query.
    async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<TmdbMovie>, ExternalCatalogError>;

    /// Get a specific movie by TMDB ID.
    async fn get_movie(&self, tmdb_id: u32) -> Result<TmdbMovie, ExternalCatalogError>;

    /// Search for TV series by query.
    async fn search_tv(&self, query: &str) -> Result<Vec<TmdbSeries>, ExternalCatalogError>;

    /// Get a specific TV series by TMDB ID.
    async fn get_tv(&self, tmdb_id: u32) -> Result<TmdbSeries, ExternalCatalogError>;

    /// Get a specific TV season.
    async fn get_tv_season(
        &self,
        tmdb_id: u32,
        season: u32,
    ) -> Result<TmdbSeason, ExternalCatalogError>;
}

/// Combined external catalog client that delegates to appropriate backends.
pub struct CombinedCatalogClient {
    musicbrainz: Option<MusicBrainzClient>,
    tmdb: Option<TmdbClient>,
}

impl CombinedCatalogClient {
    /// Create a new combined client with optional backends.
    pub fn new(musicbrainz: Option<MusicBrainzClient>, tmdb: Option<TmdbClient>) -> Self {
        Self { musicbrainz, tmdb }
    }

    /// Check if MusicBrainz is available.
    pub fn has_musicbrainz(&self) -> bool {
        self.musicbrainz.is_some()
    }

    /// Check if TMDB is available.
    pub fn has_tmdb(&self) -> bool {
        self.tmdb.is_some()
    }
}

#[async_trait]
impl ExternalCatalog for CombinedCatalogClient {
    async fn search_releases(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<MusicBrainzRelease>, ExternalCatalogError> {
        match &self.musicbrainz {
            Some(client) => client.search_releases(query, limit).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "MusicBrainz client not configured".to_string(),
            )),
        }
    }

    async fn get_release(&self, mbid: &str) -> Result<MusicBrainzRelease, ExternalCatalogError> {
        match &self.musicbrainz {
            Some(client) => client.get_release(mbid).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "MusicBrainz client not configured".to_string(),
            )),
        }
    }

    async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<TmdbMovie>, ExternalCatalogError> {
        match &self.tmdb {
            Some(client) => client.search_movies(query, year).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "TMDB client not configured".to_string(),
            )),
        }
    }

    async fn get_movie(&self, tmdb_id: u32) -> Result<TmdbMovie, ExternalCatalogError> {
        match &self.tmdb {
            Some(client) => client.get_movie(tmdb_id).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "TMDB client not configured".to_string(),
            )),
        }
    }

    async fn search_tv(&self, query: &str) -> Result<Vec<TmdbSeries>, ExternalCatalogError> {
        match &self.tmdb {
            Some(client) => client.search_tv(query).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "TMDB client not configured".to_string(),
            )),
        }
    }

    async fn get_tv(&self, tmdb_id: u32) -> Result<TmdbSeries, ExternalCatalogError> {
        match &self.tmdb {
            Some(client) => client.get_tv(tmdb_id).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "TMDB client not configured".to_string(),
            )),
        }
    }

    async fn get_tv_season(
        &self,
        tmdb_id: u32,
        season: u32,
    ) -> Result<TmdbSeason, ExternalCatalogError> {
        match &self.tmdb {
            Some(client) => client.get_tv_season(tmdb_id, season).await,
            None => Err(ExternalCatalogError::NotConfigured(
                "TMDB client not configured".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_musicbrainz_release_total_duration() {
        let release = MusicBrainzRelease {
            mbid: "test".to_string(),
            title: "Test Album".to_string(),
            artist_credit: "Test Artist".to_string(),
            release_date: None,
            tracks: vec![
                MusicBrainzTrack {
                    position: 1,
                    title: "Track 1".to_string(),
                    duration_ms: Some(180_000),
                    disc_number: None,
                    artist_credit: None,
                },
                MusicBrainzTrack {
                    position: 2,
                    title: "Track 2".to_string(),
                    duration_ms: Some(240_000),
                    disc_number: None,
                    artist_credit: None,
                },
            ],
            cover_art_available: false,
            disambiguation: None,
            country: None,
        };

        assert_eq!(release.total_duration_ms(), Some(420_000));
    }

    #[test]
    fn test_tmdb_movie_year() {
        let movie = TmdbMovie {
            id: 1,
            title: "Test Movie".to_string(),
            original_title: None,
            release_date: Some("1999-03-31".to_string()),
            runtime_minutes: Some(120),
            overview: None,
            poster_path: None,
            backdrop_path: None,
            genres: vec![],
            vote_average: None,
        };

        assert_eq!(movie.year(), Some(1999));
    }

    #[test]
    fn test_tmdb_season_total_runtime() {
        let season = TmdbSeason {
            season_number: 1,
            name: None,
            overview: None,
            episodes: vec![
                TmdbEpisode {
                    episode_number: 1,
                    name: "Pilot".to_string(),
                    overview: None,
                    runtime_minutes: Some(45),
                    air_date: None,
                    still_path: None,
                    vote_average: None,
                },
                TmdbEpisode {
                    episode_number: 2,
                    name: "Episode 2".to_string(),
                    overview: None,
                    runtime_minutes: Some(42),
                    air_date: None,
                    still_path: None,
                    vote_average: None,
                },
            ],
            poster_path: None,
        };

        assert_eq!(season.total_runtime_minutes(), Some(87));
    }
}
