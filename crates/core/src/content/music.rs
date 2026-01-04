//! Music content handling (Album, Track).
//!
//! Provides music-specific implementations for:
//! - Query building: "{artist} {album}", "{artist} FLAC", etc.
//! - Scoring: track count validation, audio format, red flags
//! - File mapping: track number extraction, disc handling
//! - Post-processing: cover art detection and fetching

use std::collections::HashSet;
use std::path::Path;

use regex_lite::Regex;

use crate::converter::AudioFormat;
use crate::searcher::{TorrentCandidate, TorrentFile};
use crate::textbrain::{
    DumbFileMapper, FileMapping, MatchResult, QueryBuildResult, ScoredCandidate, TextBrainConfig,
    TextBrainError,
};
use crate::ticket::{
    AudioSearchConstraints, CatalogReference, ExpectedContent, QueryContext, Ticket,
};

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
///
/// Also considers `search_constraints.audio` to prioritize format-specific queries.
pub async fn build_queries(
    context: &QueryContext,
    config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    // Extract audio constraints if present
    let audio_constraints = context
        .search_constraints
        .as_ref()
        .and_then(|sc| sc.audio.as_ref());

    let queries = match &context.expected {
        Some(ExpectedContent::Album {
            artist,
            title,
            tracks,
        }) => build_album_queries(artist.as_deref(), title, tracks.len(), audio_constraints),
        Some(ExpectedContent::Track { artist, title }) => {
            build_track_queries(artist.as_deref(), title, audio_constraints)
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
///
/// If audio constraints specify preferred formats, those format keywords are
/// prioritized in the query list.
fn build_album_queries(
    artist: Option<&str>,
    title: &str,
    track_count: usize,
    audio_constraints: Option<&AudioSearchConstraints>,
) -> Vec<String> {
    build_album_queries_inner(artist, title, track_count, audio_constraints, false)
}

/// Build discography/collection queries for fallback when album-specific search fails.
///
/// These queries target artist discographies, complete collections, and compilations
/// that might contain the desired album.
pub fn build_discography_queries(
    artist: Option<&str>,
    audio_constraints: Option<&AudioSearchConstraints>,
) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let Some(artist) = artist else {
        return queries; // Can't search for discography without artist
    };

    let artist_clean = clean_artist_name(artist);
    let format_keywords = get_format_keywords(audio_constraints);

    // Discography-specific queries (highest priority)
    for keyword in &format_keywords {
        add_query(
            &mut queries,
            &mut seen,
            format!("{} discography {}", artist_clean, keyword),
        );
    }
    add_query(
        &mut queries,
        &mut seen,
        format!("{} discography", artist_clean),
    );

    // Complete collection variations
    for keyword in &format_keywords {
        add_query(
            &mut queries,
            &mut seen,
            format!("{} complete {}", artist_clean, keyword),
        );
    }
    add_query(
        &mut queries,
        &mut seen,
        format!("{} complete discography", artist_clean),
    );
    add_query(
        &mut queries,
        &mut seen,
        format!("{} complete collection", artist_clean),
    );
    add_query(
        &mut queries,
        &mut seen,
        format!("{} collection", artist_clean),
    );

    // All albums / studio albums
    add_query(
        &mut queries,
        &mut seen,
        format!("{} all albums", artist_clean),
    );
    add_query(
        &mut queries,
        &mut seen,
        format!("{} studio albums", artist_clean),
    );

    // Anthology / box set variations
    add_query(
        &mut queries,
        &mut seen,
        format!("{} anthology", artist_clean),
    );
    add_query(
        &mut queries,
        &mut seen,
        format!("{} box set", artist_clean),
    );

    // Artist name with just format (broad search - might catch discographies)
    for keyword in &format_keywords {
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}", artist_clean, keyword),
        );
    }

    queries
}

/// Inner implementation for album queries with discography mode flag.
fn build_album_queries_inner(
    artist: Option<&str>,
    title: &str,
    track_count: usize,
    audio_constraints: Option<&AudioSearchConstraints>,
    _discography_mode: bool,
) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let title_clean = clean_album_title(title);

    // Determine preferred format keywords from constraints
    let format_keywords = get_format_keywords(audio_constraints);

    // Primary queries with artist
    if let Some(artist) = artist {
        let artist_clean = clean_artist_name(artist);

        // Add queries with preferred format keywords first
        for keyword in &format_keywords {
            add_query(
                &mut queries,
                &mut seen,
                format!("{} {} {}", artist_clean, title_clean, keyword),
            );
        }

        // Fallback: artist + album (no format)
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}", artist_clean, title_clean),
        );

        // Artist + album (reversed order for some indexers)
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}", title_clean, artist_clean),
        );

        // Just artist + lossless (for discography searches)
        for keyword in &format_keywords {
            add_query(
                &mut queries,
                &mut seen,
                format!("{} {}", artist_clean, keyword),
            );
        }
    }

    // Album title only queries with format keywords
    for keyword in &format_keywords {
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}", title_clean, keyword),
        );
    }
    add_query(&mut queries, &mut seen, title_clean.clone());

    // If album has many tracks, might be looking for complete album
    if track_count > 8 {
        if let Some(artist) = artist {
            add_query(
                &mut queries,
                &mut seen,
                format!("{} complete album", clean_artist_name(artist)),
            );
        }
    }

    queries
}

/// Get format keywords from audio constraints, with sensible defaults.
fn get_format_keywords(constraints: Option<&AudioSearchConstraints>) -> Vec<&'static str> {
    if let Some(c) = constraints {
        if !c.preferred_formats.is_empty() {
            return c
                .preferred_formats
                .iter()
                .filter_map(|f| match f {
                    AudioFormat::Flac => Some("FLAC"),
                    AudioFormat::Alac => Some("ALAC"),
                    AudioFormat::Aac => Some("AAC"),
                    AudioFormat::Mp3 => Some("MP3"),
                    AudioFormat::Opus => Some("OPUS"),
                    AudioFormat::OggVorbis => Some("OGG"),
                    AudioFormat::Wav => None,
                })
                .collect();
        }
    }
    // Default: prefer FLAC
    vec!["FLAC"]
}

/// Build queries for a single track.
fn build_track_queries(
    artist: Option<&str>,
    title: &str,
    audio_constraints: Option<&AudioSearchConstraints>,
) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let title_clean = clean_track_title(title);
    let format_keywords = get_format_keywords(audio_constraints);

    if let Some(artist) = artist {
        let artist_clean = clean_artist_name(artist);

        // Artist + track title
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}", artist_clean, title_clean),
        );

        // Track title + artist (some indexers prefer this)
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}", title_clean, artist_clean),
        );

        // With quality indicator
        for keyword in &format_keywords {
            add_query(
                &mut queries,
                &mut seen,
                format!("{} {} {}", artist_clean, title_clean, keyword),
            );
        }
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
    /// Audio search constraints (preferred formats, avoid live/compilations).
    audio_constraints: Option<&'a AudioSearchConstraints>,
    /// Catalog reference for validation (track count, duration).
    catalog_ref: Option<&'a CatalogReference>,
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

        let audio_constraints = context
            .search_constraints
            .as_ref()
            .and_then(|sc| sc.audio.as_ref());

        let catalog_ref = context.catalog_reference.as_ref();

        Self {
            context,
            file_mapper: DumbFileMapper::new(),
            expected_artist,
            expected_title,
            expected_track_count,
            audio_constraints,
            catalog_ref,
        }
    }

    fn score_candidate(&self, candidate: &TorrentCandidate) -> ScoredCandidate {
        let title_lower = candidate.title.to_lowercase();

        // Component scores
        let title_score = self.title_match_score(&title_lower);
        let format_score = self.format_score(&title_lower);
        let health_score = self.health_score(candidate);
        let red_flag_penalty = self.red_flag_penalty(&title_lower);

        // Constraint-based scoring adjustments
        let constraint_bonus = self.constraint_bonus(&title_lower);

        // File mapping score (if files available)
        let (file_mappings, mapping_score) = self.file_mapping_score(candidate);

        // Catalog validation (track count match if we have files)
        let catalog_bonus = self.catalog_validation_bonus(&file_mappings);

        // Weighted combination
        let base_score = (title_score * 0.40)
            + (format_score * 0.15)
            + (health_score * 0.10)
            + constraint_bonus
            + catalog_bonus
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
            constraint_bonus,
            catalog_bonus,
            candidate,
        );

        ScoredCandidate {
            candidate: candidate.clone(),
            score: final_score.clamp(0.0, 1.0),
            reasoning,
            file_mappings,
        }
    }

    /// Calculate bonus/penalty based on search constraints.
    fn constraint_bonus(&self, title: &str) -> f32 {
        let constraints = match self.audio_constraints {
            Some(c) => c,
            None => return 0.0,
        };

        let mut bonus: f32 = 0.0;

        // Check preferred formats
        if !constraints.preferred_formats.is_empty() {
            let format_match = constraints.preferred_formats.iter().any(|f| match f {
                AudioFormat::Flac => title.contains("flac"),
                AudioFormat::Alac => title.contains("alac"),
                AudioFormat::Aac => title.contains("aac"),
                AudioFormat::Mp3 => title.contains("mp3"),
                AudioFormat::Opus => title.contains("opus"),
                AudioFormat::OggVorbis => title.contains("ogg") || title.contains("vorbis"),
                _ => false,
            });
            if format_match {
                bonus += 0.10;
            }
        }

        // Check for min bitrate (only meaningful for lossy)
        if let Some(min_bitrate) = constraints.min_bitrate_kbps {
            // Try to parse bitrate from title
            if let Some(bitrate) = extract_bitrate(title) {
                if bitrate >= min_bitrate {
                    bonus += 0.05;
                } else {
                    bonus -= 0.10; // Penalty for below minimum
                }
            }
        }

        bonus
    }

    /// Calculate bonus based on catalog reference validation.
    ///
    /// If we have a MusicBrainz catalog reference with track count,
    /// we can validate the file mapping matches expected track count.
    fn catalog_validation_bonus(&self, file_mappings: &[FileMapping]) -> f32 {
        let catalog = match self.catalog_ref {
            Some(c) => c,
            None => return 0.0,
        };

        match catalog {
            CatalogReference::MusicBrainz { track_count, .. } => {
                if file_mappings.is_empty() {
                    return 0.0;
                }

                let mapped_count = file_mappings.len() as u32;
                let expected = *track_count;

                if expected == 0 {
                    return 0.0;
                }

                // Exact match is best
                if mapped_count == expected {
                    return 0.15;
                }

                // Close match (within 1-2 tracks - could be bonus tracks)
                let diff = (mapped_count as i32 - expected as i32).unsigned_abs();
                if diff <= 2 {
                    return 0.08;
                }

                // Significant mismatch is a penalty
                if diff > expected / 2 {
                    return -0.10;
                }

                0.0
            }
            CatalogReference::Tmdb { .. } => {
                // TMDB is for video, not music
                0.0
            }
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
    ///
    /// Uses audio constraints for avoid_compilations and avoid_live if present.
    fn red_flag_penalty(&self, title: &str) -> f32 {
        let mut penalty: f32 = 0.0;

        // Check if user explicitly wants to avoid compilations via constraints
        let avoid_compilations = self
            .audio_constraints
            .map(|c| c.avoid_compilations)
            .unwrap_or(false);

        // Check if user explicitly wants to avoid live recordings
        let avoid_live = self
            .audio_constraints
            .map(|c| c.avoid_live)
            .unwrap_or(false);

        // Compilation/VA detection
        let is_compilation = title.contains("various artist")
            || title.contains("v.a.")
            || title.contains("va -")
            || title.contains("compilation");

        if is_compilation {
            // Higher penalty if user explicitly wants to avoid, or if looking for specific artist
            if avoid_compilations {
                penalty += 0.5;
            } else if self.expected_artist.is_some() {
                penalty += 0.3;
            }
        }

        // Sample/preview releases
        if title.contains("sample") || title.contains("preview") || title.contains("promo") {
            penalty += 0.4;
        }

        // Live recording detection
        let is_live =
            title.contains("[live]") || title.contains("(live)") || title.contains(" live ");

        if is_live {
            // Higher penalty if user explicitly wants to avoid
            if avoid_live {
                penalty += 0.4;
            } else {
                // Only penalize if expected title doesn't contain "live"
                let expected_is_not_live = self
                    .expected_title
                    .is_none_or(|t| !t.to_lowercase().contains("live"));
                if expected_is_not_live {
                    penalty += 0.2;
                }
            }
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
    #[allow(clippy::too_many_arguments)]
    fn generate_reasoning(
        &self,
        title_score: f32,
        format_score: f32,
        health_score: f32,
        penalty: f32,
        mapping_score: f32,
        constraint_bonus: f32,
        catalog_bonus: f32,
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

        // Constraint matches
        if constraint_bonus > 0.05 {
            parts.push("matches constraints".to_string());
        } else if constraint_bonus < -0.05 {
            parts.push("below min quality".to_string());
        }

        // Catalog validation
        if catalog_bonus > 0.10 {
            parts.push("track count verified".to_string());
        } else if catalog_bonus > 0.0 {
            parts.push("track count close".to_string());
        } else if catalog_bonus < -0.05 {
            parts.push("track count mismatch".to_string());
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

/// Extract bitrate from title string (e.g., "320", "256", "192").
fn extract_bitrate(title: &str) -> Option<u32> {
    // Common bitrate patterns
    let patterns = ["320", "256", "192", "160", "128", "96", "64"];
    for pattern in patterns {
        if title.contains(pattern) {
            // Verify it's likely a bitrate (near "kbps" or "mp3")
            if title.contains("kbps")
                || title.contains("mp3")
                || title.contains("aac")
                || title.contains("cbr")
                || title.contains("vbr")
            {
                return pattern.parse().ok();
            }
        }
    }
    None
}

/// Check if a word is a stop word.
fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the" | "a" | "an" | "and" | "or" | "of" | "in" | "on" | "at" | "to" | "for" | "by"
    )
}

// =============================================================================
// Discography Detection and Scoring
// =============================================================================

/// Keywords that indicate a discography or collection torrent.
const DISCOGRAPHY_KEYWORDS: &[&str] = &[
    "discography",
    "discografia",
    "complete",
    "collection",
    "anthology",
    "box set",
    "boxset",
    "all albums",
    "studio albums",
    "complete works",
    "complete albums",
    "1965-",
    "1970-",
    "1975-",
    "1980-",
    "1985-",
    "1990-",
    "1995-",
    "2000-",
    "2005-",
    "2010-",
    "2015-",
    "2020-",
];

/// Check if a candidate appears to be a discography/collection.
///
/// Looks for keywords in the title that suggest this is a multi-album release
/// rather than a single album.
pub fn is_discography_candidate(artist: Option<&str>, candidate: &TorrentCandidate) -> bool {
    let title_lower = candidate.title.to_lowercase();

    // Check for discography keywords
    for keyword in DISCOGRAPHY_KEYWORDS {
        if title_lower.contains(keyword) {
            return true;
        }
    }

    // Check for year ranges (e.g., "1970-2010", "1985-2023")
    if let Ok(re) = Regex::new(r"\b(19|20)\d{2}\s*[-–]\s*(19|20)\d{2}\b") {
        if re.is_match(&title_lower) {
            return true;
        }
    }

    // Check for large size suggesting multiple albums (>5GB often indicates discography)
    if candidate.size_bytes > 5_000_000_000 {
        // Also verify artist name is present to avoid false positives
        if let Some(artist) = artist {
            let artist_lower = artist.to_lowercase();
            if title_lower.contains(&artist_lower) {
                // Large torrent with artist name but no album name = likely discography
                return true;
            }
        }
    }

    false
}

/// Score a discography candidate for containing the target album.
///
/// This differs from regular scoring:
/// - Does NOT penalize for being a compilation/collection
/// - Validates that the target album is present in the file listing
/// - Scores based on whether we can find the expected tracks in a subdirectory
pub async fn score_discography_candidate(
    context: &QueryContext,
    candidate: &TorrentCandidate,
    _config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    let scorer = DiscographyScorer::new(context);
    let scored = scorer.score_candidate(candidate);

    Ok(MatchResult {
        candidates: vec![scored],
        method: "music_discography".to_string(),
        llm_usage: None,
    })
}

/// Discography-specific candidate scorer.
///
/// Unlike the regular MusicScorer, this:
/// - Expects multiple albums in the file listing
/// - Looks for the target album as a subdirectory or path prefix
/// - Doesn't penalize for compilation/collection keywords
struct DiscographyScorer<'a> {
    expected_artist: Option<&'a str>,
    expected_album_title: Option<&'a str>,
    expected_track_count: usize,
    expected_tracks: Vec<String>,
    audio_constraints: Option<&'a AudioSearchConstraints>,
}

impl<'a> DiscographyScorer<'a> {
    fn new(context: &'a QueryContext) -> Self {
        let (expected_artist, expected_album_title, expected_track_count, expected_tracks) =
            match &context.expected {
                Some(ExpectedContent::Album {
                    artist,
                    title,
                    tracks,
                }) => (
                    artist.as_deref(),
                    Some(title.as_str()),
                    tracks.len(),
                    tracks.iter().map(|t| t.title.clone()).collect(),
                ),
                Some(ExpectedContent::Track { artist, title }) => {
                    (artist.as_deref(), Some(title.as_str()), 1, vec![title.clone()])
                }
                _ => (None, None, 0, vec![]),
            };

        let audio_constraints = context
            .search_constraints
            .as_ref()
            .and_then(|sc| sc.audio.as_ref());

        Self {
            expected_artist,
            expected_album_title,
            expected_track_count,
            expected_tracks,
            audio_constraints,
        }
    }

    fn score_candidate(&self, candidate: &TorrentCandidate) -> ScoredCandidate {
        let title_lower = candidate.title.to_lowercase();

        // Base scores
        let artist_score = self.artist_match_score(&title_lower);
        let format_score = self.format_score(&title_lower);
        let health_score = self.health_score(candidate);

        // Check if target album is present in files
        let (album_found, album_score, file_mappings) = self.check_album_in_files(candidate);

        // Constraint bonus
        let constraint_bonus = self.constraint_bonus(&title_lower);

        // Calculate final score
        let base_score = if album_found {
            // Album found in discography - high confidence
            (artist_score * 0.20)
                + (format_score * 0.15)
                + (health_score * 0.10)
                + (album_score * 0.45)  // Heavy weight on finding the album
                + constraint_bonus
        } else if candidate.files.is_some() {
            // Files available but album not found - low score
            (artist_score * 0.30)
                + (format_score * 0.10)
                + (health_score * 0.10)
                + constraint_bonus
                - 0.3  // Penalty for not finding album
        } else {
            // No file info - moderate score based on title alone
            (artist_score * 0.40)
                + (format_score * 0.15)
                + (health_score * 0.10)
                + constraint_bonus
                + 0.15  // Slight bonus for being a discography (might contain album)
        };

        let reasoning = self.generate_reasoning(
            artist_score,
            format_score,
            health_score,
            album_found,
            album_score,
            candidate,
        );

        ScoredCandidate {
            candidate: candidate.clone(),
            score: base_score.clamp(0.0, 1.0),
            reasoning,
            file_mappings,
        }
    }

    /// Check if the target album is present in the file listing.
    ///
    /// Discographies typically have structure like:
    /// - `{Artist} - {Album}/01 - Track.flac`
    /// - `{Year} - {Album}/01 - Track.flac`
    /// - `{Album} ({Year})/01 - Track.flac`
    fn check_album_in_files(
        &self,
        candidate: &TorrentCandidate,
    ) -> (bool, f32, Vec<FileMapping>) {
        let files = match &candidate.files {
            Some(f) if !f.is_empty() => f,
            _ => return (false, 0.0, vec![]),
        };

        let album_title = match self.expected_album_title {
            Some(t) => t,
            None => return (false, 0.0, vec![]),
        };

        let album_clean = clean_album_title(album_title).to_lowercase();
        let album_words: Vec<&str> = album_clean.split_whitespace().collect();

        // Look for album in directory structure
        let mut album_files: Vec<&TorrentFile> = vec![];
        let mut album_dir_found = false;

        for file in files {
            let path_lower = file.path.to_lowercase();

            // Check if this file is in a directory matching the album name
            // Look for significant words from album title in path
            let significant_words: Vec<&str> = album_words
                .iter()
                .filter(|w| w.len() > 2 && !is_stop_word(w))
                .copied()
                .collect();

            if significant_words.is_empty() {
                // Short album title - require exact match
                if path_lower.contains(&album_clean) {
                    album_files.push(file);
                    album_dir_found = true;
                }
            } else {
                // Check if most significant words appear in path
                let matches = significant_words
                    .iter()
                    .filter(|w| path_lower.contains(*w))
                    .count();

                let match_ratio = matches as f32 / significant_words.len() as f32;

                if match_ratio >= 0.7 {
                    album_files.push(file);
                    album_dir_found = true;
                }
            }
        }

        if !album_dir_found {
            return (false, 0.0, vec![]);
        }

        // Count audio files in matched directory
        let audio_extensions = ["flac", "mp3", "ogg", "opus", "m4a", "aac", "wav", "alac"];
        let audio_files: Vec<&TorrentFile> = album_files
            .iter()
            .filter(|f| {
                let path_lower = f.path.to_lowercase();
                audio_extensions.iter().any(|ext| path_lower.ends_with(ext))
            })
            .copied()
            .collect();

        if audio_files.is_empty() {
            return (false, 0.0, vec![]);
        }

        // Score based on track count match
        let track_count_score = if self.expected_track_count > 0 {
            let ratio = audio_files.len() as f32 / self.expected_track_count as f32;
            if (0.8..=1.2).contains(&ratio) {
                1.0  // Track count roughly matches
            } else if (0.5..=1.5).contains(&ratio) {
                0.7  // Somewhat close
            } else {
                0.4  // Mismatch but album found
            }
        } else {
            0.8  // No expected track count, but album found
        };

        // Try to map tracks if we have expected track names
        let file_mappings = if !self.expected_tracks.is_empty() {
            self.map_album_tracks(&audio_files)
        } else {
            vec![]
        };

        (true, track_count_score, file_mappings)
    }

    /// Map audio files to expected tracks.
    fn map_album_tracks(&self, files: &[&TorrentFile]) -> Vec<FileMapping> {
        let mut mappings = vec![];

        for (idx, track_name) in self.expected_tracks.iter().enumerate() {
            let track_lower = track_name.to_lowercase();
            let track_words: Vec<&str> = track_lower
                .split_whitespace()
                .filter(|w| w.len() > 2 && !is_stop_word(w))
                .collect();

            // Find best matching file
            let mut best_match: Option<(&TorrentFile, f32)> = None;

            for file in files {
                let filename = file
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&file.path)
                    .to_lowercase();

                // Score by matching words
                if track_words.is_empty() {
                    if filename.contains(&track_lower) {
                        best_match = Some((file, 1.0));
                        break;
                    }
                } else {
                    let matches = track_words.iter().filter(|w| filename.contains(*w)).count();
                    let score = matches as f32 / track_words.len() as f32;
                    if score > best_match.map(|(_, s)| s).unwrap_or(0.0) {
                        best_match = Some((file, score));
                    }
                }
            }

            if let Some((file, confidence)) = best_match {
                if confidence >= 0.5 {
                    mappings.push(FileMapping {
                        ticket_item_id: format!("track-{}", idx + 1),
                        torrent_file_path: file.path.clone(),
                        confidence,
                    });
                }
            }
        }

        mappings
    }

    fn artist_match_score(&self, title: &str) -> f32 {
        if let Some(artist) = self.expected_artist {
            let artist_lower = artist.to_lowercase();
            if title.contains(&artist_lower) {
                return 1.0;
            }
            // Partial match
            let words: Vec<&str> = artist_lower.split_whitespace().collect();
            let matches = words.iter().filter(|w| title.contains(*w)).count();
            if matches > 0 {
                return (matches as f32 / words.len() as f32) * 0.8;
            }
        }
        0.3  // No artist to match
    }

    fn format_score(&self, title: &str) -> f32 {
        if title.contains("flac") || title.contains("24bit") || title.contains("hi-res") {
            1.0
        } else if title.contains("320") || title.contains("v0") {
            0.7
        } else if title.contains("mp3") || title.contains("aac") {
            0.5
        } else {
            0.6
        }
    }

    fn health_score(&self, candidate: &TorrentCandidate) -> f32 {
        match candidate.seeders {
            0 => 0.0,
            1..=2 => 0.3,
            3..=10 => 0.6,
            11..=50 => 0.9,
            _ => 1.0,
        }
    }

    fn constraint_bonus(&self, title: &str) -> f32 {
        let constraints = match self.audio_constraints {
            Some(c) => c,
            None => return 0.0,
        };

        let mut bonus: f32 = 0.0;

        if !constraints.preferred_formats.is_empty() {
            let format_match = constraints.preferred_formats.iter().any(|f| match f {
                AudioFormat::Flac => title.contains("flac"),
                AudioFormat::Alac => title.contains("alac"),
                AudioFormat::Aac => title.contains("aac"),
                AudioFormat::Mp3 => title.contains("mp3"),
                AudioFormat::Opus => title.contains("opus"),
                AudioFormat::OggVorbis => title.contains("ogg") || title.contains("vorbis"),
                _ => false,
            });
            if format_match {
                bonus += 0.10;
            }
        }

        bonus
    }

    fn generate_reasoning(
        &self,
        artist_score: f32,
        format_score: f32,
        health_score: f32,
        album_found: bool,
        album_score: f32,
        candidate: &TorrentCandidate,
    ) -> String {
        let mut parts = vec![];

        // Discography indicator
        parts.push("discography".to_string());

        // Artist match
        if artist_score >= 0.9 {
            parts.push("artist match".to_string());
        } else if artist_score >= 0.5 {
            parts.push("partial artist match".to_string());
        }

        // Album found status
        if album_found {
            if album_score >= 0.9 {
                parts.push("album found with matching tracks".to_string());
            } else {
                parts.push("album found".to_string());
            }
        } else if candidate.files.is_some() {
            parts.push("album NOT found in files".to_string());
        } else {
            parts.push("no file listing".to_string());
        }

        // Format
        if format_score >= 0.9 {
            parts.push("FLAC/lossless".to_string());
        } else if format_score >= 0.6 {
            parts.push("high quality".to_string());
        }

        // Health
        if health_score >= 0.8 {
            parts.push(format!("{} seeders", candidate.seeders));
        } else if candidate.seeders == 0 {
            parts.push("no seeders".to_string());
        }

        parts.join(", ")
    }
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
            catalog_reference: None,
            search_constraints: None,
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
            catalog_reference: None,
            search_constraints: None,
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
            retry_count: 0,
        };

        // Use a temp dir that definitely doesn't have cover art
        let result = post_process(&ticket, Path::new("/tmp/nonexistent_music_dir"))
            .await
            .unwrap();

        assert!(result.cover_art_path.is_none());
    }

    // =========================================================================
    // Discography Query Building Tests
    // =========================================================================

    #[test]
    fn test_build_discography_queries_with_artist() {
        let queries = build_discography_queries(Some("Pink Floyd"), None);

        assert!(!queries.is_empty());
        // Should contain discography-specific queries
        assert!(queries.iter().any(|q| q.contains("discography")));
        assert!(queries.iter().any(|q| q.contains("complete")));
        assert!(queries.iter().any(|q| q.contains("collection")));
        // All should contain artist name
        assert!(queries.iter().all(|q| q.to_lowercase().contains("pink floyd")));
    }

    #[test]
    fn test_build_discography_queries_without_artist() {
        let queries = build_discography_queries(None, None);
        assert!(queries.is_empty()); // Can't search for discography without artist
    }

    #[test]
    fn test_build_discography_queries_with_format_constraints() {
        let constraints = AudioSearchConstraints {
            preferred_formats: vec![AudioFormat::Flac],
            min_bitrate_kbps: None,
            avoid_compilations: false,
            avoid_live: false,
        };

        let queries = build_discography_queries(Some("Beatles"), Some(&constraints));

        // Should contain format-specific discography queries
        assert!(queries.iter().any(|q| q.contains("FLAC") && q.contains("discography")));
    }

    // =========================================================================
    // Discography Detection Tests
    // =========================================================================

    #[test]
    fn test_is_discography_candidate_keyword_detection() {
        // Discography keyword
        let candidate = make_candidate("Pink Floyd - Discography (1967-2014) FLAC", 50);
        assert!(is_discography_candidate(Some("Pink Floyd"), &candidate));

        // Complete collection
        let candidate = make_candidate("Beatles - Complete Studio Albums FLAC", 50);
        assert!(is_discography_candidate(Some("Beatles"), &candidate));

        // Anthology
        let candidate = make_candidate("Led Zeppelin - Anthology Box Set", 50);
        assert!(is_discography_candidate(Some("Led Zeppelin"), &candidate));
    }

    #[test]
    fn test_is_discography_candidate_year_range_detection() {
        let candidate = make_candidate("Pink Floyd 1967-2014 FLAC", 50);
        assert!(is_discography_candidate(Some("Pink Floyd"), &candidate));

        let candidate = make_candidate("Radiohead [1993-2016] Complete", 50);
        assert!(is_discography_candidate(Some("Radiohead"), &candidate));
    }

    #[test]
    fn test_is_discography_candidate_rejects_regular_album() {
        let candidate = make_candidate("Pink Floyd - Dark Side of the Moon FLAC", 50);
        assert!(!is_discography_candidate(Some("Pink Floyd"), &candidate));

        let candidate = make_candidate("Beatles - Abbey Road [2019 Remaster]", 50);
        assert!(!is_discography_candidate(Some("Beatles"), &candidate));
    }

    #[test]
    fn test_is_discography_candidate_large_size_heuristic() {
        // Large torrent with artist name but no album - likely discography
        let mut candidate = make_candidate("Pink Floyd FLAC Collection", 50);
        candidate.size_bytes = 10_000_000_000; // 10 GB
        assert!(is_discography_candidate(Some("Pink Floyd"), &candidate));

        // Small torrent - not a discography
        let mut candidate = make_candidate("Pink Floyd FLAC", 50);
        candidate.size_bytes = 500_000_000; // 500 MB
        assert!(!is_discography_candidate(Some("Pink Floyd"), &candidate));
    }

    // =========================================================================
    // Discography Scoring Tests
    // =========================================================================

    fn make_candidate_with_files(title: &str, seeders: u32, files: Vec<TorrentFile>) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: "abc123".to_string(),
            size_bytes: 10_000_000_000,
            seeders,
            leechers: 5,
            category: None,
            publish_date: None,
            files: Some(files),
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

    #[tokio::test]
    async fn test_score_discography_candidate_finds_album_in_files() {
        let context = make_album_context(
            Some("Pink Floyd"),
            "Dark Side of the Moon",
            vec![
                ExpectedTrack::new(1, "Speak to Me"),
                ExpectedTrack::new(2, "Breathe"),
            ],
        );

        // Discography with the target album as a subdirectory
        let files = vec![
            TorrentFile {
                path: "Pink Floyd/1973 - Dark Side of the Moon/01 - Speak to Me.flac".to_string(),
                size_bytes: 30_000_000,
            },
            TorrentFile {
                path: "Pink Floyd/1973 - Dark Side of the Moon/02 - Breathe.flac".to_string(),
                size_bytes: 35_000_000,
            },
            TorrentFile {
                path: "Pink Floyd/1979 - The Wall/01 - In the Flesh.flac".to_string(),
                size_bytes: 25_000_000,
            },
        ];

        let candidate = make_candidate_with_files(
            "Pink Floyd - Discography (1967-2014) FLAC",
            50,
            files,
        );

        let result = score_discography_candidate(&context, &candidate, &make_config())
            .await
            .unwrap();

        assert_eq!(result.method, "music_discography");
        assert!(!result.candidates.is_empty());

        let scored = &result.candidates[0];
        // Score should be reasonably high since album was found
        assert!(scored.score > 0.5, "Score should be > 0.5, got {}", scored.score);
        assert!(scored.reasoning.contains("album found"));
    }

    #[tokio::test]
    async fn test_score_discography_candidate_penalizes_missing_album() {
        let context = make_album_context(
            Some("Pink Floyd"),
            "Dark Side of the Moon",
            vec![],
        );

        // Discography that does NOT contain the target album
        let files = vec![
            TorrentFile {
                path: "Pink Floyd/1979 - The Wall/01 - In the Flesh.flac".to_string(),
                size_bytes: 25_000_000,
            },
            TorrentFile {
                path: "Pink Floyd/1975 - Wish You Were Here/01 - Shine On.flac".to_string(),
                size_bytes: 50_000_000,
            },
        ];

        let candidate = make_candidate_with_files(
            "Pink Floyd - Discography (1975-1994) FLAC",
            50,
            files,
        );

        let result = score_discography_candidate(&context, &candidate, &make_config())
            .await
            .unwrap();

        let scored = &result.candidates[0];
        // Score should be low since target album not found
        assert!(scored.score < 0.5, "Score should be < 0.5 when album not found, got {}", scored.score);
        assert!(scored.reasoning.contains("NOT found"));
    }

    #[tokio::test]
    async fn test_score_discography_candidate_without_files() {
        let context = make_album_context(
            Some("Pink Floyd"),
            "Dark Side of the Moon",
            vec![],
        );

        // Discography without file listing (no enrichment available)
        let candidate = make_candidate("Pink Floyd - Discography (1967-2014) FLAC", 50);

        let result = score_discography_candidate(&context, &candidate, &make_config())
            .await
            .unwrap();

        let scored = &result.candidates[0];
        // Moderate score - could contain the album, but we can't verify
        assert!(scored.score > 0.3 && scored.score < 0.8);
        assert!(scored.reasoning.contains("no file listing"));
    }

    #[tokio::test]
    async fn test_score_discography_candidate_maps_tracks() {
        let context = make_album_context(
            Some("Pink Floyd"),
            "Dark Side of the Moon",
            vec![
                ExpectedTrack::new(1, "Speak to Me"),
                ExpectedTrack::new(2, "Breathe"),
                ExpectedTrack::new(3, "On the Run"),
            ],
        );

        let files = vec![
            TorrentFile {
                path: "Pink Floyd/Dark Side of the Moon/01 - Speak to Me.flac".to_string(),
                size_bytes: 30_000_000,
            },
            TorrentFile {
                path: "Pink Floyd/Dark Side of the Moon/02 - Breathe.flac".to_string(),
                size_bytes: 35_000_000,
            },
            TorrentFile {
                path: "Pink Floyd/Dark Side of the Moon/03 - On the Run.flac".to_string(),
                size_bytes: 28_000_000,
            },
        ];

        let candidate = make_candidate_with_files(
            "Pink Floyd - Complete Discography FLAC",
            50,
            files,
        );

        let result = score_discography_candidate(&context, &candidate, &make_config())
            .await
            .unwrap();

        let scored = &result.candidates[0];
        // Should have file mappings for the tracks
        assert!(!scored.file_mappings.is_empty(), "Should have file mappings");
        assert!(scored.file_mappings.iter().any(|m| m.ticket_item_id == "track-1"));
    }
}
