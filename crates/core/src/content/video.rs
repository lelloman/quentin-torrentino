//! Video content handling (Movie, TvEpisode).
//!
//! Provides video-specific implementations for:
//! - Query building: "{title} {year}", "S01E01", resolution/source tags
//! - Scoring: resolution, source quality, codec, red flags
//! - File mapping: delegates to DumbFileMapper (handles video well)
//! - Post-processing: subtitle detection

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

/// Build queries for video content.
///
/// Generates video-specific query patterns based on ExpectedContent:
/// - Movies: "{title} {year}", "{title} {year} 1080p", "{title} BluRay"
/// - TV: "{series} S01E01", "{series} season 1", "{series} complete"
pub async fn build_queries(
    context: &QueryContext,
    config: &TextBrainConfig,
) -> Result<QueryBuildResult, TextBrainError> {
    let queries = match &context.expected {
        Some(ExpectedContent::Movie { title, year }) => build_movie_queries(title, *year),
        Some(ExpectedContent::TvEpisode {
            series,
            season,
            episodes,
        }) => build_tv_queries(series, *season, episodes),
        _ => {
            // Fall back to generic for unexpected content types
            return generic::build_queries(context, config).await;
        }
    };

    if queries.is_empty() {
        return Err(TextBrainError::NoQueriesGenerated);
    }

    let confidence = estimate_query_confidence(context);

    Ok(QueryBuildResult {
        queries,
        method: "video".to_string(),
        confidence,
        llm_usage: None,
    })
}

/// Build queries for a movie.
fn build_movie_queries(title: &str, year: Option<u32>) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let title_clean = clean_movie_title(title);

    // With year (most specific)
    if let Some(y) = year {
        // Quality-specific queries
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {} 2160p", title_clean, y),
        );
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {} 1080p BluRay", title_clean, y),
        );
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {} 1080p", title_clean, y),
        );
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {} BluRay", title_clean, y),
        );
        add_query(&mut queries, &mut seen, format!("{} {}", title_clean, y));
    }

    // Without year (fallback)
    add_query(
        &mut queries,
        &mut seen,
        format!("{} 1080p BluRay", title_clean),
    );
    add_query(&mut queries, &mut seen, format!("{} 1080p", title_clean));
    add_query(&mut queries, &mut seen, title_clean);

    queries
}

/// Build queries for TV episodes.
fn build_tv_queries(series: &str, season: u32, episodes: &[u32]) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let series_clean = clean_series_title(series);

    // Specific episode queries
    if episodes.len() == 1 {
        let ep = episodes[0];
        // S01E01 format (most common)
        add_query(
            &mut queries,
            &mut seen,
            format!("{} S{:02}E{:02}", series_clean, season, ep),
        );
        add_query(
            &mut queries,
            &mut seen,
            format!("{} S{:02}E{:02} 1080p", series_clean, season, ep),
        );
        // Alternative formats
        add_query(
            &mut queries,
            &mut seen,
            format!("{} {}x{:02}", series_clean, season, ep),
        );
    } else if !episodes.is_empty() {
        // Multiple episodes - search for season pack or range
        let min_ep = episodes.iter().min().unwrap_or(&1);
        let max_ep = episodes.iter().max().unwrap_or(&1);

        if *max_ep - *min_ep + 1 == episodes.len() as u32 {
            // Consecutive episodes - might be looking for season pack
            add_query(
                &mut queries,
                &mut seen,
                format!("{} S{:02} complete", series_clean, season),
            );
            add_query(
                &mut queries,
                &mut seen,
                format!("{} season {}", series_clean, season),
            );
            add_query(
                &mut queries,
                &mut seen,
                format!("{} S{:02}", series_clean, season),
            );
        }

        // Also search for individual episodes
        for ep in episodes.iter().take(3) {
            // Limit to avoid too many queries
            add_query(
                &mut queries,
                &mut seen,
                format!("{} S{:02}E{:02}", series_clean, season, ep),
            );
        }
    }

    // Generic series queries
    add_query(
        &mut queries,
        &mut seen,
        format!("{} complete series", series_clean),
    );
    add_query(&mut queries, &mut seen, series_clean);

    queries
}

/// Add query if not already seen.
fn add_query(queries: &mut Vec<String>, seen: &mut HashSet<String>, query: String) {
    let normalized = query.to_lowercase();
    if !normalized.is_empty() && seen.insert(normalized) {
        queries.push(query);
    }
}

/// Clean movie title for search.
fn clean_movie_title(title: &str) -> String {
    let mut result = title.to_string();

    // Remove common suffixes
    let remove_patterns = [
        "(Extended Edition)",
        "(Extended Cut)",
        "(Director's Cut)",
        "(Theatrical Cut)",
        "(Unrated)",
        "(Remastered)",
        "[Remastered]",
        "(Special Edition)",
    ];

    for pattern in remove_patterns {
        result = result.replace(pattern, "");
    }

    normalize_text(&result)
}

/// Clean series title for search.
fn clean_series_title(series: &str) -> String {
    let mut result = series.to_string();

    // Remove year in parentheses often used to disambiguate series
    // e.g., "Doctor Who (2005)" -> "Doctor Who"
    if let Some(idx) = result.rfind(" (") {
        if result[idx..].starts_with(" (19") || result[idx..].starts_with(" (20") {
            result = result[..idx].to_string();
        }
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
        Some(ExpectedContent::Movie { title, year }) => {
            if year.is_some() {
                confidence += 0.2;
            }
            if title.len() > 5 {
                confidence += 0.1;
            }
        }
        Some(ExpectedContent::TvEpisode {
            series, episodes, ..
        }) => {
            if series.len() > 5 {
                confidence += 0.1;
            }
            if !episodes.is_empty() {
                confidence += 0.2;
            }
        }
        _ => {}
    }

    confidence.min(0.9)
}

// =============================================================================
// Candidate Scoring
// =============================================================================

/// Score candidates for video content.
///
/// Uses video-specific heuristics:
/// - Resolution: 2160p > 1080p > 720p > SD
/// - Source: Remux > BluRay > WEB-DL > HDTV
/// - Codec: x265/HEVC > x264
/// - Red flags: CAM, TS, wrong year/season
pub async fn score_candidates(
    context: &QueryContext,
    candidates: &[TorrentCandidate],
    _config: &TextBrainConfig,
) -> Result<MatchResult, TextBrainError> {
    let scorer = VideoScorer::new(context);

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
        method: "video".to_string(),
        llm_usage: None,
    })
}

/// Video-specific candidate scorer.
struct VideoScorer<'a> {
    context: &'a QueryContext,
    file_mapper: DumbFileMapper,
    expected_title: Option<&'a str>,
    expected_year: Option<u32>,
    expected_series: Option<&'a str>,
    expected_season: Option<u32>,
    expected_episodes: Vec<u32>,
}

impl<'a> VideoScorer<'a> {
    fn new(context: &'a QueryContext) -> Self {
        let (expected_title, expected_year, expected_series, expected_season, expected_episodes) =
            match &context.expected {
                Some(ExpectedContent::Movie { title, year }) => {
                    (Some(title.as_str()), *year, None, None, vec![])
                }
                Some(ExpectedContent::TvEpisode {
                    series,
                    season,
                    episodes,
                }) => (
                    None,
                    None,
                    Some(series.as_str()),
                    Some(*season),
                    episodes.clone(),
                ),
                _ => (None, None, None, None, vec![]),
            };

        Self {
            context,
            file_mapper: DumbFileMapper::new(),
            expected_title,
            expected_year,
            expected_series,
            expected_season,
            expected_episodes,
        }
    }

    fn score_candidate(&self, candidate: &TorrentCandidate) -> ScoredCandidate {
        let title_lower = candidate.title.to_lowercase();

        // Component scores
        let title_score = self.title_match_score(&title_lower);
        let resolution_score = self.resolution_score(&title_lower);
        let source_score = self.source_score(&title_lower);
        let codec_score = self.codec_score(&title_lower);
        let health_score = self.health_score(candidate);
        let red_flag_penalty = self.red_flag_penalty(&title_lower);

        // File mapping score (if files available)
        let (file_mappings, mapping_score) = self.file_mapping_score(candidate);

        // Weighted combination
        let base_score = (title_score * 0.35)
            + (resolution_score * 0.20)
            + (source_score * 0.15)
            + (codec_score * 0.05)
            + (health_score * 0.10)
            - red_flag_penalty;

        // If we have file mappings, factor them in
        let final_score = if mapping_score > 0.0 {
            (base_score * 0.6) + (mapping_score * 0.4)
        } else {
            base_score
        };

        let reasoning = self.generate_reasoning(
            &title_lower,
            title_score,
            resolution_score,
            source_score,
            health_score,
            red_flag_penalty,
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
        // Movie matching
        if let Some(expected_title) = self.expected_title {
            let expected_lower = expected_title.to_lowercase();
            let mut score: f32 = 0.0;

            // Full title match
            if title.contains(&expected_lower) {
                score += 0.7;
            } else {
                // Partial match on significant words
                let words: Vec<&str> = expected_lower
                    .split_whitespace()
                    .filter(|w| w.len() > 2 && !is_stop_word(w))
                    .collect();
                if !words.is_empty() {
                    let matches = words.iter().filter(|w| title.contains(*w)).count();
                    score += (matches as f32 / words.len() as f32) * 0.5;
                }
            }

            // Year match bonus
            if let Some(year) = self.expected_year {
                if title.contains(&year.to_string()) {
                    score += 0.3;
                }
            }

            return score.min(1.0);
        }

        // TV series matching
        if let Some(series) = self.expected_series {
            let series_lower = series.to_lowercase();
            let mut score: f32 = 0.0;

            // Series name match
            if title.contains(&series_lower) {
                score += 0.5;
            } else {
                let words: Vec<&str> = series_lower
                    .split_whitespace()
                    .filter(|w| w.len() > 2 && !is_stop_word(w))
                    .collect();
                if !words.is_empty() {
                    let matches = words.iter().filter(|w| title.contains(*w)).count();
                    score += (matches as f32 / words.len() as f32) * 0.4;
                }
            }

            // Season/episode match
            if let Some(season) = self.expected_season {
                let season_pattern = format!("s{:02}", season);
                if title.contains(&season_pattern) {
                    score += 0.2;

                    // Check specific episodes
                    for ep in &self.expected_episodes {
                        let ep_pattern = format!("s{:02}e{:02}", season, ep);
                        if title.contains(&ep_pattern) {
                            score += 0.15;
                            break; // Only count once
                        }
                    }
                }
            }

            return score.min(1.0);
        }

        0.5 // No expected content
    }

    /// Score video resolution.
    fn resolution_score(&self, title: &str) -> f32 {
        // Check for resolution indicators (higher is better)
        if title.contains("2160p") || title.contains("4k") || title.contains("uhd") {
            return 1.0;
        }
        if title.contains("1080p") {
            return 0.85;
        }
        if title.contains("1080i") {
            return 0.75;
        }
        if title.contains("720p") {
            return 0.6;
        }
        if title.contains("480p") || title.contains("dvdrip") {
            return 0.4;
        }

        // No resolution info - neutral
        0.5
    }

    /// Score video source quality.
    fn source_score(&self, title: &str) -> f32 {
        // Remux is best (untouched from disc)
        if title.contains("remux") {
            return 1.0;
        }

        // BluRay sources
        if title.contains("bluray") || title.contains("blu-ray") || title.contains("bdrip") {
            return 0.9;
        }

        // Web sources (good quality)
        if title.contains("web-dl") || title.contains("webdl") {
            return 0.8;
        }
        if title.contains("webrip") || title.contains("web-rip") {
            return 0.75;
        }
        if title.contains("amzn") || title.contains("nf") || title.contains("dsnp") {
            return 0.75; // Streaming service indicators
        }

        // HDTV (broadcast)
        if title.contains("hdtv") {
            return 0.6;
        }

        // DVD
        if title.contains("dvd") && !title.contains("dvdscr") {
            return 0.5;
        }

        // No source info
        0.5
    }

    /// Score video codec.
    fn codec_score(&self, title: &str) -> f32 {
        // x265/HEVC is more efficient
        if title.contains("x265") || title.contains("hevc") || title.contains("h265") {
            return 1.0;
        }

        // x264 is standard
        if title.contains("x264") || title.contains("h264") || title.contains("avc") {
            return 0.8;
        }

        // AV1 (newer, very efficient)
        if title.contains("av1") {
            return 0.95;
        }

        // Older codecs
        if title.contains("xvid") || title.contains("divx") {
            return 0.4;
        }

        0.6 // No codec info
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

        // CAM/TS releases (terrible quality)
        if title.contains("cam")
            || title.contains("hdcam")
            || title.contains("telesync")
            || title.contains(" ts ")
            || title.contains(".ts.")
        {
            penalty += 0.7;
        }

        // Screener releases
        if title.contains("screener") || title.contains("dvdscr") || title.contains("scr") {
            penalty += 0.5;
        }

        // Wrong year for movies
        if let Some(expected_year) = self.expected_year {
            // Check if a different year is prominently featured
            for year in (1990..=2030).rev() {
                if year != expected_year && title.contains(&year.to_string()) {
                    // Different year found - could be wrong movie
                    penalty += 0.3;
                    break;
                }
            }
        }

        // Wrong season for TV
        if let Some(expected_season) = self.expected_season {
            // Check for wrong season indicator
            for s in 1..=30 {
                if s != expected_season {
                    let wrong_pattern = format!("s{:02}e", s);
                    if title.contains(&wrong_pattern) {
                        penalty += 0.4;
                        break;
                    }
                }
            }
        }

        // Hardcoded subtitles (often low quality)
        if title.contains("hc ") || title.contains("[hc]") || title.contains("hardcoded") {
            penalty += 0.15;
        }

        // Sample files
        if title.contains("sample") {
            penalty += 0.6;
        }

        penalty.min(0.9) // Don't completely eliminate
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

        if mappings.is_empty() {
            return (Vec::new(), 0.0);
        }

        let expected_count = expected.expected_file_count();
        let coverage = if expected_count > 0 {
            (mappings.len() as f32 / expected_count as f32).min(1.0)
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
        title: &str,
        title_score: f32,
        resolution_score: f32,
        _source_score: f32,
        health_score: f32,
        penalty: f32,
        candidate: &TorrentCandidate,
    ) -> String {
        let mut parts = Vec::new();

        // Title match
        if title_score >= 0.8 {
            parts.push("excellent match".to_string());
        } else if title_score >= 0.6 {
            parts.push("good match".to_string());
        } else if title_score >= 0.4 {
            parts.push("partial match".to_string());
        } else {
            parts.push("weak match".to_string());
        }

        // Resolution
        if title.contains("2160p") || title.contains("4k") {
            parts.push("4K".to_string());
        } else if title.contains("1080p") {
            parts.push("1080p".to_string());
        } else if title.contains("720p") {
            parts.push("720p".to_string());
        } else if resolution_score < 0.5 {
            parts.push("low res".to_string());
        }

        // Source
        if title.contains("remux") {
            parts.push("Remux".to_string());
        } else if title.contains("bluray") || title.contains("blu-ray") {
            parts.push("BluRay".to_string());
        } else if title.contains("web-dl") || title.contains("webdl") {
            parts.push("WEB-DL".to_string());
        } else if title.contains("webrip") {
            parts.push("WEBRip".to_string());
        } else if title.contains("hdtv") {
            parts.push("HDTV".to_string());
        }

        // Codec
        if title.contains("x265") || title.contains("hevc") {
            parts.push("HEVC".to_string());
        }

        // Health
        if health_score >= 0.8 {
            parts.push(format!("{} seeders", candidate.seeders));
        } else if candidate.seeders == 0 {
            parts.push("dead (0 seeders)".to_string());
        } else {
            parts.push(format!("low seeders ({})", candidate.seeders));
        }

        // Red flags
        if penalty > 0.3 {
            if title.contains("cam") || title.contains("hdcam") || title.contains("telesync") {
                parts.push("CAM quality".to_string());
            }
            if title.contains("screener") || title.contains("scr") {
                parts.push("screener".to_string());
            }
            if title.contains("sample") {
                parts.push("sample".to_string());
            }
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

/// Map files for video content.
///
/// Uses the standard file mapper which handles video well.
pub fn map_files(context: &QueryContext, files: &[TorrentFile]) -> Vec<FileMapping> {
    generic::map_files(context, files)
}

// =============================================================================
// Post-Processing
// =============================================================================

/// Subtitle file extensions to check.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "sub", "idx", "vtt"];

/// Common subtitle filename patterns.
const SUBTITLE_LANG_PATTERNS: &[&str] = &[
    ".en.", ".eng.", ".english.", ".en-", ".eng-", ".en_", ".eng_",
];

/// Post-process video content.
///
/// Checks for existing subtitles in the download directory.
pub async fn post_process(
    ticket: &Ticket,
    download_path: &Path,
) -> Result<PostProcessResult, ContentError> {
    // Find existing subtitles
    let subtitle_paths = find_existing_subtitles(download_path).await;

    if !subtitle_paths.is_empty() {
        return Ok(PostProcessResult {
            cover_art_path: None,
            subtitle_paths,
            metadata: None,
            warnings: vec![],
        });
    }

    // TODO: Fetch from OpenSubtitles if configured
    // This would require:
    // 1. OpenSubtitles API integration
    // 2. Movie/series identification
    // 3. Subtitle download and extraction

    let _ = ticket; // Acknowledge unused for now
    Ok(PostProcessResult::empty())
}

/// Find existing subtitle files in download directory.
async fn find_existing_subtitles(download_path: &Path) -> Vec<std::path::PathBuf> {
    let mut subtitles = Vec::new();

    // Recursive search for subtitle files
    if let Ok(mut walker) = async_walkdir(download_path).await {
        while let Some(entry) = walker.next().await {
            if let Some(path) = entry {
                if is_subtitle_file(&path) {
                    subtitles.push(path);
                }
            }
        }
    }

    // Sort by preference (English first, then by name)
    subtitles.sort_by(|a, b| {
        let a_is_english = is_english_subtitle(a);
        let b_is_english = is_english_subtitle(b);

        match (a_is_english, b_is_english) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.cmp(b),
        }
    });

    subtitles
}

/// Simple async directory walker.
async fn async_walkdir(
    path: &Path,
) -> Result<AsyncWalker, std::io::Error> {
    Ok(AsyncWalker {
        stack: vec![path.to_path_buf()],
        current_entries: None,
    })
}

/// Simple async directory walker implementation.
struct AsyncWalker {
    stack: Vec<std::path::PathBuf>,
    current_entries: Option<tokio::fs::ReadDir>,
}

impl AsyncWalker {
    async fn next(&mut self) -> Option<Option<std::path::PathBuf>> {
        loop {
            // Try to get next entry from current directory
            if let Some(ref mut entries) = self.current_entries {
                match entries.next_entry().await {
                    Ok(Some(entry)) => {
                        let path = entry.path();
                        if let Ok(file_type) = entry.file_type().await {
                            if file_type.is_dir() {
                                self.stack.push(path);
                            } else if file_type.is_file() {
                                return Some(Some(path));
                            }
                        }
                        continue;
                    }
                    Ok(None) => {
                        self.current_entries = None;
                    }
                    Err(_) => {
                        self.current_entries = None;
                    }
                }
            }

            // Move to next directory in stack
            if let Some(dir) = self.stack.pop() {
                if let Ok(entries) = tokio::fs::read_dir(&dir).await {
                    self.current_entries = Some(entries);
                    continue;
                }
            } else {
                return None; // Done
            }
        }
    }
}

/// Check if a file is a subtitle file.
fn is_subtitle_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        return SUBTITLE_EXTENSIONS.contains(&ext_lower.as_str());
    }
    false
}

/// Check if a subtitle file is likely English.
fn is_english_subtitle(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    SUBTITLE_LANG_PATTERNS
        .iter()
        .any(|pattern| path_str.contains(pattern))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::TorrentSource;
    use crate::textbrain::TextBrainMode;

    fn make_config() -> TextBrainConfig {
        TextBrainConfig {
            mode: TextBrainMode::DumbOnly,
            ..Default::default()
        }
    }

    fn make_movie_context(title: &str, year: Option<u32>) -> QueryContext {
        QueryContext {
            tags: vec!["movie".to_string()],
            description: format!("{} {}", title, year.map_or(String::new(), |y| y.to_string())),
            expected: Some(ExpectedContent::Movie {
                title: title.to_string(),
                year,
            }),
        }
    }

    fn make_tv_context(series: &str, season: u32, episodes: Vec<u32>) -> QueryContext {
        QueryContext {
            tags: vec!["tv".to_string()],
            description: format!("{} S{:02}", series, season),
            expected: Some(ExpectedContent::TvEpisode {
                series: series.to_string(),
                season,
                episodes,
            }),
        }
    }

    fn make_candidate(title: &str, seeders: u32) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: "abc123".to_string(),
            size_bytes: 5_000_000_000,
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
    async fn test_build_movie_queries() {
        let context = make_movie_context("The Matrix", Some(1999));

        let result = build_queries(&context, &make_config()).await.unwrap();

        assert_eq!(result.method, "video");
        assert!(!result.queries.is_empty());

        // Should contain title + year
        assert!(result.queries.iter().any(|q| q.contains("Matrix") && q.contains("1999")));

        // Should contain quality queries
        assert!(result.queries.iter().any(|q| q.contains("1080p")));
    }

    #[tokio::test]
    async fn test_build_tv_queries_single_episode() {
        let context = make_tv_context("Breaking Bad", 1, vec![1]);

        let result = build_queries(&context, &make_config()).await.unwrap();

        assert_eq!(result.method, "video");
        assert!(!result.queries.is_empty());

        // Should contain S01E01 format
        assert!(result.queries.iter().any(|q| q.contains("S01E01")));
    }

    #[tokio::test]
    async fn test_build_tv_queries_season_pack() {
        let context = make_tv_context("Breaking Bad", 1, vec![1, 2, 3, 4, 5]);

        let result = build_queries(&context, &make_config()).await.unwrap();

        // Should contain season pack query
        assert!(result.queries.iter().any(|q| q.contains("S01") && q.contains("complete")));
    }

    #[tokio::test]
    async fn test_clean_movie_title() {
        assert_eq!(
            clean_movie_title("Blade Runner (Director's Cut)"),
            "Blade Runner"
        );
        assert_eq!(
            clean_movie_title("The Godfather (Remastered)"),
            "The Godfather"
        );
    }

    #[tokio::test]
    async fn test_clean_series_title() {
        assert_eq!(clean_series_title("Doctor Who (2005)"), "Doctor Who");
        assert_eq!(clean_series_title("Breaking Bad"), "Breaking Bad");
    }

    // =========================================================================
    // Scoring Tests
    // =========================================================================

    #[tokio::test]
    async fn test_score_candidates_prefers_higher_resolution() {
        let context = make_movie_context("Inception", Some(2010));

        let candidates = vec![
            make_candidate("Inception 2010 720p BluRay x264", 100),
            make_candidate("Inception 2010 1080p BluRay x264", 50),
            make_candidate("Inception 2010 2160p UHD BluRay x265", 30),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // 4K should score highest despite fewer seeders
        assert!(result.candidates[0].candidate.title.contains("2160p"));
    }

    #[tokio::test]
    async fn test_score_candidates_prefers_bluray_over_hdtv() {
        let context = make_movie_context("The Dark Knight", Some(2008));

        let candidates = vec![
            make_candidate("The Dark Knight 2008 1080p HDTV", 100),
            make_candidate("The Dark Knight 2008 1080p BluRay", 50),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // BluRay should score higher
        assert!(result.candidates[0].candidate.title.contains("BluRay"));
    }

    #[tokio::test]
    async fn test_score_candidates_penalizes_cam() {
        let context = make_movie_context("Dune", Some(2021));

        let candidates = vec![
            make_candidate("Dune 2021 720p BluRay", 20),
            make_candidate("Dune 2021 HDCAM", 200),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // BluRay should beat CAM despite fewer seeders
        assert!(result.candidates[0].candidate.title.contains("BluRay"));
    }

    #[tokio::test]
    async fn test_score_candidates_tv_matches_episode() {
        let context = make_tv_context("Breaking Bad", 1, vec![1]);

        let candidates = vec![
            make_candidate("Breaking Bad S01E01 1080p BluRay", 50),
            make_candidate("Breaking Bad S02E01 1080p BluRay", 100),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // S01E01 should score higher than S02E01
        assert!(result.candidates[0].candidate.title.contains("S01E01"));
    }

    #[tokio::test]
    async fn test_score_candidates_penalizes_wrong_year() {
        let context = make_movie_context("Spider-Man", Some(2002));

        let candidates = vec![
            make_candidate("Spider-Man 2002 1080p BluRay", 50),
            make_candidate("Spider-Man 2012 1080p BluRay", 100),
        ];

        let result = score_candidates(&context, &candidates, &make_config())
            .await
            .unwrap();

        // 2002 version should score higher
        assert!(result.candidates[0].candidate.title.contains("2002"));
    }

    // =========================================================================
    // File Mapping Tests
    // =========================================================================

    #[test]
    fn test_map_files_for_movie() {
        let context = make_movie_context("The Matrix", Some(1999));

        let files = vec![
            TorrentFile {
                path: "The.Matrix.1999.1080p.BluRay.mkv".to_string(),
                size_bytes: 15_000_000_000,
            },
            TorrentFile {
                path: "Sample/sample.mkv".to_string(),
                size_bytes: 50_000_000,
            },
        ];

        let mappings = map_files(&context, &files);

        // Should map the main movie file
        assert!(!mappings.is_empty());
        assert!(mappings[0].torrent_file_path.contains("Matrix"));
    }

    #[test]
    fn test_map_files_for_tv() {
        let context = make_tv_context("Breaking Bad", 1, vec![1, 2]);

        let files = vec![
            TorrentFile {
                path: "Breaking.Bad.S01E01.mkv".to_string(),
                size_bytes: 500_000_000,
            },
            TorrentFile {
                path: "Breaking.Bad.S01E02.mkv".to_string(),
                size_bytes: 500_000_000,
            },
        ];

        let mappings = map_files(&context, &files);

        assert_eq!(mappings.len(), 2);
    }

    // =========================================================================
    // Post-Processing Tests
    // =========================================================================

    #[test]
    fn test_is_subtitle_file() {
        assert!(is_subtitle_file(Path::new("movie.srt")));
        assert!(is_subtitle_file(Path::new("movie.en.srt")));
        assert!(is_subtitle_file(Path::new("subs/english.ass")));
        assert!(!is_subtitle_file(Path::new("movie.mkv")));
        assert!(!is_subtitle_file(Path::new("movie.mp4")));
    }

    #[test]
    fn test_is_english_subtitle() {
        assert!(is_english_subtitle(Path::new("movie.en.srt")));
        assert!(is_english_subtitle(Path::new("movie.eng.srt")));
        assert!(is_english_subtitle(Path::new("movie.english.srt")));
        assert!(!is_english_subtitle(Path::new("movie.srt")));
        assert!(!is_english_subtitle(Path::new("movie.fr.srt")));
    }

    #[tokio::test]
    async fn test_post_process_returns_empty_when_no_subtitles() {
        let now = chrono::Utc::now();
        let ticket = Ticket {
            id: "test-123".to_string(),
            query_context: make_movie_context("Test Movie", Some(2020)),
            dest_path: "/tmp/test".to_string(),
            priority: 0,
            state: crate::ticket::TicketState::Pending,
            created_at: now,
            updated_at: now,
            created_by: "test".to_string(),
            output_constraints: None,
        };

        let result = post_process(&ticket, Path::new("/tmp/nonexistent_video_dir"))
            .await
            .unwrap();

        assert!(result.subtitle_paths.is_empty());
    }
}
