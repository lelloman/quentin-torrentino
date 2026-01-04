//! Content-specific processing dispatch.
//!
//! Routes query building, scoring, file mapping, and post-processing
//! to content-type-specific implementations based on `ExpectedContent`.
//!
//! # Architecture
//!
//! ```text
//! match ticket.query_context.expected {
//!     Album/Track    => content::music::*
//!     Movie/TvEpisode => content::video::*
//!     _              => content::generic::*
//! }
//! ```
//!
//! # Content Types
//!
//! - **Music** (`music.rs`): Albums and tracks - Phase 5c
//! - **Video** (`video.rs`): Movies and TV episodes - Phase 5d
//! - **Generic** (`generic.rs`): Fallback for unknown content

mod generic;
mod music;
mod types;
mod video;

use std::path::Path;

use crate::searcher::{TorrentCandidate, TorrentFile};
use crate::textbrain::{
    FileMapping, MatchResult, QueryBuildResult, TextBrainConfig, TextBrainError,
};
use crate::ticket::{ExpectedContent, QueryContext, Ticket};

pub use types::{ContentError, PostProcessResult};

/// Build fallback queries for discography/collection search.
///
/// Called when specific content queries fail to find suitable matches.
/// Returns broader queries targeting artist discographies, complete collections, etc.
///
/// Currently only implemented for music content.
pub fn build_fallback_queries(context: &QueryContext) -> Vec<String> {
    match &context.expected {
        Some(ExpectedContent::Album { artist, .. })
        | Some(ExpectedContent::Track { artist, .. }) => {
            let audio_constraints = context
                .search_constraints
                .as_ref()
                .and_then(|sc| sc.audio.as_ref());
            music::build_discography_queries(artist.as_deref(), audio_constraints)
        }
        // Video and generic content don't have discography fallback
        _ => vec![],
    }
}

/// Check if a candidate is a discography/collection that contains the target album.
///
/// For discography candidates, we need to verify the target album is present
/// in the file listing before considering it a match.
pub fn is_discography_candidate(context: &QueryContext, candidate: &TorrentCandidate) -> bool {
    match &context.expected {
        Some(ExpectedContent::Album { artist, .. })
        | Some(ExpectedContent::Track { artist, .. }) => {
            music::is_discography_candidate(artist.as_deref(), candidate)
        }
        _ => false,
    }
}

/// Score a discography candidate for containing the target album.
///
/// This scoring function is used during fallback acquisition to evaluate
/// whether a discography/collection contains the specific album we're looking for.
/// Returns a score and whether the album was found in the file listing.
pub async fn score_discography_candidate(
    context: &QueryContext,
    candidate: &TorrentCandidate,
    config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    match &context.expected {
        Some(ExpectedContent::Album { .. }) | Some(ExpectedContent::Track { .. }) => {
            music::score_discography_candidate(context, candidate, config).await
        }
        _ => {
            // Non-music content: regular scoring
            score_candidates(context, std::slice::from_ref(candidate), config).await
        }
    }
}

/// Build search queries based on content type.
///
/// Dispatches to content-specific query builders based on `ExpectedContent`.
pub async fn build_queries(
    context: &QueryContext,
    config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    match &context.expected {
        Some(ExpectedContent::Album { .. }) | Some(ExpectedContent::Track { .. }) => {
            music::build_queries(context, config).await
        }
        Some(ExpectedContent::Movie { .. }) | Some(ExpectedContent::TvEpisode { .. }) => {
            video::build_queries(context, config).await
        }
        _ => generic::build_queries(context, config).await,
    }
}

/// Score candidates based on content type.
///
/// Dispatches to content-specific scorers based on `ExpectedContent`.
pub async fn score_candidates(
    context: &QueryContext,
    candidates: &[TorrentCandidate],
    config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    match &context.expected {
        Some(ExpectedContent::Album { .. }) | Some(ExpectedContent::Track { .. }) => {
            music::score_candidates(context, candidates, config).await
        }
        Some(ExpectedContent::Movie { .. }) | Some(ExpectedContent::TvEpisode { .. }) => {
            video::score_candidates(context, candidates, config).await
        }
        _ => generic::score_candidates(context, candidates, config).await,
    }
}

/// Map torrent files to expected content items.
///
/// Dispatches to content-specific file mappers based on `ExpectedContent`.
pub fn map_files(context: &QueryContext, files: &[TorrentFile]) -> Vec<FileMapping> {
    match &context.expected {
        Some(ExpectedContent::Album { .. }) | Some(ExpectedContent::Track { .. }) => {
            music::map_files(context, files)
        }
        Some(ExpectedContent::Movie { .. }) | Some(ExpectedContent::TvEpisode { .. }) => {
            video::map_files(context, files)
        }
        _ => generic::map_files(context, files),
    }
}

/// Post-process after download completes.
///
/// Dispatches to content-specific post-processors based on `ExpectedContent`.
/// Can fetch external assets like cover art (music) or subtitles (video).
pub async fn post_process(
    ticket: &Ticket,
    download_path: &Path,
) -> Result<PostProcessResult, ContentError> {
    match &ticket.query_context.expected {
        Some(ExpectedContent::Album { .. }) | Some(ExpectedContent::Track { .. }) => {
            music::post_process(ticket, download_path).await
        }
        Some(ExpectedContent::Movie { .. }) | Some(ExpectedContent::TvEpisode { .. }) => {
            video::post_process(ticket, download_path).await
        }
        _ => generic::post_process(ticket, download_path).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::TorrentSource;
    use crate::textbrain::TextBrainMode;
    use crate::ticket::{ExpectedContent, ExpectedTrack, TicketState};

    fn make_query_context(description: &str, expected: Option<ExpectedContent>) -> QueryContext {
        QueryContext {
            tags: vec![],
            description: description.to_string(),
            expected,
            catalog_reference: None,
            search_constraints: None,
        }
    }

    fn make_config() -> TextBrainConfig {
        TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            ..Default::default()
        }
    }

    fn make_candidate(title: &str) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: "abc123".to_string(),
            size_bytes: 100_000_000,
            seeders: 10,
            leechers: 2,
            category: None,
            publish_date: None,
            files: None,
            sources: vec![TorrentSource {
                indexer: "test".to_string(),
                magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
                torrent_url: None,
                seeders: 10,
                leechers: 2,
                details_url: None,
            }],
            from_cache: false,
        }
    }

    fn make_ticket(expected: Option<ExpectedContent>) -> Ticket {
        let now = chrono::Utc::now();
        Ticket {
            id: "test-123".to_string(),
            query_context: QueryContext {
                tags: vec![],
                description: "test".to_string(),
                expected,
                catalog_reference: None,
                search_constraints: None,
            },
            dest_path: "/tmp/test".to_string(),
            priority: 0,
            state: TicketState::Pending,
            created_at: now,
            updated_at: now,
            created_by: "test".to_string(),
            output_constraints: None,
            retry_count: 0,
        }
    }

    // =========================================================================
    // build_queries dispatch tests
    // =========================================================================

    #[tokio::test]
    async fn test_build_queries_dispatches_to_music_album() {
        let context = make_query_context(
            "Dark Side of the Moon by Pink Floyd",
            Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![],
            }),
        );

        let result = build_queries(&context, &make_config()).await.unwrap();
        assert!(!result.queries.is_empty());
    }

    #[tokio::test]
    async fn test_build_queries_dispatches_to_music_track() {
        let context = make_query_context(
            "Comfortably Numb by Pink Floyd",
            Some(ExpectedContent::Track {
                artist: Some("Pink Floyd".to_string()),
                title: "Comfortably Numb".to_string(),
            }),
        );

        let result = build_queries(&context, &make_config()).await.unwrap();
        assert!(!result.queries.is_empty());
    }

    #[tokio::test]
    async fn test_build_queries_dispatches_to_video_movie() {
        let context = make_query_context(
            "Inception 2010",
            Some(ExpectedContent::Movie {
                title: "Inception".to_string(),
                year: Some(2010),
            }),
        );

        let result = build_queries(&context, &make_config()).await.unwrap();
        assert!(!result.queries.is_empty());
    }

    #[tokio::test]
    async fn test_build_queries_dispatches_to_video_tv() {
        let context = make_query_context(
            "Breaking Bad S01E01",
            Some(ExpectedContent::TvEpisode {
                series: "Breaking Bad".to_string(),
                season: 1,
                episodes: vec![1],
            }),
        );

        let result = build_queries(&context, &make_config()).await.unwrap();
        assert!(!result.queries.is_empty());
    }

    #[tokio::test]
    async fn test_build_queries_dispatches_to_generic() {
        let context = make_query_context("Some random content", None);

        let result = build_queries(&context, &make_config()).await.unwrap();
        assert!(!result.queries.is_empty());
    }

    // =========================================================================
    // score_candidates dispatch tests
    // =========================================================================

    #[tokio::test]
    async fn test_score_candidates_dispatches_to_music() {
        let context = make_query_context(
            "Dark Side of the Moon",
            Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![],
            }),
        );
        let candidates = vec![make_candidate("Pink Floyd - Dark Side of the Moon FLAC")];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();
        assert!(!result.candidates.is_empty());
    }

    #[tokio::test]
    async fn test_score_candidates_dispatches_to_video() {
        let context = make_query_context(
            "Inception 2010",
            Some(ExpectedContent::Movie {
                title: "Inception".to_string(),
                year: Some(2010),
            }),
        );
        let candidates = vec![make_candidate("Inception 2010 1080p BluRay")];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();
        assert!(!result.candidates.is_empty());
    }

    #[tokio::test]
    async fn test_score_candidates_dispatches_to_generic() {
        let context = make_query_context("Some random content", None);
        let candidates = vec![make_candidate("Some random content")];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();
        assert!(!result.candidates.is_empty());
    }

    // =========================================================================
    // map_files dispatch tests
    // =========================================================================

    #[test]
    fn test_map_files_dispatches_to_music() {
        let context = make_query_context(
            "Dark Side of the Moon",
            Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![
                    ExpectedTrack {
                        number: 1,
                        title: "Speak to Me".to_string(),
                        duration_secs: None,
                        duration_ms: None,
                        disc_number: None,
                    },
                    ExpectedTrack {
                        number: 2,
                        title: "Breathe".to_string(),
                        duration_secs: None,
                        duration_ms: None,
                        disc_number: None,
                    },
                ],
            }),
        );
        let files = vec![
            TorrentFile {
                path: "01 - Speak to Me.flac".to_string(),
                size_bytes: 10_000_000,
            },
            TorrentFile {
                path: "02 - Breathe.flac".to_string(),
                size_bytes: 20_000_000,
            },
        ];

        let mappings = map_files(&context, &files);
        // Should attempt to map files to tracks
        assert!(!mappings.is_empty());
    }

    #[test]
    fn test_map_files_dispatches_to_video() {
        let context = make_query_context(
            "Inception",
            Some(ExpectedContent::Movie {
                title: "Inception".to_string(),
                year: Some(2010),
            }),
        );
        let files = vec![TorrentFile {
            path: "Inception.2010.1080p.BluRay.mkv".to_string(),
            size_bytes: 5_000_000_000,
        }];

        let mappings = map_files(&context, &files);
        // Video content should map the main video file
        assert!(!mappings.is_empty());
    }

    #[test]
    fn test_map_files_with_no_expected_returns_empty() {
        let context = make_query_context("test", None);

        let files: Vec<TorrentFile> = vec![];
        let mappings = map_files(&context, &files);
        assert!(mappings.is_empty());
    }

    // =========================================================================
    // post_process dispatch tests
    // =========================================================================

    #[tokio::test]
    async fn test_post_process_dispatches_to_music() {
        let ticket = make_ticket(Some(ExpectedContent::Album {
            artist: Some("Pink Floyd".to_string()),
            title: "Dark Side of the Moon".to_string(),
            tracks: vec![],
        }));

        // Currently returns empty (stub), but should dispatch to music handler
        let result = post_process(&ticket, Path::new("/tmp")).await.unwrap();
        assert!(result.cover_art_path.is_none()); // Stub returns empty
    }

    #[tokio::test]
    async fn test_post_process_dispatches_to_video() {
        let ticket = make_ticket(Some(ExpectedContent::Movie {
            title: "Inception".to_string(),
            year: Some(2010),
        }));

        // Currently returns empty (stub), but should dispatch to video handler
        let result = post_process(&ticket, Path::new("/tmp")).await.unwrap();
        assert!(result.subtitle_paths.is_empty()); // Stub returns empty
    }

    #[tokio::test]
    async fn test_post_process_dispatches_to_generic() {
        let ticket = make_ticket(None);

        let result = post_process(&ticket, Path::new("/tmp")).await.unwrap();
        assert!(result.cover_art_path.is_none());
        assert!(result.subtitle_paths.is_empty());
    }

    // =========================================================================
    // Discography fallback tests
    // =========================================================================

    #[test]
    fn test_build_fallback_queries_returns_discography_queries_for_album() {
        let context = make_query_context(
            "Dark Side of the Moon by Pink Floyd",
            Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![],
            }),
        );

        let queries = build_fallback_queries(&context);
        assert!(!queries.is_empty());
        // Should contain discography-specific queries
        assert!(queries.iter().any(|q| q.contains("discography")));
        assert!(queries
            .iter()
            .any(|q| q.to_lowercase().contains("pink floyd")));
    }

    #[test]
    fn test_build_fallback_queries_returns_empty_for_video() {
        let context = make_query_context(
            "Inception 2010",
            Some(ExpectedContent::Movie {
                title: "Inception".to_string(),
                year: Some(2010),
            }),
        );

        let queries = build_fallback_queries(&context);
        assert!(queries.is_empty()); // No fallback for video content
    }

    #[test]
    fn test_build_fallback_queries_returns_empty_for_album_without_artist() {
        let context = make_query_context(
            "Some Album",
            Some(ExpectedContent::Album {
                artist: None, // No artist - can't search for discography
                title: "Some Album".to_string(),
                tracks: vec![],
            }),
        );

        let queries = build_fallback_queries(&context);
        assert!(queries.is_empty());
    }

    #[test]
    fn test_is_discography_candidate_detects_discography_keyword() {
        let context = make_query_context(
            "Dark Side of the Moon",
            Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![],
            }),
        );

        let discography_candidate = make_candidate("Pink Floyd - Discography (1967-2014) FLAC");
        assert!(is_discography_candidate(&context, &discography_candidate));

        let album_candidate = make_candidate("Pink Floyd - Dark Side of the Moon FLAC");
        assert!(!is_discography_candidate(&context, &album_candidate));
    }

    #[test]
    fn test_is_discography_candidate_detects_collection_keyword() {
        let context = make_query_context(
            "Dark Side of the Moon",
            Some(ExpectedContent::Album {
                artist: Some("Pink Floyd".to_string()),
                title: "Dark Side of the Moon".to_string(),
                tracks: vec![],
            }),
        );

        let collection_candidate = make_candidate("Pink Floyd - Complete Collection FLAC");
        assert!(is_discography_candidate(&context, &collection_candidate));
    }
}
