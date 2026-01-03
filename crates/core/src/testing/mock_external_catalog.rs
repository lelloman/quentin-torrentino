//! Mock external catalog for testing.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::external_catalog::{
    ExternalCatalog, ExternalCatalogError, MusicBrainzRelease, TmdbMovie, TmdbSeason, TmdbSeries,
};

/// A recorded catalog query for test assertions.
#[derive(Debug, Clone)]
pub enum RecordedCatalogQuery {
    SearchReleases { query: String, limit: u32 },
    GetRelease { mbid: String },
    SearchMovies { query: String, year: Option<u32> },
    GetMovie { tmdb_id: u32 },
    SearchTv { query: String },
    GetTv { tmdb_id: u32 },
    GetTvSeason { tmdb_id: u32, season: u32 },
}

/// Mock implementation of the ExternalCatalog trait.
///
/// Provides controllable behavior for testing:
/// - Return configurable music/movie/TV results
/// - Track queries for assertions
/// - Simulate failures
///
/// # Example
///
/// ```rust,ignore
/// use torrentino_core::testing::{MockExternalCatalog, fixtures};
///
/// let catalog = MockExternalCatalog::new();
///
/// // Add some releases
/// catalog.add_release(fixtures::musicbrainz_release("The Beatles", "Abbey Road", 17)).await;
///
/// // Search
/// let results = catalog.search_releases("beatles", 10).await?;
/// assert_eq!(results.len(), 1);
/// ```
#[derive(Debug)]
pub struct MockExternalCatalog {
    /// MusicBrainz releases by MBID.
    releases: Arc<RwLock<HashMap<String, MusicBrainzRelease>>>,
    /// TMDB movies by ID.
    movies: Arc<RwLock<HashMap<u32, TmdbMovie>>>,
    /// TMDB series by ID.
    series: Arc<RwLock<HashMap<u32, TmdbSeries>>>,
    /// TMDB seasons by (series_id, season_number).
    seasons: Arc<RwLock<HashMap<(u32, u32), TmdbSeason>>>,
    /// Recorded queries.
    queries: Arc<RwLock<Vec<RecordedCatalogQuery>>>,
    /// If set, the next operation will fail with this error.
    next_error: Arc<RwLock<Option<ExternalCatalogError>>>,
}

impl Default for MockExternalCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl MockExternalCatalog {
    /// Create a new empty mock external catalog.
    pub fn new() -> Self {
        Self {
            releases: Arc::new(RwLock::new(HashMap::new())),
            movies: Arc::new(RwLock::new(HashMap::new())),
            series: Arc::new(RwLock::new(HashMap::new())),
            seasons: Arc::new(RwLock::new(HashMap::new())),
            queries: Arc::new(RwLock::new(Vec::new())),
            next_error: Arc::new(RwLock::new(None)),
        }
    }

    // =========================================================================
    // MusicBrainz Configuration
    // =========================================================================

    /// Add a MusicBrainz release.
    pub async fn add_release(&self, release: MusicBrainzRelease) {
        self.releases
            .write()
            .await
            .insert(release.mbid.clone(), release);
    }

    /// Set multiple releases at once.
    pub async fn set_releases(&self, releases: Vec<MusicBrainzRelease>) {
        let mut map = self.releases.write().await;
        map.clear();
        for release in releases {
            map.insert(release.mbid.clone(), release);
        }
    }

    /// Clear all releases.
    pub async fn clear_releases(&self) {
        self.releases.write().await.clear();
    }

    // =========================================================================
    // TMDB Movies Configuration
    // =========================================================================

    /// Add a TMDB movie.
    pub async fn add_movie(&self, movie: TmdbMovie) {
        self.movies.write().await.insert(movie.id, movie);
    }

    /// Set multiple movies at once.
    pub async fn set_movies(&self, movies: Vec<TmdbMovie>) {
        let mut map = self.movies.write().await;
        map.clear();
        for movie in movies {
            map.insert(movie.id, movie);
        }
    }

    /// Clear all movies.
    pub async fn clear_movies(&self) {
        self.movies.write().await.clear();
    }

    // =========================================================================
    // TMDB TV Configuration
    // =========================================================================

    /// Add a TMDB TV series.
    pub async fn add_series(&self, series: TmdbSeries) {
        self.series.write().await.insert(series.id, series);
    }

    /// Set multiple series at once.
    pub async fn set_series(&self, all_series: Vec<TmdbSeries>) {
        let mut map = self.series.write().await;
        map.clear();
        for series in all_series {
            map.insert(series.id, series);
        }
    }

    /// Clear all series.
    pub async fn clear_series(&self) {
        self.series.write().await.clear();
    }

    /// Add a TMDB season.
    pub async fn add_season(&self, series_id: u32, season: TmdbSeason) {
        self.seasons
            .write()
            .await
            .insert((series_id, season.season_number), season);
    }

    /// Clear all seasons.
    pub async fn clear_seasons(&self) {
        self.seasons.write().await.clear();
    }

    // =========================================================================
    // Query Recording
    // =========================================================================

    /// Get all recorded queries.
    pub async fn recorded_queries(&self) -> Vec<RecordedCatalogQuery> {
        self.queries.read().await.clone()
    }

    /// Clear recorded queries.
    pub async fn clear_recorded(&self) {
        self.queries.write().await.clear();
    }

    /// Get the number of queries performed.
    pub async fn query_count(&self) -> usize {
        self.queries.read().await.len()
    }

    // =========================================================================
    // Error Injection
    // =========================================================================

    /// Configure the next operation to fail with the given error.
    pub async fn set_next_error(&self, error: ExternalCatalogError) {
        *self.next_error.write().await = Some(error);
    }

    /// Clear any pending error.
    pub async fn clear_next_error(&self) {
        *self.next_error.write().await = None;
    }

    /// Take the next error if set.
    async fn take_error(&self) -> Option<ExternalCatalogError> {
        self.next_error.write().await.take()
    }

    /// Record a query.
    async fn record(&self, query: RecordedCatalogQuery) {
        self.queries.write().await.push(query);
    }
}

#[async_trait]
impl ExternalCatalog for MockExternalCatalog {
    async fn search_releases(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<MusicBrainzRelease>, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::SearchReleases {
            query: query.to_string(),
            limit,
        })
        .await;

        let releases = self.releases.read().await;
        let query_lower = query.to_lowercase();

        let results: Vec<MusicBrainzRelease> = releases
            .values()
            .filter(|r| {
                r.title.to_lowercase().contains(&query_lower)
                    || r.artist_credit.to_lowercase().contains(&query_lower)
            })
            .take(limit as usize)
            .cloned()
            .collect();

        Ok(results)
    }

    async fn get_release(&self, mbid: &str) -> Result<MusicBrainzRelease, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::GetRelease {
            mbid: mbid.to_string(),
        })
        .await;

        self.releases
            .read()
            .await
            .get(mbid)
            .cloned()
            .ok_or_else(|| ExternalCatalogError::NotFound(format!("Release {} not found", mbid)))
    }

    async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<TmdbMovie>, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::SearchMovies {
            query: query.to_string(),
            year,
        })
        .await;

        let movies = self.movies.read().await;
        let query_lower = query.to_lowercase();

        let results: Vec<TmdbMovie> = movies
            .values()
            .filter(|m| {
                let title_match = m.title.to_lowercase().contains(&query_lower);
                let year_match = year.map_or(true, |y| m.year() == Some(y));
                title_match && year_match
            })
            .cloned()
            .collect();

        Ok(results)
    }

    async fn get_movie(&self, tmdb_id: u32) -> Result<TmdbMovie, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::GetMovie { tmdb_id }).await;

        self.movies
            .read()
            .await
            .get(&tmdb_id)
            .cloned()
            .ok_or_else(|| ExternalCatalogError::NotFound(format!("Movie {} not found", tmdb_id)))
    }

    async fn search_tv(&self, query: &str) -> Result<Vec<TmdbSeries>, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::SearchTv {
            query: query.to_string(),
        })
        .await;

        let series = self.series.read().await;
        let query_lower = query.to_lowercase();

        let results: Vec<TmdbSeries> = series
            .values()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();

        Ok(results)
    }

    async fn get_tv(&self, tmdb_id: u32) -> Result<TmdbSeries, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::GetTv { tmdb_id }).await;

        self.series
            .read()
            .await
            .get(&tmdb_id)
            .cloned()
            .ok_or_else(|| ExternalCatalogError::NotFound(format!("Series {} not found", tmdb_id)))
    }

    async fn get_tv_season(
        &self,
        tmdb_id: u32,
        season: u32,
    ) -> Result<TmdbSeason, ExternalCatalogError> {
        if let Some(err) = self.take_error().await {
            return Err(err);
        }

        self.record(RecordedCatalogQuery::GetTvSeason { tmdb_id, season })
            .await;

        self.seasons
            .read()
            .await
            .get(&(tmdb_id, season))
            .cloned()
            .ok_or_else(|| {
                ExternalCatalogError::NotFound(format!(
                    "Season {} of series {} not found",
                    season, tmdb_id
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures;

    #[tokio::test]
    async fn test_search_releases() {
        let catalog = MockExternalCatalog::new();
        catalog
            .add_release(fixtures::musicbrainz_release("The Beatles", "Abbey Road", 17))
            .await;
        catalog
            .add_release(fixtures::musicbrainz_release("Pink Floyd", "The Wall", 26))
            .await;

        let results = catalog.search_releases("beatles", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artist_credit, "The Beatles");
    }

    #[tokio::test]
    async fn test_get_release() {
        let catalog = MockExternalCatalog::new();
        let release = fixtures::musicbrainz_release("Artist", "Album", 10);
        let mbid = release.mbid.clone();
        catalog.add_release(release).await;

        let result = catalog.get_release(&mbid).await.unwrap();
        assert_eq!(result.title, "Album");
    }

    #[tokio::test]
    async fn test_search_movies() {
        let catalog = MockExternalCatalog::new();
        catalog.add_movie(fixtures::tmdb_movie("The Matrix", 1999)).await;
        catalog.add_movie(fixtures::tmdb_movie("The Matrix Reloaded", 2003)).await;

        // Search without year filter
        let results = catalog.search_movies("matrix", None).await.unwrap();
        assert_eq!(results.len(), 2);

        // Search with year filter
        let results = catalog.search_movies("matrix", Some(1999)).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "The Matrix");
    }

    #[tokio::test]
    async fn test_search_tv() {
        let catalog = MockExternalCatalog::new();
        catalog.add_series(fixtures::tmdb_series("Breaking Bad", 5)).await;
        catalog.add_series(fixtures::tmdb_series("The Office", 9)).await;

        let results = catalog.search_tv("breaking").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Breaking Bad");
        assert_eq!(results[0].number_of_seasons, 5);
    }

    #[tokio::test]
    async fn test_get_tv_season() {
        let catalog = MockExternalCatalog::new();
        let series = fixtures::tmdb_series("Test Show", 3);
        let series_id = series.id;
        catalog.add_series(series).await;
        catalog.add_season(series_id, fixtures::tmdb_season(1, 10)).await;
        catalog.add_season(series_id, fixtures::tmdb_season(2, 12)).await;

        let season = catalog.get_tv_season(series_id, 2).await.unwrap();
        assert_eq!(season.season_number, 2);
        assert_eq!(season.episodes.len(), 12);
    }

    #[tokio::test]
    async fn test_recorded_queries() {
        let catalog = MockExternalCatalog::new();

        catalog.search_releases("test", 10).await.ok();
        catalog.search_movies("movie", None).await.ok();

        let queries = catalog.recorded_queries().await;
        assert_eq!(queries.len(), 2);

        match &queries[0] {
            RecordedCatalogQuery::SearchReleases { query, limit } => {
                assert_eq!(query, "test");
                assert_eq!(*limit, 10);
            }
            _ => panic!("Expected SearchReleases"),
        }
    }

    #[tokio::test]
    async fn test_error_injection() {
        let catalog = MockExternalCatalog::new();
        catalog
            .set_next_error(ExternalCatalogError::RateLimitExceeded)
            .await;

        let result = catalog.search_releases("test", 10).await;
        assert!(result.is_err());

        // Error should be consumed
        let result = catalog.search_releases("test", 10).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_not_found() {
        let catalog = MockExternalCatalog::new();

        let result = catalog.get_release("nonexistent").await;
        assert!(matches!(result, Err(ExternalCatalogError::NotFound(_))));

        let result = catalog.get_movie(99999).await;
        assert!(matches!(result, Err(ExternalCatalogError::NotFound(_))));
    }
}
