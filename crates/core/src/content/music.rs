//! Music content handling (Album, Track).
//!
//! Provides music-specific implementations for:
//! - Query building: "{artist} {album}", "{artist} FLAC", etc.
//! - Scoring: track count validation, audio format, red flags
//! - File mapping: track number extraction, disc handling
//! - Post-processing: cover art detection and fetching

use std::collections::HashSet;
use std::path::Path;

use crate::searcher::{TorrentCandidate, TorrentFile};
use crate::textbrain::{
    DumbFileMapper, FileMapping, MatchResult, QueryBuildResult, ScoredCandidate, TextBrainConfig,
    TextBrainError,
};
use crate::ticket::{ExpectedContent, QueryContext, Ticket};

use super::generic;
use super::types::{ContentError, PostProcessResult};

// =============================================================================
// Query Building
// =============================================================================

/// Build queries for music content.
///
/// Generates music-specific query patterns based on ExpectedContent:
/// - Albums: "{artist} {album}", "{artist} {album} FLAC", "{album} {year}"
/// - Tracks: "{artist} {track}", "{track} {artist}"
pub async fn build_queries(
    context: &QueryContext,
    config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    let queries = match &context.expected {
        Some(ExpectedContent::Album {
            artist,
            title,
            tracks,
        }) => build_album_queries(artist.as_deref(), title, tracks.len()),
        Some(ExpectedContent::Track { artist, title }) => {
            build_track_queries(artist.as_deref(), title)
        }
        _ => {
            // Fall back to generic for unexpected content types
            return generic::build_queries(context, config).await;
        }
    };

    if queries.is_empty() {
        return Err(TextBrainError::NoQueriesGenerated);
    }

    // Estimate confidence based on available info
    let confidence = estimate_query_confidence(context);

    Ok(QueryBuildResult {
        queries,
        method: "music".to_string(),
        confidence,
        llm_usage: None,
    })
}

/// Build queries for an album.
fn build_album_queries(artist: Option<&str>, title: &str, track_count: usize) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let title_clean = clean_album_title(title);

    // Primary queries with artist
    if let Some(artist) = artist {
        let artist_clean = clean_artist_name(artist);

        // Most specific: artist + album + quality
        add_query(&mut queries, &mut seen, format!("{} {} FLAC", artist_clean, title_clean));
        add_query(&mut queries, &mut seen, format!("{} {}", artist_clean, title_clean));

        // Artist + album (reversed order for some indexers)
        add_query(&mut queries, &mut seen, format!("{} {}", title_clean, artist_clean));

        // Just artist + lossless (for discography searches)
        add_query(&mut queries, &mut seen, format!("{} FLAC", artist_clean));
        add_query(&mut queries, &mut seen, format!("{} lossless", artist_clean));
    }

    // Album title only queries
    add_query(&mut queries, &mut seen, format!("{} FLAC", title_clean));
    add_query(&mut queries, &mut seen, title_clean.clone());

    // If album has many tracks, might be looking for complete album
    if track_count > 8 {
        if let Some(artist) = artist {
            add_query(&mut queries, &mut seen, format!("{} complete album", clean_artist_name(artist)));
        }
    }

    queries
}

/// Build queries for a single track.
fn build_track_queries(artist: Option<&str>, title: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let title_clean = clean_track_title(title);

    if let Some(artist) = artist {
        let artist_clean = clean_artist_name(artist);

        // Artist + track title
        add_query(&mut queries, &mut seen, format!("{} {}", artist_clean, title_clean));

        // Track title + artist (some indexers prefer this)
        add_query(&mut queries, &mut seen, format!("{} {}", title_clean, artist_clean));

        // With quality indicator
        add_query(&mut queries, &mut seen, format!("{} {} FLAC", artist_clean, title_clean));
    }

    // Just track title
    add_query(&mut queries, &mut seen, title_clean);

    queries
}

/// Add query if not already seen.
fn add_query(queries: &mut Vec<String>, seen: &mut HashSet<String>, query: String) {
    let normalized = query.to_lowercase();
    if !normalized.is_empty() && seen.insert(normalized) {
        queries.push(query);
    }
}

/// Clean album title for search.
fn clean_album_title(title: &str) -> String {
    let mut result = title.to_string();

    // Remove common suffixes
    let remove_patterns = [
        "(Deluxe Edition)",
        "(Deluxe)",
        "(Remastered)",
        "(Remaster)",
        "[Remastered]",
        "(Anniversary Edition)",
        "(Special Edition)",
        "(Expanded Edition)",
        "(Bonus Track Version)",
    ];

    for pattern in remove_patterns {
        result = result.replace(pattern, "");
    }

    normalize_text(&result)
}

/// Clean artist name for search.
fn clean_artist_name(artist: &str) -> String {
    let mut result = artist.to_string();

    // Handle "The X" -> "X" for some searches
    // But keep original too

    // Remove featuring artists for cleaner primary search
    if let Some(idx) = result.to_lowercase().find(" feat") {
        result = result[..idx].to_string();
    }
    if let Some(idx) = result.to_lowercase().find(" ft.") {
        result = result[..idx].to_string();
    }
    if let Some(idx) = result.to_lowercase().find(" ft ") {
        result = result[..idx].to_string();
    }

    normalize_text(&result)
}

/// Clean track title for search.
fn clean_track_title(title: &str) -> String {
    let mut result = title.to_string();

    // Remove featuring info (keep for album artist)
    if let Some(idx) = result.to_lowercase().find(" (feat") {
        result = result[..idx].to_string();
    }

    normalize_text(&result)
}

/// Normalize text for search queries.
fn normalize_text(text: &str) -> String {
    text.split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Estimate confidence in generated queries.
fn estimate_query_confidence(context: &QueryContext) -> f32 {
    let mut confidence: f32 = 0.5;

    match &context.expected {
        Some(ExpectedContent::Album {
            artist,
            title,
            tracks,
        }) => {
            // Having artist info helps a lot
            if artist.is_some() {
                confidence += 0.2;
            }
            // Longer title = more specific
            if title.len() > 10 {
                confidence += 0.1;
            }
            // Having track info helps validate results
            if !tracks.is_empty() {
                confidence += 0.1;
            }
        }
        Some(ExpectedContent::Track { artist, title }) => {
            if artist.is_some() {
                confidence += 0.15;
            }
            if title.len() > 5 {
                confidence += 0.1;
            }
        }
        _ => {}
    }

    confidence.min(0.9)
}

// =============================================================================
// Candidate Scoring
// =============================================================================

/// Score candidates for music content.
///
/// Uses music-specific heuristics:
/// - Audio format quality (FLAC > lossy)
/// - Track count validation
/// - Red flags (compilation, wrong artist)
/// - File mapping quality
pub async fn score_candidates(
    context: &QueryContext,
    candidates: &[TorrentCandidate],
    _config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    let scorer = MusicScorer::new(context);

    let mut scored: Vec<ScoredCandidate> = candidates
        .iter()
        .map(|c| scorer.score_candidate(c))
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(MatchResult {
        candidates: scored,
        method: "music".to_string(),
        llm_usage: None,
    })
}

/// Music-specific candidate scorer.
struct MusicScorer<'a> {
    context: &'a QueryContext,
    file_mapper: DumbFileMapper,
    expected_artist: Option<&'a str>,
    expected_title: Option<&'a str>,
    expected_track_count: usize,
}

impl<'a> MusicScorer<'a> {
    fn new(context: &'a QueryContext) -> Self {
        let (expected_artist, expected_title, expected_track_count) = match &context.expected {
            Some(ExpectedContent::Album {
                artist,
                title,
                tracks,
            }) => (artist.as_deref(), Some(title.as_str()), tracks.len()),
            Some(ExpectedContent::Track { artist, title }) => {
                (artist.as_deref(), Some(title.as_str()), 1)
            }
            _ => (None, None, 0),
        };

        Self {
            context,
            file_mapper: DumbFileMapper::new(),
            expected_artist,
            expected_title,
            expected_track_count,
        }
    }

    fn score_candidate(&self, candidate: &TorrentCandidate) -> ScoredCandidate {
        let title_lower = candidate.title.to_lowercase();

        // Component scores
        let title_score = self.title_match_score(&title_lower);
        let format_score = self.format_score(&title_lower);
        let health_score = self.health_score(candidate);
        let red_flag_penalty = self.red_flag_penalty(&title_lower);

        // File mapping score (if files available)
        let (file_mappings, mapping_score) = self.file_mapping_score(candidate);

        // Weighted combination
        let base_score = (title_score * 0.45)
            + (format_score * 0.20)
            + (health_score * 0.10)
            - red_flag_penalty;

        // If we have file mappings, factor them in heavily
        let final_score = if mapping_score > 0.0 {
            (base_score * 0.5) + (mapping_score * 0.5)
        } else {
            base_score
        };

        let reasoning = self.generate_reasoning(
            title_score,
            format_score,
            health_score,
            red_flag_penalty,
            mapping_score,
            candidate,
        );

        ScoredCandidate {
            candidate: candidate.clone(),
            score: final_score.clamp(0.0, 1.0),
            reasoning,
            file_mappings,
        }
    }

    /// Score title match against expected content.
    fn title_match_score(&self, title: &str) -> f32 {
        let mut score = 0.0;
        let mut max_possible = 0.0;

        // Artist match
        if let Some(artist) = self.expected_artist {
            max_possible += 1.0;
            let artist_lower = artist.to_lowercase();
            if title.contains(&artist_lower) {
                score += 1.0;
            } else {
                // Check for partial match (e.g., "Beatles" in "The Beatles")
                let artist_words: Vec<&str> = artist_lower.split_whitespace().collect();
                let matches = artist_words.iter().filter(|w| title.contains(*w)).count();
                if matches > 0 {
                    score += (matches as f32 / artist_words.len() as f32) * 0.7;
                }
            }
        }

        // Album/track title match
        if let Some(expected_title) = self.expected_title {
            max_possible += 1.0;
            let expected_lower = expected_title.to_lowercase();

            if title.contains(&expected_lower) {
                score += 1.0;
            } else {
                // Check for partial match
                let title_words: Vec<&str> = expected_lower.split_whitespace().collect();
                let significant_words: Vec<&str> = title_words
                    .iter()
                    .filter(|w| w.len() > 2 && !is_stop_word(w))
                    .copied()
                    .collect();

                if !significant_words.is_empty() {
                    let matches = significant_words.iter().filter(|w| title.contains(*w)).count();
                    score += (matches as f32 / significant_words.len() as f32) * 0.8;
                }
            }
        }

        if max_possible > 0.0 {
            score / max_possible
        } else {
            0.5 // No expected content to match
        }
    }

    /// Score audio format quality.
    fn format_score(&self, title: &str) -> f32 {
        // Lossless formats (best)
        if title.contains("flac")
            || title.contains("24bit")
            || title.contains("24-bit")
            || title.contains("hi-res")
            || title.contains("dsd")
            || title.contains("alac")
        {
            return 1.0;
        }

        // High quality lossy
        if title.contains("320") || title.contains("v0") {
            return 0.7;
        }

        // Other lossy indicators
        if title.contains("mp3") || title.contains("aac") || title.contains("ogg") {
            return 0.5;
        }

        // No format info - neutral
        0.6
    }

    /// Score torrent health.
    fn health_score(&self, candidate: &TorrentCandidate) -> f32 {
        match candidate.seeders {
            0 => 0.0,
            1..=2 => 0.3,
            3..=10 => 0.6,
            11..=50 => 0.9,
            _ => 1.0,
        }
    }

    /// Calculate penalty for red flags.
    fn red_flag_penalty(&self, title: &str) -> f32 {
        let mut penalty: f32 = 0.0;

        // Compilation/VA when looking for specific artist
        if self.expected_artist.is_some()
            && (title.contains("various artist")
                || title.contains("v.a.")
                || title.contains("va -")
                || title.contains("compilation"))
        {
            penalty += 0.3;
        }

        // Sample/preview releases
        if title.contains("sample") || title.contains("preview") || title.contains("promo") {
            penalty += 0.4;
        }

        // Live recordings when studio expected (common mismatch)
        let expected_is_not_live = self
            .expected_title
            .is_none_or(|t| !t.to_lowercase().contains("live"));
        if expected_is_not_live
            && (title.contains("[live]") || title.contains("(live)") || title.contains(" live "))
        {
            penalty += 0.2;
        }

        // Tribute/cover albums
        if title.contains("tribute") || title.contains("covered by") || title.contains("karaoke") {
            penalty += 0.4;
        }

        // Wrong format indicator for music (video instead of audio)
        if title.contains("dvd") || title.contains("blu-ray") || title.contains("concert video") {
            penalty += 0.2;
        }

        penalty.min(0.8) // Don't completely eliminate
    }

    /// Calculate file mapping score.
    fn file_mapping_score(&self, candidate: &TorrentCandidate) -> (Vec<FileMapping>, f32) {
        let expected = match &self.context.expected {
            Some(e) => e,
            None => return (Vec::new(), 0.0),
        };

        let files = match &candidate.files {
            Some(f) if !f.is_empty() => f,
            _ => return (Vec::new(), 0.0),
        };

        let mappings = self.file_mapper.map_files(files, expected);

        // Score based on coverage and confidence
        if mappings.is_empty() {
            return (Vec::new(), 0.0);
        }

        let coverage = if self.expected_track_count > 0 {
            (mappings.len() as f32 / self.expected_track_count as f32).min(1.0)
        } else {
            1.0
        };

        let avg_confidence: f32 =
            mappings.iter().map(|m| m.confidence).sum::<f32>() / mappings.len() as f32;

        let score = (coverage * 0.6) + (avg_confidence * 0.4);

        (mappings, score)
    }

    /// Generate human-readable reasoning.
    fn generate_reasoning(
        &self,
        title_score: f32,
        format_score: f32,
        health_score: f32,
        penalty: f32,
        mapping_score: f32,
        candidate: &TorrentCandidate,
    ) -> String {
        let mut parts = Vec::new();

        // Title match
        if title_score >= 0.9 {
            parts.push("excellent match".to_string());
        } else if title_score >= 0.7 {
            parts.push("good match".to_string());
        } else if title_score >= 0.5 {
            parts.push("partial match".to_string());
        } else {
            parts.push("weak match".to_string());
        }

        // Format
        if format_score >= 0.9 {
            parts.push("FLAC/lossless".to_string());
        } else if format_score >= 0.6 {
            parts.push("high quality".to_string());
        } else if format_score < 0.5 {
            parts.push("lossy format".to_string());
        }

        // Health
        if health_score >= 0.8 {
            parts.push(format!("{} seeders", candidate.seeders));
        } else if candidate.seeders == 0 {
            parts.push("dead (0 seeders)".to_string());
        } else {
            parts.push(format!("low seeders ({})", candidate.seeders));
        }

        // Penalties
        if penalty > 0.2 {
            let title_lower = candidate.title.to_lowercase();
            if title_lower.contains("compilation") || title_lower.contains("various") {
                parts.push("compilation".to_string());
            }
            if title_lower.contains("live") {
                parts.push("live recording".to_string());
            }
            if title_lower.contains("tribute") {
                parts.push("tribute album".to_string());
            }
        }

        // File mapping
        if mapping_score > 0.0 {
            parts.push(format!("{:.0}% tracks mapped", mapping_score * 100.0));
        }

        parts.join(", ")
    }
}

/// Check if a word is a stop word.
fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the" | "a" | "an" | "and" | "or" | "of" | "in" | "on" | "at" | "to" | "for" | "by"
    )
}

// =============================================================================
// File Mapping
// =============================================================================

/// Map files for music content.
///
/// Uses the standard file mapper with music-aware extensions.
pub fn map_files(context: &QueryContext, files: &[TorrentFile]) -> Vec<FileMapping> {
    // Use the generic mapper which already handles music well
    generic::map_files(context, files)
}

// =============================================================================
// Post-Processing
// =============================================================================

/// Cover art file patterns to check.
const COVER_ART_PATTERNS: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "album.jpg",
    "album.jpeg",
    "album.png",
    "albumart.jpg",
    "albumart.jpeg",
    "albumart.png",
];

/// Post-process music content.
///
/// Checks for existing cover art and fetches if missing.
pub async fn post_process(
    ticket: &Ticket,
    download_path: &Path,
) -> Result<PostProcessResult, ContentError> {
    // Check for existing cover art
    if let Some(cover_path) = find_existing_cover_art(download_path).await {
        return Ok(PostProcessResult::with_cover_art(cover_path));
    }

    // TODO: Fetch from MusicBrainz Cover Art Archive
    // This would require:
    // 1. MusicBrainz ID lookup by artist/album
    // 2. Cover Art Archive API call
    // 3. Download and save cover.jpg

    // For now, return empty result if no existing cover found
    let _ = ticket; // Acknowledge unused for now
    Ok(PostProcessResult::empty())
}

/// Find existing cover art in download directory.
async fn find_existing_cover_art(download_path: &Path) -> Option<std::path::PathBuf> {
    // Check root directory
    for pattern in COVER_ART_PATTERNS {
        let path = download_path.join(pattern);
        if tokio::fs::metadata(&path).await.is_ok() {
            return Some(path);
        }

        // Also check case-insensitive
        let upper_path = download_path.join(pattern.to_uppercase());
        if tokio::fs::metadata(&upper_path).await.is_ok() {
            return Some(upper_path);
        }
    }

    // Check subdirectories (common for multi-disc albums)
    if let Ok(mut entries) = tokio::fs::read_dir(download_path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    let subdir = entry.path();
                    for pattern in COVER_ART_PATTERNS {
                        let path = subdir.join(pattern);
                        if tokio::fs::metadata(&path).await.is_ok() {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::TorrentSource;
    use crate::textbrain::TextBrainMode;
    use crate::ticket::ExpectedTrack;

    fn make_config() -> TextBrainConfig {
        TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            ..Default::default()
        }
    }

    fn make_album_context(artist: Option<&str>, title: &str, tracks: Vec<ExpectedTrack>) -> QueryContext {
        QueryContext {
            tags: vec!["music".to_string(), "flac".to_string()],
            description: format!(
                "{} {}",
                artist.unwrap_or(""),
                title
            ),
            expected: Some(ExpectedContent::Album {
                artist: artist.map(String::from),
                title: title.to_string(),
                tracks,
            }),
        }
    }

    fn make_track_context(artist: Option<&str>, title: &str) -> QueryContext {
        QueryContext {
            tags: vec!["music".to_string()],
            description: format!("{} {}", artist.unwrap_or(""), title),
            expected: Some(ExpectedContent::Track {
                artist: artist.map(String::from),
                title: title.to_string(),
            }),
        }
    }

    fn make_candidate(title: &str, seeders: u32) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: "abc123".to_string(),
            size_bytes: 500_000_000,
            seeders,
            leechers: 5,
            category: None,
            publish_date: None,
            files: None,
            sources: vec![TorrentSource {
                indexer: "test".to_string(),
                magnet_uri: Some("magnet:?xt=urn:btih:abc123".to_string()),
                torrent_url: None,
                seeders,
                leechers: 5,
                details_url: None,
            }],
            from_cache: false,
        }
    }

    // =========================================================================
    // Query Building Tests
    // =========================================================================

    #[tokio::test]
    async fn test_build_album_queries() {
        let context = make_album_context(
            Some("Pink Floyd"),
            "Dark Side of the Moon",
            vec![],
        );

        let result = build_queries(&context, &make_config()).await.unwrap();

        assert_eq!(result.method, "music");
        assert!(!result.queries.is_empty());

        // Should contain artist + album query
        assert!(result.queries.iter().any(|q| q.contains("Pink Floyd") && q.contains("Dark Side")));

        // Should contain FLAC query
        assert!(result.queries.iter().any(|q| q.contains("FLAC")));
    }

    #[tokio::test]
    async fn test_build_track_queries() {
        let context = make_track_context(Some("Beatles"), "Yesterday");

        let result = build_queries(&context, &make_config()).await.unwrap();

        assert_eq!(result.method, "music");
        assert!(!result.queries.is_empty());

        // Should contain artist + track
        assert!(result.queries.iter().any(|q| q.contains("Beatles") && q.contains("Yesterday")));
    }

    #[tokio::test]
    async fn test_clean_album_title() {
        assert_eq!(
            clean_album_title("Abbey Road (Remastered)"),
            "Abbey Road"
        );
        assert_eq!(
            clean_album_title("Rumours (Deluxe Edition)"),
            "Rumours"
        );
    }

    #[tokio::test]
    async fn test_clean_artist_name() {
        assert_eq!(
            clean_artist_name("Queen feat. David Bowie"),
            "Queen"
        );
        assert_eq!(
            clean_artist_name("Pink Floyd ft. Roger Waters"),
            "Pink Floyd"
        );
    }

    // =========================================================================
    // Scoring Tests
    // =========================================================================

    #[tokio::test]
    async fn test_score_candidates_prefers_exact_match() {
        let context = make_album_context(Some("Pink Floyd"), "Dark Side of the Moon", vec![]);

        let candidates = vec![
            make_candidate("Pink Floyd - Dark Side of the Moon [FLAC]", 50),
            make_candidate("Pink Floyd - The Wall [FLAC]", 100),
            make_candidate("Random Artist - Random Album", 200),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        assert_eq!(result.method, "music");

        // Best match should be first
        assert!(result.candidates[0].candidate.title.contains("Dark Side"));

        // Scores should be descending
        assert!(result.candidates[0].score >= result.candidates[1].score);
    }

    #[tokio::test]
    async fn test_score_candidates_prefers_flac() {
        let context = make_album_context(Some("Beatles"), "Abbey Road", vec![]);

        let candidates = vec![
            make_candidate("Beatles - Abbey Road [MP3 320]", 100),
            make_candidate("Beatles - Abbey Road [FLAC]", 50),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // FLAC should score higher despite fewer seeders
        let flac_idx = result
            .candidates
            .iter()
            .position(|c| c.candidate.title.contains("FLAC"))
            .unwrap();
        let mp3_idx = result
            .candidates
            .iter()
            .position(|c| c.candidate.title.contains("MP3"))
            .unwrap();

        assert!(
            result.candidates[flac_idx].score >= result.candidates[mp3_idx].score,
            "FLAC should score higher than MP3"
        );
    }

    #[tokio::test]
    async fn test_score_candidates_penalizes_compilations() {
        let context = make_album_context(Some("Queen"), "A Night at the Opera", vec![]);

        let candidates = vec![
            make_candidate("Queen - A Night at the Opera [FLAC]", 50),
            make_candidate("Various Artists - Queen Tribute [FLAC]", 100),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // Original should beat tribute/compilation
        assert!(result.candidates[0].candidate.title.contains("A Night at the Opera"));
    }

    #[tokio::test]
    async fn test_score_candidates_penalizes_live() {
        let context = make_album_context(Some("Led Zeppelin"), "Led Zeppelin IV", vec![]);

        let candidates = vec![
            make_candidate("Led Zeppelin - Led Zeppelin IV [FLAC]", 50),
            make_candidate("Led Zeppelin - Led Zeppelin IV [Live] [FLAC]", 100),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // Studio should beat live (when not looking for live)
        assert!(!result.candidates[0].candidate.title.contains("[Live]"));
    }

    // =========================================================================
    // File Mapping Tests
    // =========================================================================

    #[test]
    fn test_map_files_for_album() {
        let context = make_album_context(
            Some("Beatles"),
            "Abbey Road",
            vec![
                ExpectedTrack::new(1, "Come Together"),
                ExpectedTrack::new(2, "Something"),
            ],
        );

        let files = vec![
            TorrentFile {
                path: "01 - Come Together.flac".to_string(),
                size_bytes: 30_000_000,
            },
            TorrentFile {
                path: "02 - Something.flac".to_string(),
                size_bytes: 25_000_000,
            },
        ];

        let mappings = map_files(&context, &files);

        assert_eq!(mappings.len(), 2);
        assert!(mappings.iter().any(|m| m.ticket_item_id == "track-1"));
        assert!(mappings.iter().any(|m| m.ticket_item_id == "track-2"));
    }

    // =========================================================================
    // Post-Processing Tests
    // =========================================================================

    #[tokio::test]
    async fn test_post_process_returns_empty_when_no_cover() {
        let now = chrono::Utc::now();
        let ticket = Ticket {
            id: "test-123".to_string(),
            query_context: make_album_context(Some("Artist"), "Album", vec![]),
            dest_path: "/tmp/test".to_string(),
            priority: 0,
            state: crate::ticket::TicketState::Pending,
            created_at: now,
            updated_at: now,
            created_by: "test".to_string(),
            output_constraints: None,
        };

        // Use a temp dir that definitely doesn't have cover art
        let result = post_process(&ticket, Path::new("/tmp/nonexistent_music_dir"))
            .await
            .unwrap();

        assert!(result.cover_art_path.is_none());
    }
}
