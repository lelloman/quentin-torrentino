//! MusicBrainz API client.
//!
//! MusicBrainz requires:
//! - User-Agent header with application name/version and contact info
//! - Rate limiting: 1 request per second

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::{debug, warn};

use super::types::{MusicBrainzRelease, MusicBrainzTrack};
use super::ExternalCatalogError;

/// MusicBrainz API client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzConfig {
    /// User-Agent string (required by MusicBrainz).
    /// Format: "AppName/Version ( contact@example.com )"
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    /// Rate limit delay in milliseconds (default: 1100 for 1 req/sec).
    #[serde(default = "default_rate_limit")]
    pub rate_limit_ms: u64,
    /// Base URL (default: https://musicbrainz.org/ws/2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

fn default_user_agent() -> String {
    format!(
        "QuentinTorrentino/{} ( https://github.com/quentin )",
        env!("CARGO_PKG_VERSION")
    )
}

fn default_rate_limit() -> u64 {
    1100
}

impl Default for MusicBrainzConfig {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            rate_limit_ms: default_rate_limit(),
            base_url: None,
        }
    }
}

/// MusicBrainz API client.
pub struct MusicBrainzClient {
    client: Client,
    base_url: String,
    last_request: Arc<Mutex<Option<Instant>>>,
    rate_limit: Duration,
}

impl MusicBrainzClient {
    /// Create a new MusicBrainz client.
    pub fn new(config: MusicBrainzConfig) -> Result<Self, ExternalCatalogError> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(30))
            .build()?;

        let base_url = config
            .base_url
            .unwrap_or_else(|| "https://musicbrainz.org/ws/2".to_string());

        Ok(Self {
            client,
            base_url,
            last_request: Arc::new(Mutex::new(None)),
            rate_limit: Duration::from_millis(config.rate_limit_ms),
        })
    }

    /// Wait for rate limit if needed.
    async fn wait_for_rate_limit(&self) {
        let mut last = self.last_request.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.rate_limit {
                let wait_time = self.rate_limit - elapsed;
                debug!("MusicBrainz rate limit: waiting {:?}", wait_time);
                sleep(wait_time).await;
            }
        }

        *last = Some(Instant::now());
    }

    /// Search for releases by query string.
    pub async fn search_releases(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<MusicBrainzRelease>, ExternalCatalogError> {
        self.wait_for_rate_limit().await;

        let url = format!("{}/release", self.base_url);
        let limit = limit.min(100); // MusicBrainz max is 100

        debug!("MusicBrainz search: query='{}', limit={}", query, limit);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("query", query),
                ("fmt", "json"),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await?;

        let status = response.status();
        if status == 429 {
            warn!("MusicBrainz rate limit exceeded");
            return Err(ExternalCatalogError::RateLimitExceeded);
        }
        if status == 404 {
            return Err(ExternalCatalogError::NotFound(query.to_string()));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let search_result: MbSearchResponse = response.json().await.map_err(|e| {
            ExternalCatalogError::ParseError(format!("Failed to parse search response: {}", e))
        })?;

        let releases = search_result
            .releases
            .into_iter()
            .map(|r| r.into())
            .collect();

        Ok(releases)
    }

    /// Get a specific release by MBID with full details including tracks.
    pub async fn get_release(
        &self,
        mbid: &str,
    ) -> Result<MusicBrainzRelease, ExternalCatalogError> {
        self.wait_for_rate_limit().await;

        // Include recordings to get track list
        let url = format!("{}/release/{}", self.base_url, mbid);

        debug!("MusicBrainz get release: mbid={}", mbid);

        let response = self
            .client
            .get(&url)
            .query(&[("inc", "recordings+artist-credits"), ("fmt", "json")])
            .send()
            .await?;

        let status = response.status();
        if status == 429 {
            warn!("MusicBrainz rate limit exceeded");
            return Err(ExternalCatalogError::RateLimitExceeded);
        }
        if status == 404 {
            return Err(ExternalCatalogError::NotFound(mbid.to_string()));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ExternalCatalogError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let release: MbRelease = response.json().await.map_err(|e| {
            ExternalCatalogError::ParseError(format!("Failed to parse release response: {}", e))
        })?;

        Ok(release.into())
    }
}

// ============================================================================
// MusicBrainz API Response Types (private)
// ============================================================================

#[derive(Debug, Deserialize)]
struct MbSearchResponse {
    #[serde(default)]
    releases: Vec<MbRelease>,
}

#[derive(Debug, Deserialize)]
struct MbRelease {
    id: String,
    title: String,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MbArtistCredit>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    disambiguation: Option<String>,
    #[serde(rename = "cover-art-archive", default)]
    cover_art_archive: Option<MbCoverArtArchive>,
    #[serde(default)]
    media: Vec<MbMedium>,
}

#[derive(Debug, Deserialize)]
struct MbArtistCredit {
    #[serde(default)]
    name: Option<String>,
    artist: MbArtist,
    #[serde(default)]
    joinphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MbArtist {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct MbCoverArtArchive {
    #[serde(default)]
    artwork: bool,
}

#[derive(Debug, Deserialize)]
struct MbMedium {
    #[serde(default)]
    position: u32,
    #[serde(default)]
    tracks: Vec<MbTrack>,
}

#[derive(Debug, Deserialize)]
struct MbTrack {
    #[serde(default)]
    position: u32,
    title: String,
    #[serde(default)]
    length: Option<u64>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MbArtistCredit>,
}

impl From<MbRelease> for MusicBrainzRelease {
    fn from(mb: MbRelease) -> Self {
        // Build artist credit string
        let artist_credit = mb
            .artist_credit
            .iter()
            .map(|ac| {
                let name = ac.name.clone().unwrap_or_else(|| ac.artist.name.clone());
                let join = ac.joinphrase.clone().unwrap_or_default();
                format!("{}{}", name, join)
            })
            .collect::<String>();

        // Flatten tracks from all media
        let mut tracks = Vec::new();
        for medium in mb.media {
            let disc_number = if medium.position > 0 {
                Some(medium.position)
            } else {
                None
            };

            for track in medium.tracks {
                let track_artist = if !track.artist_credit.is_empty() {
                    Some(
                        track
                            .artist_credit
                            .iter()
                            .map(|ac| {
                                let name =
                                    ac.name.clone().unwrap_or_else(|| ac.artist.name.clone());
                                let join = ac.joinphrase.clone().unwrap_or_default();
                                format!("{}{}", name, join)
                            })
                            .collect::<String>(),
                    )
                } else {
                    None
                };

                tracks.push(MusicBrainzTrack {
                    position: track.position,
                    title: track.title,
                    duration_ms: track.length,
                    disc_number,
                    artist_credit: track_artist,
                });
            }
        }

        let cover_art_available = mb.cover_art_archive.map(|caa| caa.artwork).unwrap_or(false);

        MusicBrainzRelease {
            mbid: mb.id,
            title: mb.title,
            artist_credit,
            release_date: mb.date,
            tracks,
            cover_art_available,
            disambiguation: mb.disambiguation,
            country: mb.country,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artist_credit_parsing() {
        let mb_release = MbRelease {
            id: "test-id".to_string(),
            title: "Test Album".to_string(),
            artist_credit: vec![
                MbArtistCredit {
                    name: Some("Artist 1".to_string()),
                    artist: MbArtist {
                        name: "Artist 1".to_string(),
                    },
                    joinphrase: Some(" & ".to_string()),
                },
                MbArtistCredit {
                    name: Some("Artist 2".to_string()),
                    artist: MbArtist {
                        name: "Artist 2".to_string(),
                    },
                    joinphrase: None,
                },
            ],
            date: Some("2023-01-01".to_string()),
            country: Some("US".to_string()),
            disambiguation: None,
            cover_art_archive: Some(MbCoverArtArchive { artwork: true }),
            media: vec![],
        };

        let release: MusicBrainzRelease = mb_release.into();
        assert_eq!(release.artist_credit, "Artist 1 & Artist 2");
        assert!(release.cover_art_available);
    }

    #[test]
    fn test_tracks_from_media() {
        let mb_release = MbRelease {
            id: "test-id".to_string(),
            title: "Test Album".to_string(),
            artist_credit: vec![MbArtistCredit {
                name: Some("Artist".to_string()),
                artist: MbArtist {
                    name: "Artist".to_string(),
                },
                joinphrase: None,
            }],
            date: None,
            country: None,
            disambiguation: None,
            cover_art_archive: None,
            media: vec![
                MbMedium {
                    position: 1,
                    tracks: vec![
                        MbTrack {
                            position: 1,
                            title: "Track 1".to_string(),
                            length: Some(180000),
                            artist_credit: vec![],
                        },
                        MbTrack {
                            position: 2,
                            title: "Track 2".to_string(),
                            length: Some(200000),
                            artist_credit: vec![],
                        },
                    ],
                },
                MbMedium {
                    position: 2,
                    tracks: vec![MbTrack {
                        position: 1,
                        title: "Track 3".to_string(),
                        length: Some(220000),
                        artist_credit: vec![],
                    }],
                },
            ],
        };

        let release: MusicBrainzRelease = mb_release.into();
        assert_eq!(release.tracks.len(), 3);
        assert_eq!(release.tracks[0].disc_number, Some(1));
        assert_eq!(release.tracks[2].disc_number, Some(2));
        assert_eq!(release.tracks[2].title, "Track 3");
    }
}
