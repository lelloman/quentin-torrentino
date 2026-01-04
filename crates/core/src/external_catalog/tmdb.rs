//! TMDB (The Movie Database) API client.
//!
//! TMDB requires an API key for access.
//! Rate limits are generous (around 40 requests per second).

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::types::{TmdbEpisode, TmdbMovie, TmdbSeason, TmdbSeasonSummary, TmdbSeries};
use super::ExternalCatalogError;

/// TMDB API client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbConfig {
    /// TMDB API key (required).
    /// Can use ${ENV_VAR} syntax to read from environment.
    pub api_key: String,
    /// Base URL (default: https://api.themoviedb.org/3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Image base URL for posters/backdrops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_base_url: Option<String>,
}

/// TMDB API client.
pub struct TmdbClient {
    client: Client,
    base_url: String,
    api_key: String,
    #[allow(dead_code)]
    image_base_url: String,
}

impl TmdbClient {
    /// Create a new TMDB client.
    pub fn new(config: TmdbConfig) -> Result<Self, ExternalCatalogError> {
        if config.api_key.is_empty() {
            return Err(ExternalCatalogError::NotConfigured(
                "TMDB API key is required".to_string(),
            ));
        }

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        let base_url = config
            .base_url
            .unwrap_or_else(|| "https://api.themoviedb.org/3".to_string());

        let image_base_url = config
            .image_base_url
            .unwrap_or_else(|| "https://image.tmdb.org/t/p".to_string());

        Ok(Self {
            client,
            base_url,
            api_key: config.api_key,
            image_base_url,
        })
    }

    /// Search for movies by query.
    pub async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<TmdbMovie>, ExternalCatalogError> {
        let url = format!("{}/search/movie", self.base_url);

        debug!("TMDB movie search: query='{}', year={:?}", query, year);

        let mut request = self
            .client
            .get(&url)
            .query(&[("api_key", &self.api_key), ("query", &query.to_string())]);

        if let Some(y) = year {
            request = request.query(&[("year", &y.to_string())]);
        }

        let response = request.send().await?;

        let status = response.status();
        if status == 401 {
            return Err(ExternalCatalogError::NotConfigured(
                "Invalid TMDB API key".to_string(),
            ));
        }
        if status == 429 {
            return Err(ExternalCatalogError::RateLimitExceeded);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let search_result: TmdbSearchResponse<TmdbMovieResult> =
            response.json().await.map_err(|e| {
                ExternalCatalogError::ParseError(format!(
                    "Failed to parse movie search response: {}",
                    e
                ))
            })?;

        let movies = search_result
            .results
            .into_iter()
            .map(|r| r.into())
            .collect();

        Ok(movies)
    }

    /// Get a specific movie by TMDB ID.
    pub async fn get_movie(&self, tmdb_id: u32) -> Result<TmdbMovie, ExternalCatalogError> {
        let url = format!("{}/movie/{}", self.base_url, tmdb_id);

        debug!("TMDB get movie: id={}", tmdb_id);

        let response = self
            .client
            .get(&url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;

        let status = response.status();
        if status == 404 {
            return Err(ExternalCatalogError::NotFound(format!(
                "Movie ID {}",
                tmdb_id
            )));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let movie: TmdbMovieDetails = response.json().await.map_err(|e| {
            ExternalCatalogError::ParseError(format!("Failed to parse movie response: {}", e))
        })?;

        Ok(movie.into())
    }

    /// Search for TV series by query.
    pub async fn search_tv(&self, query: &str) -> Result<Vec<TmdbSeries>, ExternalCatalogError> {
        let url = format!("{}/search/tv", self.base_url);

        debug!("TMDB TV search: query='{}'", query);

        let response = self
            .client
            .get(&url)
            .query(&[("api_key", &self.api_key), ("query", &query.to_string())])
            .send()
            .await?;

        let status = response.status();
        if status == 401 {
            return Err(ExternalCatalogError::NotConfigured(
                "Invalid TMDB API key".to_string(),
            ));
        }
        if status == 429 {
            return Err(ExternalCatalogError::RateLimitExceeded);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let search_result: TmdbSearchResponse<TmdbTvResult> =
            response.json().await.map_err(|e| {
                ExternalCatalogError::ParseError(format!(
                    "Failed to parse TV search response: {}",
                    e
                ))
            })?;

        let series = search_result
            .results
            .into_iter()
            .map(|r| r.into())
            .collect();

        Ok(series)
    }

    /// Get a specific TV series by TMDB ID.
    pub async fn get_tv(&self, tmdb_id: u32) -> Result<TmdbSeries, ExternalCatalogError> {
        let url = format!("{}/tv/{}", self.base_url, tmdb_id);

        debug!("TMDB get TV: id={}", tmdb_id);

        let response = self
            .client
            .get(&url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;

        let status = response.status();
        if status == 404 {
            return Err(ExternalCatalogError::NotFound(format!(
                "TV series ID {}",
                tmdb_id
            )));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let series: TmdbTvDetails = response.json().await.map_err(|e| {
            ExternalCatalogError::ParseError(format!("Failed to parse TV response: {}", e))
        })?;

        Ok(series.into())
    }

    /// Get a specific TV season.
    pub async fn get_tv_season(
        &self,
        tmdb_id: u32,
        season: u32,
    ) -> Result<TmdbSeason, ExternalCatalogError> {
        let url = format!("{}/tv/{}/season/{}", self.base_url, tmdb_id, season);

        debug!("TMDB get season: series={}, season={}", tmdb_id, season);

        let response = self
            .client
            .get(&url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;

        let status = response.status();
        if status == 404 {
            return Err(ExternalCatalogError::NotFound(format!(
                "TV series {} season {}",
                tmdb_id, season
            )));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let season_details: TmdbSeasonDetails = response.json().await.map_err(|e| {
            ExternalCatalogError::ParseError(format!("Failed to parse season response: {}", e))
        })?;

        Ok(season_details.into())
    }
}

// ============================================================================
// TMDB API Response Types (private)
// ============================================================================

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse<T> {
    results: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct TmdbMovieResult {
    id: u32,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    vote_average: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct TmdbMovieDetails {
    id: u32,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    runtime: Option<u32>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    genres: Vec<TmdbGenre>,
    vote_average: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct TmdbGenre {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TmdbTvResult {
    id: u32,
    name: String,
    original_name: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    vote_average: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct TmdbTvDetails {
    id: u32,
    name: String,
    original_name: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    number_of_seasons: Option<u32>,
    number_of_episodes: Option<u32>,
    #[serde(default)]
    seasons: Vec<TmdbSeasonResult>,
    #[serde(default)]
    genres: Vec<TmdbGenre>,
    vote_average: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct TmdbSeasonResult {
    season_number: u32,
    name: Option<String>,
    episode_count: Option<u32>,
    air_date: Option<String>,
    poster_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbSeasonDetails {
    season_number: u32,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    #[serde(default)]
    episodes: Vec<TmdbEpisodeResult>,
}

#[derive(Debug, Deserialize)]
struct TmdbEpisodeResult {
    episode_number: u32,
    name: String,
    overview: Option<String>,
    runtime: Option<u32>,
    air_date: Option<String>,
    still_path: Option<String>,
    vote_average: Option<f32>,
}

// ============================================================================
// Conversions
// ============================================================================

impl From<TmdbMovieResult> for TmdbMovie {
    fn from(r: TmdbMovieResult) -> Self {
        Self {
            id: r.id,
            title: r.title,
            original_title: r.original_title,
            release_date: r.release_date,
            runtime_minutes: None, // Not available in search results
            overview: r.overview,
            poster_path: r.poster_path,
            backdrop_path: r.backdrop_path,
            genres: vec![],
            vote_average: r.vote_average,
        }
    }
}

impl From<TmdbMovieDetails> for TmdbMovie {
    fn from(d: TmdbMovieDetails) -> Self {
        Self {
            id: d.id,
            title: d.title,
            original_title: d.original_title,
            release_date: d.release_date,
            runtime_minutes: d.runtime,
            overview: d.overview,
            poster_path: d.poster_path,
            backdrop_path: d.backdrop_path,
            genres: d.genres.into_iter().map(|g| g.name).collect(),
            vote_average: d.vote_average,
        }
    }
}

impl From<TmdbTvResult> for TmdbSeries {
    fn from(r: TmdbTvResult) -> Self {
        Self {
            id: r.id,
            name: r.name,
            original_name: r.original_name,
            first_air_date: r.first_air_date,
            overview: r.overview,
            poster_path: r.poster_path,
            backdrop_path: r.backdrop_path,
            number_of_seasons: 0,
            number_of_episodes: 0,
            seasons: vec![],
            genres: vec![],
            vote_average: r.vote_average,
        }
    }
}

impl From<TmdbTvDetails> for TmdbSeries {
    fn from(d: TmdbTvDetails) -> Self {
        Self {
            id: d.id,
            name: d.name,
            original_name: d.original_name,
            first_air_date: d.first_air_date,
            overview: d.overview,
            poster_path: d.poster_path,
            backdrop_path: d.backdrop_path,
            number_of_seasons: d.number_of_seasons.unwrap_or(0),
            number_of_episodes: d.number_of_episodes.unwrap_or(0),
            seasons: d.seasons.into_iter().map(|s| s.into()).collect(),
            genres: d.genres.into_iter().map(|g| g.name).collect(),
            vote_average: d.vote_average,
        }
    }
}

impl From<TmdbSeasonResult> for TmdbSeasonSummary {
    fn from(s: TmdbSeasonResult) -> Self {
        Self {
            season_number: s.season_number,
            name: s.name,
            episode_count: s.episode_count.unwrap_or(0),
            air_date: s.air_date,
            poster_path: s.poster_path,
        }
    }
}

impl From<TmdbSeasonDetails> for TmdbSeason {
    fn from(d: TmdbSeasonDetails) -> Self {
        Self {
            season_number: d.season_number,
            name: d.name,
            overview: d.overview,
            episodes: d.episodes.into_iter().map(|e| e.into()).collect(),
            poster_path: d.poster_path,
        }
    }
}

impl From<TmdbEpisodeResult> for TmdbEpisode {
    fn from(e: TmdbEpisodeResult) -> Self {
        Self {
            episode_number: e.episode_number,
            name: e.name,
            overview: e.overview,
            runtime_minutes: e.runtime,
            air_date: e.air_date,
            still_path: e.still_path,
            vote_average: e.vote_average,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movie_result_conversion() {
        let result = TmdbMovieResult {
            id: 603,
            title: "The Matrix".to_string(),
            original_title: Some("The Matrix".to_string()),
            release_date: Some("1999-03-30".to_string()),
            overview: Some("A computer hacker...".to_string()),
            poster_path: Some("/poster.jpg".to_string()),
            backdrop_path: None,
            vote_average: Some(8.2),
        };

        let movie: TmdbMovie = result.into();
        assert_eq!(movie.id, 603);
        assert_eq!(movie.title, "The Matrix");
        assert_eq!(movie.year(), Some(1999));
        assert!(movie.runtime_minutes.is_none()); // Not in search results
    }

    #[test]
    fn test_movie_details_conversion() {
        let details = TmdbMovieDetails {
            id: 603,
            title: "The Matrix".to_string(),
            original_title: Some("The Matrix".to_string()),
            release_date: Some("1999-03-30".to_string()),
            runtime: Some(136),
            overview: Some("A computer hacker...".to_string()),
            poster_path: Some("/poster.jpg".to_string()),
            backdrop_path: None,
            genres: vec![
                TmdbGenre {
                    name: "Action".to_string(),
                },
                TmdbGenre {
                    name: "Science Fiction".to_string(),
                },
            ],
            vote_average: Some(8.2),
        };

        let movie: TmdbMovie = details.into();
        assert_eq!(movie.runtime_minutes, Some(136));
        assert_eq!(movie.genres, vec!["Action", "Science Fiction"]);
    }

    #[test]
    fn test_tv_details_conversion() {
        let details = TmdbTvDetails {
            id: 1396,
            name: "Breaking Bad".to_string(),
            original_name: Some("Breaking Bad".to_string()),
            first_air_date: Some("2008-01-20".to_string()),
            overview: Some("A high school chemistry teacher...".to_string()),
            poster_path: Some("/poster.jpg".to_string()),
            backdrop_path: None,
            number_of_seasons: Some(5),
            number_of_episodes: Some(62),
            seasons: vec![TmdbSeasonResult {
                season_number: 1,
                name: Some("Season 1".to_string()),
                episode_count: Some(7),
                air_date: Some("2008-01-20".to_string()),
                poster_path: None,
            }],
            genres: vec![TmdbGenre {
                name: "Drama".to_string(),
            }],
            vote_average: Some(9.5),
        };

        let series: TmdbSeries = details.into();
        assert_eq!(series.id, 1396);
        assert_eq!(series.number_of_seasons, 5);
        assert_eq!(series.seasons.len(), 1);
        assert_eq!(series.seasons[0].episode_count, 7);
    }
}
