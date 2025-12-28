//! Dumb (heuristic-based) candidate matcher implementation.
//!
//! Scores torrent candidates using fuzzy matching and heuristics.
//! No LLM required - works entirely offline.

use async_trait::async_trait;
use std::collections::HashSet;

use crate::searcher::TorrentCandidate;
use crate::ticket::QueryContext;
use crate::textbrain::file_mapper::{DumbFileMapper, calculate_mapping_quality};
use crate::textbrain::traits::{CandidateMatcher, TextBrainError};
use crate::textbrain::types::{FileMapping, MatchResult, ScoredCandidate};

/// Configuration for the dumb matcher.
#[derive(Debug, Clone)]
pub struct DumbMatcherConfig {
    /// Weight for title similarity (0.0-1.0).
    pub title_weight: f32,
    /// Weight for quality tag matching (0.0-1.0).
    pub quality_weight: f32,
    /// Weight for torrent health (seeders) (0.0-1.0).
    pub health_weight: f32,
    /// Weight for size heuristics (0.0-1.0).
    pub size_weight: f32,
    /// Minimum seeders to consider a torrent healthy.
    pub min_seeders: u32,
    /// Ideal seeder count (diminishing returns above this).
    pub ideal_seeders: u32,
    /// Minimum size in bytes to not be suspicious.
    pub min_size_bytes: u64,
    /// Maximum size in bytes before penalty (per category).
    pub max_size_bytes: u64,
}

impl Default for DumbMatcherConfig {
    fn default() -> Self {
        Self {
            // Title match is the primary factor - content relevance matters most
            title_weight: 0.75,
            quality_weight: 0.15,
            // Health (seeders) as a tiebreaker
            health_weight: 0.10,
            // Size weight disabled - too crude without category-aware thresholds
            // (50GB is small for a TV series collection, huge for a single album)
            size_weight: 0.0,
            min_seeders: 1,
            ideal_seeders: 20,
            min_size_bytes: 1024 * 1024,        // 1 MB (unused when weight is 0)
            max_size_bytes: 50 * 1024 * 1024 * 1024, // 50 GB (unused when weight is 0)
        }
    }
}

/// Heuristic-based candidate matcher.
///
/// Scores candidates by:
/// 1. Title similarity to description keywords
/// 2. Quality tag presence (flac, 1080p, etc.)
/// 3. Torrent health (seeder count)
/// 4. Size reasonableness
/// 5. File mapping quality (when expected content is specified)
pub struct DumbMatcher {
    config: DumbMatcherConfig,
    file_mapper: DumbFileMapper,
}

impl DumbMatcher {
    /// Create a new dumb matcher with default config.
    pub fn new() -> Self {
        Self {
            config: DumbMatcherConfig::default(),
            file_mapper: DumbFileMapper::new(),
        }
    }

    /// Create a new dumb matcher with custom config.
    pub fn with_config(config: DumbMatcherConfig) -> Self {
        Self {
            config,
            file_mapper: DumbFileMapper::new(),
        }
    }

    /// Extract keywords from text for matching.
    fn extract_keywords(text: &str) -> HashSet<String> {
        let stop_words: HashSet<&str> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "as", "is", "was", "are", "were", "been", "be", "have", "has", "had",
        ]
        .into_iter()
        .collect();

        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() > 1)
            .filter(|s| !stop_words.contains(s.as_str()))
            .collect()
    }

    /// Calculate title similarity score (0.0-1.0).
    fn title_similarity(&self, title: &str, context: &QueryContext) -> f32 {
        let title_keywords = Self::extract_keywords(title);
        let desc_keywords = Self::extract_keywords(&context.description);

        if desc_keywords.is_empty() {
            return 0.5; // No description to match against
        }

        // Count how many description keywords appear in title
        let matches = desc_keywords
            .iter()
            .filter(|kw| title_keywords.contains(*kw))
            .count();

        // Check for partial matches (substring)
        let partial_matches = desc_keywords
            .iter()
            .filter(|kw| {
                !title_keywords.contains(*kw)
                    && title_keywords.iter().any(|tk| tk.contains(kw.as_str()) || kw.contains(tk.as_str()))
            })
            .count();

        // Check for fuzzy matches (spelling variations like Rachmaninov/Rahmaninov)
        let fuzzy_matches = desc_keywords
            .iter()
            .filter(|kw| {
                !title_keywords.contains(*kw)
                    && !title_keywords.iter().any(|tk| tk.contains(kw.as_str()) || kw.contains(tk.as_str()))
                    && title_keywords.iter().any(|tk| Self::is_fuzzy_match(kw, tk))
            })
            .count();

        let total_score = matches as f32 + (partial_matches as f32 * 0.5) + (fuzzy_matches as f32 * 0.8);
        let max_score = desc_keywords.len() as f32;

        (total_score / max_score).min(1.0)
    }

    /// Check if two strings are fuzzy matches (small edit distance).
    /// Useful for spelling variations like Rachmaninov/Rahmaninov.
    fn is_fuzzy_match(a: &str, b: &str) -> bool {
        // Only check words of similar length (within 2 chars)
        let len_diff = (a.len() as i32 - b.len() as i32).abs();
        if len_diff > 2 {
            return false;
        }

        // Only check words that are at least 4 chars (avoid false positives on short words)
        if a.len() < 4 || b.len() < 4 {
            return false;
        }

        // Calculate Levenshtein distance
        let distance = Self::levenshtein_distance(a, b);

        // Allow 1-2 character differences for longer words
        let threshold = if a.len() >= 8 { 2 } else { 1 };
        distance <= threshold
    }

    /// Calculate Levenshtein edit distance between two strings.
    fn levenshtein_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

        for i in 0..=a_len {
            matrix[i][0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[a_len][b_len]
    }

    /// Calculate quality tag match score (0.0-1.0).
    fn quality_match(&self, title: &str, context: &QueryContext) -> f32 {
        let quality_tags: Vec<&str> = context
            .tags
            .iter()
            .filter(|t| self.is_quality_tag(t))
            .map(|s| s.as_str())
            .collect();

        if quality_tags.is_empty() {
            return 0.5; // No quality requirements
        }

        let title_lower = title.to_lowercase();
        let matches = quality_tags
            .iter()
            .filter(|tag| title_lower.contains(&tag.to_lowercase()))
            .count();

        matches as f32 / quality_tags.len() as f32
    }

    /// Check if a tag is quality-related.
    fn is_quality_tag(&self, tag: &str) -> bool {
        let quality_patterns = [
            // Audio formats
            "flac", "mp3", "aac", "opus", "wav", "alac", "dsd", "ape",
            "320", "v0", "v2", "lossless",
            // Video resolution
            "1080p", "720p", "2160p", "4k", "uhd", "hdr", "hdr10", "dolby",
            // Video codec
            "x264", "x265", "hevc", "h264", "h265", "av1", "xvid",
            // Source
            "bluray", "blu-ray", "remux", "web-dl", "webrip", "hdtv", "dvdrip",
            // Audio for video
            "dts", "atmos", "truehd", "dd5.1", "aac5.1",
        ];

        let lower = tag.to_lowercase();
        quality_patterns.iter().any(|p| lower.contains(p))
    }

    /// Calculate health score based on seeders (0.0-1.0).
    fn health_score(&self, candidate: &TorrentCandidate) -> f32 {
        if candidate.seeders < self.config.min_seeders {
            return 0.0; // Dead torrent
        }

        // Logarithmic scaling - diminishing returns above ideal
        let seeders = candidate.seeders as f32;
        let ideal = self.config.ideal_seeders as f32;

        if seeders >= ideal {
            1.0
        } else {
            // Linear scale from min to ideal
            let min = self.config.min_seeders as f32;
            (seeders - min) / (ideal - min)
        }
    }

    /// Calculate size score (0.0-1.0).
    /// Penalizes suspiciously small or large torrents.
    fn size_score(&self, candidate: &TorrentCandidate) -> f32 {
        let size = candidate.size_bytes;

        if size < self.config.min_size_bytes {
            // Too small - likely fake or incomplete
            return 0.2;
        }

        if size > self.config.max_size_bytes {
            // Very large - slight penalty but not disqualifying
            return 0.7;
        }

        // Sweet spot
        1.0
    }

    /// Generate reasoning for the score.
    fn generate_reasoning(
        &self,
        title_score: f32,
        quality_score: f32,
        health_score: f32,
        _size_score: f32,
        candidate: &TorrentCandidate,
    ) -> String {
        let mut parts: Vec<String> = Vec::new();

        // Title match
        if title_score >= 0.8 {
            parts.push("strong title match".to_string());
        } else if title_score >= 0.5 {
            parts.push("partial title match".to_string());
        } else if title_score < 0.3 {
            parts.push("weak title match".to_string());
        }

        // Quality
        if quality_score >= 0.8 {
            parts.push("quality tags present".to_string());
        } else if quality_score < 0.5 && quality_score > 0.0 {
            parts.push("missing some quality tags".to_string());
        }

        // Health
        if health_score >= 0.8 {
            parts.push(format!("well-seeded ({})", candidate.seeders));
        } else if health_score < 0.3 {
            parts.push("low seeders".to_string());
        }

        if parts.is_empty() {
            "average match".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Score a single candidate.
    fn score_candidate(&self, candidate: &TorrentCandidate, context: &QueryContext) -> ScoredCandidate {
        let title_score = self.title_similarity(&candidate.title, context);
        let quality_score = self.quality_match(&candidate.title, context);
        let health_score = self.health_score(candidate);
        let size_score = self.size_score(candidate);

        // Calculate file mapping if expected content and files are available
        let (file_mappings, mapping_quality) = self.calculate_file_mappings(candidate, context);

        // Base weighted score
        let base_score = (title_score * self.config.title_weight)
            + (quality_score * self.config.quality_weight)
            + (health_score * self.config.health_weight)
            + (size_score * self.config.size_weight);

        // If we have expected content and files, factor in mapping quality
        let weighted_score = if mapping_quality > 0.0 {
            // Blend base score with mapping quality (mapping is important!)
            base_score * 0.6 + mapping_quality * 0.4
        } else {
            base_score
        };

        let mut reasoning = self.generate_reasoning(
            title_score,
            quality_score,
            health_score,
            size_score,
            candidate,
        );

        // Add file mapping info to reasoning
        if !file_mappings.is_empty() {
            reasoning.push_str(&format!(
                ", {} file(s) mapped ({:.0}% quality)",
                file_mappings.len(),
                mapping_quality * 100.0
            ));
        } else if context.expected.is_some() && candidate.files.is_some() {
            reasoning.push_str(", no files matched expected content");
        }

        ScoredCandidate {
            candidate: candidate.clone(),
            score: weighted_score,
            reasoning,
            file_mappings,
        }
    }

    /// Calculate file mappings for a candidate.
    ///
    /// Returns (mappings, quality_score).
    fn calculate_file_mappings(
        &self,
        candidate: &TorrentCandidate,
        context: &QueryContext,
    ) -> (Vec<FileMapping>, f32) {
        // Need both expected content and files to map
        let expected = match &context.expected {
            Some(e) => e,
            None => return (Vec::new(), 0.0),
        };

        let files = match &candidate.files {
            Some(f) if !f.is_empty() => f,
            _ => return (Vec::new(), 0.0),
        };

        let mappings = self.file_mapper.map_files(files, expected);
        let quality = calculate_mapping_quality(&mappings, expected);

        (mappings, quality)
    }
}

impl Default for DumbMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CandidateMatcher for DumbMatcher {
    fn name(&self) -> &str {
        "dumb"
    }

    async fn score_candidates(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        let mut scored: Vec<ScoredCandidate> = candidates
            .iter()
            .map(|c| self.score_candidate(c, context))
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(MatchResult {
            candidates: scored,
            method: "dumb".to_string(),
            llm_usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::TorrentSource;

    fn make_candidate(title: &str, seeders: u32, size_bytes: u64) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: "abc123".to_string(),
            size_bytes,
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

    fn make_context(tags: &[&str], description: &str) -> QueryContext {
        QueryContext::new(
            tags.iter().map(|s| s.to_string()).collect(),
            description,
        )
    }

    #[test]
    fn test_extract_keywords() {
        let keywords = DumbMatcher::extract_keywords("Abbey Road by The Beatles");
        assert!(keywords.contains("abbey"));
        assert!(keywords.contains("road"));
        assert!(keywords.contains("beatles"));
        assert!(!keywords.contains("the")); // stop word
        assert!(!keywords.contains("by")); // stop word
    }

    #[test]
    fn test_title_similarity_exact_match() {
        let matcher = DumbMatcher::new();
        let context = make_context(&[], "Abbey Road Beatles");

        let score = matcher.title_similarity("Abbey Road - The Beatles (2019 Remaster) [FLAC]", &context);
        assert!(score >= 0.9, "Expected high score for exact match, got {}", score);
    }

    #[test]
    fn test_title_similarity_partial_match() {
        let matcher = DumbMatcher::new();
        let context = make_context(&[], "Abbey Road Beatles");

        let score = matcher.title_similarity("Beatles Greatest Hits", &context);
        assert!(score > 0.0 && score < 0.8, "Expected partial score, got {}", score);
    }

    #[test]
    fn test_title_similarity_no_match() {
        let matcher = DumbMatcher::new();
        let context = make_context(&[], "Abbey Road Beatles");

        let score = matcher.title_similarity("Pink Floyd Dark Side of the Moon", &context);
        assert!(score < 0.3, "Expected low score for no match, got {}", score);
    }

    #[test]
    fn test_quality_match_all_present() {
        let matcher = DumbMatcher::new();
        let context = make_context(&["music", "flac", "1080p"], "test");

        let score = matcher.quality_match("Concert Video [FLAC Audio] [1080p]", &context);
        assert!(score >= 0.9, "Expected high score when all quality tags present, got {}", score);
    }

    #[test]
    fn test_quality_match_partial() {
        let matcher = DumbMatcher::new();
        let context = make_context(&["flac", "x265"], "test"); // both are quality tags

        let score = matcher.quality_match("Album [FLAC]", &context);
        assert!(score == 0.5, "Expected 50% for 1/2 tags, got {}", score);
    }

    #[test]
    fn test_quality_match_no_quality_tags() {
        let matcher = DumbMatcher::new();
        let context = make_context(&["music", "album"], "test"); // non-quality tags

        let score = matcher.quality_match("Some Album", &context);
        assert_eq!(score, 0.5, "Expected neutral score when no quality requirements");
    }

    #[test]
    fn test_health_score_dead_torrent() {
        let matcher = DumbMatcher::new();
        let candidate = make_candidate("test", 0, 1024 * 1024 * 100);

        let score = matcher.health_score(&candidate);
        assert_eq!(score, 0.0, "Dead torrent should score 0");
    }

    #[test]
    fn test_health_score_well_seeded() {
        let matcher = DumbMatcher::new();
        let candidate = make_candidate("test", 50, 1024 * 1024 * 100);

        let score = matcher.health_score(&candidate);
        assert_eq!(score, 1.0, "Well-seeded torrent should score 1.0");
    }

    #[test]
    fn test_health_score_moderate() {
        let matcher = DumbMatcher::new();
        let candidate = make_candidate("test", 10, 1024 * 1024 * 100);

        let score = matcher.health_score(&candidate);
        assert!(score > 0.3 && score < 1.0, "Moderate seeders should give moderate score, got {}", score);
    }

    #[test]
    fn test_size_score_too_small() {
        let matcher = DumbMatcher::new();
        let candidate = make_candidate("test", 10, 1024); // 1 KB - too small

        let score = matcher.size_score(&candidate);
        assert_eq!(score, 0.2, "Suspiciously small torrent should be penalized");
    }

    #[test]
    fn test_size_score_normal() {
        let matcher = DumbMatcher::new();
        let candidate = make_candidate("test", 10, 500 * 1024 * 1024); // 500 MB

        let score = matcher.size_score(&candidate);
        assert_eq!(score, 1.0, "Normal size should score 1.0");
    }

    #[test]
    fn test_size_score_very_large() {
        let matcher = DumbMatcher::new();
        let candidate = make_candidate("test", 10, 100 * 1024 * 1024 * 1024); // 100 GB

        let score = matcher.size_score(&candidate);
        assert_eq!(score, 0.7, "Very large torrent should have slight penalty");
    }

    #[test]
    fn test_score_candidate_overall() {
        let matcher = DumbMatcher::new();
        let context = make_context(&["flac"], "Abbey Road Beatles");
        let candidate = make_candidate(
            "Abbey Road - The Beatles [FLAC]",
            25,
            500 * 1024 * 1024,
        );

        let scored = matcher.score_candidate(&candidate, &context);
        assert!(scored.score >= 0.8, "Good match should score high, got {}", scored.score);
        assert!(!scored.reasoning.is_empty());
    }

    #[tokio::test]
    async fn test_score_candidates_sorted() {
        let matcher = DumbMatcher::new();
        let context = make_context(&[], "Abbey Road Beatles");

        let candidates = vec![
            make_candidate("Pink Floyd - Dark Side", 100, 500 * 1024 * 1024),
            make_candidate("Abbey Road - Beatles [FLAC]", 50, 500 * 1024 * 1024),
            make_candidate("Beatles - Help!", 30, 500 * 1024 * 1024),
        ];

        let result = matcher.score_candidates(&context, &candidates).await.unwrap();

        assert_eq!(result.candidates.len(), 3);
        assert_eq!(result.method, "dumb");
        // Best match should be first
        assert!(result.candidates[0].candidate.title.contains("Abbey Road"));
        // Scores should be descending
        assert!(result.candidates[0].score >= result.candidates[1].score);
        assert!(result.candidates[1].score >= result.candidates[2].score);
    }

    #[tokio::test]
    async fn test_score_candidates_empty() {
        let matcher = DumbMatcher::new();
        let context = make_context(&[], "test");

        let result = matcher.score_candidates(&context, &[]).await.unwrap();
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_is_quality_tag() {
        let matcher = DumbMatcher::new();

        assert!(matcher.is_quality_tag("flac"));
        assert!(matcher.is_quality_tag("FLAC"));
        assert!(matcher.is_quality_tag("1080p"));
        assert!(matcher.is_quality_tag("x265"));
        assert!(matcher.is_quality_tag("bluray"));

        assert!(!matcher.is_quality_tag("music"));
        assert!(!matcher.is_quality_tag("album"));
        assert!(!matcher.is_quality_tag("movie"));
    }

    #[test]
    fn test_matcher_name() {
        let matcher = DumbMatcher::new();
        assert_eq!(matcher.name(), "dumb");
    }

    #[test]
    fn test_custom_config() {
        let config = DumbMatcherConfig {
            title_weight: 0.7,
            quality_weight: 0.1,
            health_weight: 0.1,
            size_weight: 0.1,
            min_seeders: 5,
            ideal_seeders: 50,
            min_size_bytes: 10 * 1024 * 1024,
            max_size_bytes: 100 * 1024 * 1024 * 1024,
        };
        let matcher = DumbMatcher::with_config(config);

        // With higher title weight, title match matters more
        let context = make_context(&[], "Abbey Road Beatles");
        let good_title = make_candidate("Abbey Road Beatles", 1, 1024); // low seeders, tiny
        let bad_title = make_candidate("Random Torrent", 100, 500 * 1024 * 1024); // great health

        let good_score = matcher.score_candidate(&good_title, &context);
        let bad_score = matcher.score_candidate(&bad_title, &context);

        // Even with poor health/size, good title should win with 0.7 weight
        assert!(good_score.score > bad_score.score);
    }
}
