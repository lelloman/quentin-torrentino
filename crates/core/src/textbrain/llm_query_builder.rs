//! LLM-powered query builder implementation.
//!
//! Uses a language model to generate optimized search queries
//! based on ticket context, expected content, and domain knowledge.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::ticket::{ExpectedContent, QueryContext};
use crate::textbrain::llm::{CompletionRequest, LlmClient, LlmUsage};
use crate::textbrain::traits::{QueryBuilder, TextBrainError};
use crate::textbrain::types::QueryBuildResult;

/// Configuration for the LLM query builder.
#[derive(Debug, Clone)]
pub struct LlmQueryBuilderConfig {
    /// Maximum number of queries to generate.
    pub max_queries: usize,
    /// Maximum tokens for the LLM response.
    pub max_tokens: u32,
    /// Temperature for generation (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
}

impl Default for LlmQueryBuilderConfig {
    fn default() -> Self {
        Self {
            max_queries: 5,
            max_tokens: 512,
            temperature: 0.2, // Slightly creative but mostly deterministic
        }
    }
}

/// LLM-powered query builder.
///
/// Generates search queries by prompting an LLM with the ticket context
/// and asking for optimized torrent search queries.
///
/// Generic over the LLM client type to support different backends
/// (Anthropic, Ollama, etc.).
pub struct LlmQueryBuilder<C: LlmClient> {
    client: Arc<C>,
    config: LlmQueryBuilderConfig,
}

impl<C: LlmClient> LlmQueryBuilder<C> {
    /// Create a new LLM query builder.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            config: LlmQueryBuilderConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(client: Arc<C>, config: LlmQueryBuilderConfig) -> Self {
        Self { client, config }
    }

    /// Build the system prompt for query generation.
    fn build_system_prompt(&self) -> String {
        r#"You are a torrent search query optimizer. Your task is to generate effective search queries for finding specific media content on torrent sites.

IMPORTANT RULES:
1. Generate 3-5 search queries, ordered from most specific to most general
2. Use common torrent naming conventions (e.g., "Artist - Album [FLAC]", "Movie.2023.1080p.BluRay")
3. Include quality indicators when specified (FLAC, 1080p, x265, etc.)
4. For albums: try "Artist - Album" and "Artist Album" formats
5. For movies: include year, try with and without resolution
6. For TV: use "Series S01E01" format, also try season packs
7. Remove request phrases like "looking for", "please", "I want"
8. Keep queries concise - torrent search works best with key terms
9. When required audio languages are specified, include the 3-letter language code in queries (e.g., "ita" for Italian, "eng" for English, "ger" for German)

Respond with JSON only, no other text:
{
  "queries": ["query1", "query2", "query3"],
  "confidence": 0.85,
  "reasoning": "Brief explanation of query strategy"
}"#.to_string()
    }

    /// Build the user prompt from context.
    fn build_user_prompt(&self, context: &QueryContext) -> String {
        use crate::ticket::LanguagePriority;

        let mut prompt = format!("Generate search queries for:\n\nDescription: {}", context.description);

        if !context.tags.is_empty() {
            prompt.push_str(&format!("\nTags: {}", context.tags.join(", ")));
        }

        if let Some(expected) = &context.expected {
            prompt.push_str("\n\nExpected content:\n");
            match expected {
                ExpectedContent::Album { artist, title, tracks } => {
                    if let Some(artist) = artist {
                        prompt.push_str(&format!("Album: {} - {}\n", artist, title));
                    } else {
                        prompt.push_str(&format!("Album: {}\n", title));
                    }
                    if !tracks.is_empty() {
                        prompt.push_str(&format!("Tracks: {} tracks\n", tracks.len()));
                    }
                }
                ExpectedContent::Track { artist, title } => {
                    if let Some(artist) = artist {
                        prompt.push_str(&format!("Single track: {} - {}\n", artist, title));
                    } else {
                        prompt.push_str(&format!("Single track: {}\n", title));
                    }
                }
                ExpectedContent::Movie { title, year } => {
                    if let Some(year) = year {
                        prompt.push_str(&format!("Movie: {} ({})\n", title, year));
                    } else {
                        prompt.push_str(&format!("Movie: {}\n", title));
                    }
                }
                ExpectedContent::TvEpisode { series, season, episodes } => {
                    let ep_str = if episodes.len() == 1 {
                        format!("E{:02}", episodes[0])
                    } else {
                        format!("E{:02}-E{:02}", episodes[0], episodes[episodes.len() - 1])
                    };
                    prompt.push_str(&format!("TV Episode: {} S{:02}{}\n", series, season, ep_str));
                }
            }
        }

        // Add required language constraints
        if let Some(ref constraints) = context.search_constraints {
            if let Some(ref video) = constraints.video {
                let required_audio: Vec<_> = video.audio_languages.iter()
                    .filter(|l| l.priority == LanguagePriority::Required)
                    .map(|l| Self::language_code_to_name(&l.code))
                    .collect();

                if !required_audio.is_empty() {
                    prompt.push_str(&format!(
                        "\n\nREQUIRED audio language(s): {}. Include language codes (e.g., ita, eng) in the queries.",
                        required_audio.join(", ")
                    ));
                }
            }
        }

        prompt.push_str(&format!("\n\nGenerate up to {} search queries.", self.config.max_queries));
        prompt
    }

    /// Convert ISO 639-1 language code to human-readable name.
    fn language_code_to_name(code: &str) -> &'static str {
        match code.to_lowercase().as_str() {
            "en" => "English (eng)",
            "it" => "Italian (ita)",
            "de" => "German (ger)",
            "fr" => "French (fre)",
            "es" => "Spanish (spa)",
            "pt" => "Portuguese (por)",
            "ru" => "Russian (rus)",
            "ja" => "Japanese (jpn)",
            "ko" => "Korean (kor)",
            "zh" => "Chinese (chi)",
            "nl" => "Dutch (dut)",
            "pl" => "Polish (pol)",
            "sv" => "Swedish (swe)",
            "no" => "Norwegian (nor)",
            "da" => "Danish (dan)",
            "fi" => "Finnish (fin)",
            "tr" => "Turkish (tur)",
            "ar" => "Arabic (ara)",
            "hi" => "Hindi (hin)",
            "th" => "Thai (tha)",
            _ => "Unknown",
        }
    }

    /// Parse the LLM response into a QueryBuildResult.
    fn parse_response(&self, text: &str, usage: LlmUsage) -> Result<QueryBuildResult, TextBrainError> {
        // Try to extract JSON from the response (handle markdown code blocks)
        let json_str = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else {
                text
            }
        } else {
            text
        };

        let parsed: LlmQueryResponse = serde_json::from_str(json_str)
            .map_err(|e| TextBrainError::LlmError(format!("Failed to parse LLM response: {} - Response: {}", e, text)))?;

        if parsed.queries.is_empty() {
            return Err(TextBrainError::NoQueriesGenerated);
        }

        // Limit to max queries
        let queries: Vec<String> = parsed.queries
            .into_iter()
            .take(self.config.max_queries)
            .collect();

        Ok(QueryBuildResult {
            queries,
            method: "llm".to_string(),
            confidence: parsed.confidence.unwrap_or(0.8),
            llm_usage: Some(usage),
        })
    }
}

/// Expected JSON response from the LLM.
#[derive(Debug, Deserialize, Serialize)]
struct LlmQueryResponse {
    queries: Vec<String>,
    confidence: Option<f32>,
    #[allow(dead_code)]
    reasoning: Option<String>,
}

#[async_trait]
impl<C: LlmClient + 'static> QueryBuilder for LlmQueryBuilder<C> {
    fn name(&self) -> &str {
        "llm"
    }

    async fn build_queries(&self, context: &QueryContext) -> Result<QueryBuildResult, TextBrainError> {
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(context);

        let request = CompletionRequest::new(user_prompt)
            .with_system(system_prompt)
            .with_max_tokens(self.config.max_tokens)
            .with_temperature(self.config.temperature);

        let response = self.client.complete(request).await
            .map_err(|e| TextBrainError::LlmError(e.to_string()))?;

        self.parse_response(&response.text, response.usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textbrain::llm::{CompletionResponse, LlmError};
    use crate::ticket::ExpectedTrack;
    use std::sync::Mutex;

    /// Mock LLM client for testing.
    struct MockLlmClient {
        response: Mutex<String>,
    }

    impl MockLlmClient {
        fn new(response: &str) -> Self {
            Self {
                response: Mutex::new(response.to_string()),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        fn provider(&self) -> &str {
            "mock"
        }

        fn model(&self) -> &str {
            "mock-model"
        }

        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            let text = self.response.lock().unwrap().clone();
            Ok(CompletionResponse {
                text,
                usage: LlmUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                },
                model: "mock-model".to_string(),
            })
        }
    }

    fn make_context(description: &str, tags: &[&str]) -> QueryContext {
        QueryContext::new(
            tags.iter().map(|s| s.to_string()).collect(),
            description,
        )
    }

    #[tokio::test]
    async fn test_build_queries_success() {
        let response = r#"{
            "queries": ["Pink Floyd - Dark Side of the Moon FLAC", "Pink Floyd Dark Side Moon", "Dark Side Moon FLAC"],
            "confidence": 0.9,
            "reasoning": "Standard album search patterns"
        }"#;

        let client = Arc::new(MockLlmClient::new(response));
        let builder = LlmQueryBuilder::new(client);

        let context = make_context("Dark Side of the Moon by Pink Floyd", &["flac", "music"]);
        let result = builder.build_queries(&context).await.unwrap();

        assert_eq!(result.queries.len(), 3);
        assert!(result.queries[0].contains("Pink Floyd"));
        assert_eq!(result.method, "llm");
        assert_eq!(result.confidence, 0.9);
        assert!(result.llm_usage.is_some());
    }

    #[tokio::test]
    async fn test_build_queries_with_code_block() {
        // LLM might wrap response in markdown code block
        let response = r#"```json
{
    "queries": ["Test Query 1", "Test Query 2"],
    "confidence": 0.85
}
```"#;

        let client = Arc::new(MockLlmClient::new(response));
        let builder = LlmQueryBuilder::new(client);

        let context = make_context("Test content", &[]);
        let result = builder.build_queries(&context).await.unwrap();

        assert_eq!(result.queries.len(), 2);
    }

    #[tokio::test]
    async fn test_build_queries_empty_response() {
        let response = r#"{"queries": [], "confidence": 0.0}"#;

        let client = Arc::new(MockLlmClient::new(response));
        let builder = LlmQueryBuilder::new(client);

        let context = make_context("Test", &[]);
        let result = builder.build_queries(&context).await;

        assert!(matches!(result, Err(TextBrainError::NoQueriesGenerated)));
    }

    #[tokio::test]
    async fn test_build_queries_invalid_json() {
        let response = "This is not valid JSON";

        let client = Arc::new(MockLlmClient::new(response));
        let builder = LlmQueryBuilder::new(client);

        let context = make_context("Test", &[]);
        let result = builder.build_queries(&context).await;

        assert!(matches!(result, Err(TextBrainError::LlmError(_))));
    }

    #[test]
    fn test_build_user_prompt_album() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let builder = LlmQueryBuilder::new(client);

        let mut context = make_context("Dark Side of the Moon", &["flac"]);
        context.expected = Some(ExpectedContent::Album {
            artist: Some("Pink Floyd".to_string()),
            title: "The Dark Side of the Moon".to_string(),
            tracks: vec![
                ExpectedTrack { number: 1, title: "Speak to Me".to_string(), duration_secs: None, duration_ms: None, disc_number: None },
            ],
        });

        let prompt = builder.build_user_prompt(&context);
        assert!(prompt.contains("Pink Floyd"));
        assert!(prompt.contains("The Dark Side of the Moon"));
        assert!(prompt.contains("Album:"));
    }

    #[test]
    fn test_build_user_prompt_movie() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let builder = LlmQueryBuilder::new(client);

        let mut context = make_context("Inception", &["1080p"]);
        context.expected = Some(ExpectedContent::Movie {
            title: "Inception".to_string(),
            year: Some(2010),
        });

        let prompt = builder.build_user_prompt(&context);
        assert!(prompt.contains("Inception"));
        assert!(prompt.contains("2010"));
        assert!(prompt.contains("Movie:"));
    }

    #[test]
    fn test_build_user_prompt_tv() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let builder = LlmQueryBuilder::new(client);

        let mut context = make_context("Breaking Bad S01E01", &[]);
        context.expected = Some(ExpectedContent::TvEpisode {
            series: "Breaking Bad".to_string(),
            season: 1,
            episodes: vec![1],
        });

        let prompt = builder.build_user_prompt(&context);
        assert!(prompt.contains("Breaking Bad"));
        assert!(prompt.contains("S01E01"));
        assert!(prompt.contains("TV Episode:"));
    }

    #[test]
    fn test_parse_response_minimal() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let builder = LlmQueryBuilder::new(client);

        let text = r#"{"queries": ["query1"]}"#;
        let usage = LlmUsage { input_tokens: 10, output_tokens: 5 };

        let result = builder.parse_response(text, usage).unwrap();
        assert_eq!(result.queries.len(), 1);
        assert_eq!(result.confidence, 0.8); // Default when not specified
    }

    #[test]
    fn test_max_queries_limit() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let config = LlmQueryBuilderConfig {
            max_queries: 2,
            ..Default::default()
        };
        let builder = LlmQueryBuilder::with_config(client, config);

        let text = r#"{"queries": ["q1", "q2", "q3", "q4", "q5"]}"#;
        let usage = LlmUsage { input_tokens: 10, output_tokens: 5 };

        let result = builder.parse_response(text, usage).unwrap();
        assert_eq!(result.queries.len(), 2);
    }

    #[test]
    fn test_builder_name() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let builder = LlmQueryBuilder::new(client);
        assert_eq!(builder.name(), "llm");
    }
}
