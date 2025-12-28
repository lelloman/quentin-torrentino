//! Dumb (heuristic-based) query builder implementation.
//!
//! Generates search queries using simple string manipulation and templates.
//! No LLM required - works entirely offline.

use async_trait::async_trait;

use crate::ticket::QueryContext;
use crate::textbrain::traits::{QueryBuilder, TextBrainError};
use crate::textbrain::types::QueryBuildResult;

/// Configuration for the dumb query builder.
#[derive(Debug, Clone)]
pub struct DumbQueryBuilderConfig {
    /// Maximum number of queries to generate.
    pub max_queries: usize,
    /// Include tags in queries.
    pub include_tags: bool,
    /// Common stop words to filter out.
    pub stop_words: Vec<String>,
}

impl Default for DumbQueryBuilderConfig {
    fn default() -> Self {
        Self {
            max_queries: 5,
            include_tags: true,
            stop_words: vec![
                "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
                "by", "from", "as", "is", "was", "are", "were", "been", "be", "have", "has", "had",
                "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
                "prefer", "preferably", "preferred", "please", "want", "wanted", "looking",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

/// Heuristic-based query builder.
///
/// Generates search queries by:
/// 1. Using the description with optional quality tags
/// 2. Extracting key terms and combining with tags
/// 3. Simplifying to core terms only
pub struct DumbQueryBuilder {
    config: DumbQueryBuilderConfig,
}

impl DumbQueryBuilder {
    /// Create a new dumb query builder with default config.
    pub fn new() -> Self {
        Self {
            config: DumbQueryBuilderConfig::default(),
        }
    }

    /// Create a new dumb query builder with custom config.
    pub fn with_config(config: DumbQueryBuilderConfig) -> Self {
        Self { config }
    }

    /// Extract key terms from a description.
    ///
    /// Removes stop words, punctuation, and normalizes whitespace.
    fn extract_key_terms(&self, description: &str) -> Vec<String> {
        let stop_words: std::collections::HashSet<_> = self
            .config
            .stop_words
            .iter()
            .map(|s| s.to_lowercase())
            .collect();

        description
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| s.len() > 1) // Skip single chars
            .filter(|s| !stop_words.contains(&s.to_lowercase()))
            .collect()
    }

    /// Extract quality-related tags (resolution, codec, format).
    fn extract_quality_tags(&self, tags: &[String]) -> Vec<String> {
        let quality_patterns = [
            "flac", "mp3", "aac", "opus", "wav", "alac", // Audio
            "1080p", "720p", "2160p", "4k", "uhd", "hdr", // Video resolution
            "x264", "x265", "hevc", "h264", "h265", "av1", // Video codec
            "bluray", "blu-ray", "remux", "web-dl", "webrip", "hdtv", // Source
            "dts", "atmos", "truehd", // Audio for video
        ];

        tags.iter()
            .filter(|t| {
                let lower = t.to_lowercase();
                quality_patterns.iter().any(|p| lower.contains(p))
            })
            .cloned()
            .collect()
    }

    /// Generate queries with decreasing specificity.
    fn generate_queries(&self, context: &QueryContext) -> Vec<String> {
        let mut queries = Vec::new();
        let key_terms = self.extract_key_terms(&context.description);
        let quality_tags = self.extract_quality_tags(&context.tags);

        // Query 1: Full description + quality tags (most specific)
        if !context.description.is_empty() {
            let cleaned_desc = self.clean_description(&context.description);
            if !quality_tags.is_empty() {
                queries.push(format!("{} {}", cleaned_desc, quality_tags.join(" ")));
            }
            // Also add just the cleaned description
            if !cleaned_desc.is_empty() {
                queries.push(cleaned_desc);
            }
        }

        // Query 2: Key terms + quality tags
        if key_terms.len() >= 2 {
            let terms_str = key_terms.join(" ");
            if !quality_tags.is_empty() {
                queries.push(format!("{} {}", terms_str, quality_tags.join(" ")));
            }
            // Key terms without quality
            queries.push(terms_str);
        }

        // Query 3: Main terms only (first 3-4 key terms)
        if key_terms.len() > 3 {
            let main_terms: Vec<_> = key_terms.iter().take(4).cloned().collect();
            queries.push(main_terms.join(" "));
        }

        // Query 4: If we have year-like numbers, try without them
        let without_years: Vec<_> = key_terms
            .iter()
            .filter(|t| !self.looks_like_year(t))
            .cloned()
            .collect();
        if without_years.len() >= 2 && without_years.len() < key_terms.len() {
            queries.push(without_years.join(" "));
        }

        // Deduplicate and limit
        let mut seen = std::collections::HashSet::new();
        queries
            .into_iter()
            .filter(|q| !q.is_empty())
            .filter(|q| seen.insert(q.to_lowercase()))
            .take(self.config.max_queries)
            .collect()
    }

    /// Clean a description by removing common request phrases.
    fn clean_description(&self, description: &str) -> String {
        let patterns_to_remove = [
            "prefer ", "preferably ", "preferred ",
            "please ", "looking for ", "want ", "wanted ",
            "need ", "searching for ", "find ",
        ];

        let mut result = description.to_string();
        for pattern in patterns_to_remove {
            result = result.replace(pattern, "");
        }

        // Normalize whitespace
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Check if a string looks like a year (1900-2099).
    fn looks_like_year(&self, s: &str) -> bool {
        if s.len() != 4 {
            return false;
        }
        match s.parse::<u32>() {
            Ok(n) => (1900..=2099).contains(&n),
            Err(_) => false,
        }
    }
}

impl Default for DumbQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueryBuilder for DumbQueryBuilder {
    fn name(&self) -> &str {
        "dumb"
    }

    async fn build_queries(
        &self,
        context: &QueryContext,
    ) -> Result<QueryBuildResult, TextBrainError> {
        let queries = self.generate_queries(context);

        if queries.is_empty() {
            return Err(TextBrainError::NoQueriesGenerated);
        }

        // Confidence based on how much info we have to work with
        let confidence = self.estimate_confidence(context, &queries);

        Ok(QueryBuildResult {
            queries,
            method: "dumb".to_string(),
            confidence,
            llm_usage: None,
        })
    }
}

impl DumbQueryBuilder {
    /// Estimate confidence based on input quality.
    fn estimate_confidence(&self, context: &QueryContext, queries: &[String]) -> f32 {
        let mut confidence: f32 = 0.5; // Base confidence

        // More key terms = higher confidence
        let key_terms = self.extract_key_terms(&context.description);
        if key_terms.len() >= 4 {
            confidence += 0.15;
        } else if key_terms.len() >= 2 {
            confidence += 0.1;
        }

        // Quality tags help
        if !self.extract_quality_tags(&context.tags).is_empty() {
            confidence += 0.1;
        }

        // Multiple queries = we have fallback options
        if queries.len() >= 3 {
            confidence += 0.1;
        }

        // Tags present
        if !context.tags.is_empty() {
            confidence += 0.05;
        }

        confidence.min(0.9) // Cap at 0.9, always some uncertainty without LLM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(tags: &[&str], description: &str) -> QueryContext {
        QueryContext::new(
            tags.iter().map(|s| s.to_string()).collect(),
            description,
        )
    }

    #[test]
    fn test_extract_key_terms() {
        let builder = DumbQueryBuilder::new();

        let terms = builder.extract_key_terms("Abbey Road by The Beatles");
        assert!(terms.contains(&"Abbey".to_string()));
        assert!(terms.contains(&"Road".to_string()));
        assert!(terms.contains(&"Beatles".to_string()));
        assert!(!terms.contains(&"by".to_string())); // stop word
        assert!(!terms.contains(&"The".to_string())); // stop word
    }

    #[test]
    fn test_extract_key_terms_with_punctuation() {
        let builder = DumbQueryBuilder::new();

        let terms = builder.extract_key_terms("Pink Floyd - Dark Side of the Moon (1973)");
        assert!(terms.contains(&"Pink".to_string()));
        assert!(terms.contains(&"Floyd".to_string()));
        assert!(terms.contains(&"Dark".to_string()));
        assert!(terms.contains(&"Side".to_string()));
        assert!(terms.contains(&"Moon".to_string()));
        assert!(terms.contains(&"1973".to_string()));
    }

    #[test]
    fn test_extract_quality_tags() {
        let builder = DumbQueryBuilder::new();

        let tags = vec![
            "music".to_string(),
            "flac".to_string(),
            "album".to_string(),
        ];
        let quality = builder.extract_quality_tags(&tags);
        assert_eq!(quality, vec!["flac"]);

        let tags = vec!["movie".to_string(), "1080p".to_string(), "x265".to_string()];
        let quality = builder.extract_quality_tags(&tags);
        assert!(quality.contains(&"1080p".to_string()));
        assert!(quality.contains(&"x265".to_string()));
    }

    #[test]
    fn test_generate_queries_music() {
        let builder = DumbQueryBuilder::new();
        let context = make_context(
            &["music", "flac", "album"],
            "Abbey Road by The Beatles, prefer 2019 remaster",
        );

        let queries = builder.generate_queries(&context);
        assert!(!queries.is_empty());

        // Should have query with flac
        assert!(queries.iter().any(|q| q.contains("flac")));
        // Should have Beatles
        assert!(queries.iter().any(|q| q.contains("Beatles")));
        // Should have Abbey Road
        assert!(queries.iter().any(|q| q.contains("Abbey")));
    }

    #[test]
    fn test_generate_queries_movie() {
        let builder = DumbQueryBuilder::new();
        let context = make_context(&["movie", "1080p", "x265"], "The Matrix 1999");

        let queries = builder.generate_queries(&context);
        assert!(!queries.is_empty());

        // Should have query with quality tags
        assert!(queries.iter().any(|q| q.contains("1080p") || q.contains("x265")));
        // Should have Matrix
        assert!(queries.iter().any(|q| q.contains("Matrix")));
    }

    #[test]
    fn test_generate_queries_removes_duplicates() {
        let builder = DumbQueryBuilder::new();
        let context = make_context(&[], "Test Query");

        let queries = builder.generate_queries(&context);
        let unique: std::collections::HashSet<_> = queries.iter().collect();
        assert_eq!(queries.len(), unique.len());
    }

    #[test]
    fn test_clean_description() {
        let builder = DumbQueryBuilder::new();

        assert_eq!(
            builder.clean_description("prefer 2019 remaster"),
            "2019 remaster"
        );
        assert_eq!(
            builder.clean_description("looking for Abbey Road"),
            "Abbey Road"
        );
    }

    #[test]
    fn test_looks_like_year() {
        let builder = DumbQueryBuilder::new();

        assert!(builder.looks_like_year("1999"));
        assert!(builder.looks_like_year("2024"));
        assert!(!builder.looks_like_year("123"));
        assert!(!builder.looks_like_year("12345"));
        assert!(!builder.looks_like_year("abcd"));
        assert!(!builder.looks_like_year("1800")); // Too old
    }

    #[tokio::test]
    async fn test_build_queries_success() {
        let builder = DumbQueryBuilder::new();
        let context = make_context(&["music", "flac"], "Abbey Road Beatles");

        let result = builder.build_queries(&context).await.unwrap();
        assert!(!result.queries.is_empty());
        assert_eq!(result.method, "dumb");
        assert!(result.confidence > 0.0);
        assert!(result.llm_usage.is_none());
    }

    #[tokio::test]
    async fn test_build_queries_empty_description() {
        let builder = DumbQueryBuilder::new();
        let context = make_context(&[], "");

        let result = builder.build_queries(&context).await;
        assert!(matches!(result, Err(TextBrainError::NoQueriesGenerated)));
    }

    #[test]
    fn test_confidence_estimation() {
        let builder = DumbQueryBuilder::new();

        // Low info context
        let low_ctx = make_context(&[], "Test");
        let low_queries = builder.generate_queries(&low_ctx);
        let low_conf = builder.estimate_confidence(&low_ctx, &low_queries);

        // High info context
        let high_ctx = make_context(
            &["music", "flac", "album"],
            "Abbey Road by The Beatles 2019 Anniversary Edition",
        );
        let high_queries = builder.generate_queries(&high_ctx);
        let high_conf = builder.estimate_confidence(&high_ctx, &high_queries);

        assert!(high_conf > low_conf);
    }

    #[test]
    fn test_max_queries_limit() {
        let config = DumbQueryBuilderConfig {
            max_queries: 2,
            ..Default::default()
        };
        let builder = DumbQueryBuilder::with_config(config);
        let context = make_context(
            &["music", "flac", "lossless"],
            "A very long description with many words that would generate many queries normally",
        );

        let queries = builder.generate_queries(&context);
        assert!(queries.len() <= 2);
    }

    #[test]
    fn test_query_builder_name() {
        let builder = DumbQueryBuilder::new();
        assert_eq!(builder.name(), "dumb");
    }
}
