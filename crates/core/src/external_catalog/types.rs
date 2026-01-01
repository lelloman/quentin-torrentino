//! Types for external catalog API responses.

use serde::{Deserialize, Serialize};

// ============================================================================
// MusicBrainz Types
// ============================================================================

/// A MusicBrainz release (album).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MusicBrainzRelease {
    /// MusicBrainz Release ID (MBID).
    pub mbid: String,
    /// Release title.
    pub title: String,
    /// Artist credit (combined artist name).
    pub artist_credit: String,
    /// Release date (YYYY-MM-DD or partial).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    /// Tracks in this release.
    #[serde(default)]
    pub tracks: Vec<MusicBrainzTrack>,
    /// Whether cover art is available from Cover Art Archive.
    #[serde(default)]
    pub cover_art_available: bool,
    /// Disambiguation comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
    /// Release country.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

impl MusicBrainzRelease {
    /// Calculate total duration in milliseconds.
    pub fn total_duration_ms(&self) -> Option<u64> {
        let durations: Vec<u64> = self
            .tracks
            .iter()
            .filter_map(|t| t.duration_ms)
            .collect();

        if durations.len() == self.tracks.len() && !durations.is_empty() {
            Some(durations.iter().sum())
        } else {
            None
        }
    }
}

/// A track from a MusicBrainz release.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MusicBrainzTrack {
    /// Track position (1-indexed).
    pub position: u32,
    /// Track title.
    pub title: String,
    /// Duration in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Disc number (for multi-disc releases).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    /// Artist credit if different from release artist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist_credit: Option<String>,
}

// ============================================================================
// TMDB Types
// ============================================================================

/// A TMDB movie.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmdbMovie {
    /// TMDB movie ID.
    pub id: u32,
    /// Movie title.
    pub title: String,
    /// Original title (in original language).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,
    /// Release date (YYYY-MM-DD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    /// Runtime in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_minutes: Option<u32>,
    /// Movie overview/synopsis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// Poster path (relative to TMDB image base URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    /// Backdrop path (relative to TMDB image base URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    /// Genre names.
    #[serde(default)]
    pub genres: Vec<String>,
    /// Average vote (0-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f32>,
}

impl TmdbMovie {
    /// Get the release year from the release date.
    pub fn year(&self) -> Option<u32> {
        self.release_date
            .as_ref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok())
    }
}

/// A TMDB TV series.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmdbSeries {
    /// TMDB series ID.
    pub id: u32,
    /// Series name.
    pub name: String,
    /// Original name (in original language).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
    /// First air date (YYYY-MM-DD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_air_date: Option<String>,
    /// Series overview/synopsis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// Poster path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    /// Backdrop path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    /// Number of seasons.
    #[serde(default)]
    pub number_of_seasons: u32,
    /// Number of episodes.
    #[serde(default)]
    pub number_of_episodes: u32,
    /// Season summaries.
    #[serde(default)]
    pub seasons: Vec<TmdbSeasonSummary>,
    /// Genre names.
    #[serde(default)]
    pub genres: Vec<String>,
    /// Average vote (0-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f32>,
}

/// Summary of a TMDB season (from series response).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmdbSeasonSummary {
    /// Season number (0 for specials).
    pub season_number: u32,
    /// Season name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Episode count.
    #[serde(default)]
    pub episode_count: u32,
    /// Air date of first episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_date: Option<String>,
    /// Poster path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
}

/// Full TMDB season details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmdbSeason {
    /// Season number.
    pub season_number: u32,
    /// Season name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Season overview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// Episodes in this season.
    #[serde(default)]
    pub episodes: Vec<TmdbEpisode>,
    /// Poster path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
}

impl TmdbSeason {
    /// Total runtime in minutes.
    pub fn total_runtime_minutes(&self) -> Option<u32> {
        let runtimes: Vec<u32> = self
            .episodes
            .iter()
            .filter_map(|e| e.runtime_minutes)
            .collect();

        if runtimes.len() == self.episodes.len() && !runtimes.is_empty() {
            Some(runtimes.iter().sum())
        } else {
            None
        }
    }
}

/// A TMDB episode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmdbEpisode {
    /// Episode number.
    pub episode_number: u32,
    /// Episode name.
    pub name: String,
    /// Episode overview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// Runtime in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_minutes: Option<u32>,
    /// Air date (YYYY-MM-DD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_date: Option<String>,
    /// Still image path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still_path: Option<String>,
    /// Average vote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f32>,
}
