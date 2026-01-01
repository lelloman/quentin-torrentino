//! LLM-powered candidate matcher implementation.
//!
//! Uses a language model to score torrent candidates against
//! the ticket context, providing nuanced matching with reasoning.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::searcher::TorrentCandidate;
use crate::ticket::{ExpectedContent, QueryContext};
use crate::textbrain::llm::{CompletionRequest, LlmClient, LlmUsage};
use crate::textbrain::traits::{CandidateMatcher, TextBrainError};
use crate::textbrain::types::{MatchResult, ScoredCandidate};

/// Configuration for the LLM matcher.
#[derive(Debug, Clone)]
pub struct LlmMatcherConfig {
    /// Maximum candidates to send to LLM (to limit token usage).
    pub max_candidates: usize,
    /// Maximum tokens for the LLM response.
    pub max_tokens: u32,
    /// Temperature for generation.
    pub temperature: f32,
}

impl Default for LlmMatcherConfig {
    fn default() -> Self {
        Self {
            max_candidates: 10, // Top 10 candidates to score
            max_tokens: 1024,
            temperature: 0.0, // Deterministic scoring
        }
    }
}

/// LLM-powered candidate matcher.
///
/// Scores torrent candidates by prompting an LLM with the context
/// and candidate list, asking for quality assessments.
///
/// Generic over the LLM client type to support different backends
/// (Anthropic, Ollama, etc.).
pub struct LlmMatcher<C: LlmClient> {
    client: Arc<C>,
    config: LlmMatcherConfig,
}

impl<C: LlmClient> LlmMatcher<C> {
    /// Create a new LLM matcher.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            config: LlmMatcherConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(client: Arc<C>, config: LlmMatcherConfig) -> Self {
        Self { client, config }
    }

    /// Build the system prompt for candidate scoring.
    fn build_system_prompt(&self) -> String {
        r#"You are a torrent quality evaluator. Your task is to score torrent candidates based on how well they match the user's request.

SCORING GUIDELINES (0.0 to 1.0):
- 0.95-1.0: Perfect match - exact content, preferred quality, healthy seeders
- 0.85-0.94: Excellent match - correct content, good quality
- 0.70-0.84: Good match - likely correct, acceptable quality
- 0.50-0.69: Partial match - might be correct, some concerns
- 0.30-0.49: Poor match - probably wrong content or bad quality
- 0.00-0.29: No match - wrong content, fake, or unusable

RED FLAGS (reduce score significantly):
- CAM, HDCAM, TS, TELESYNC = low quality bootleg
- SCREENER, SCR = early leak, often poor quality
- Wrong year for movies
- Wrong season for TV
- Very few seeders (< 2) = may be dead
- Suspiciously small file size
- MP3 when FLAC requested (or similar quality mismatch)

GREEN FLAGS (increase score):
- REMUX, PROPER, REPACK = high quality
- Multiple release groups = well-seeded
- File list matches expected tracks/episodes
- Matches requested codec/resolution exactly

Respond with JSON only:
{
  "scores": [
    {"index": 0, "score": 0.95, "reasoning": "Brief explanation"},
    {"index": 1, "score": 0.72, "reasoning": "Brief explanation"}
  ]
}"#.to_string()
    }

    /// Build the user prompt with context and candidates.
    fn build_user_prompt(&self, context: &QueryContext, candidates: &[TorrentCandidate]) -> String {
        let mut prompt = String::new();

        // Describe what we're looking for
        prompt.push_str("LOOKING FOR:\n");
        prompt.push_str(&format!("Description: {}\n", context.description));

        if !context.tags.is_empty() {
            prompt.push_str(&format!("Quality requirements: {}\n", context.tags.join(", ")));
        }

        if let Some(expected) = &context.expected {
            match expected {
                ExpectedContent::Album { artist, title, tracks } => {
                    prompt.push_str("Type: Music Album\n");
                    if let Some(artist) = artist {
                        prompt.push_str(&format!("Artist: {}\n", artist));
                    }
                    prompt.push_str(&format!("Album: {}\n", title));
                    if !tracks.is_empty() {
                        prompt.push_str(&format!("Expected tracks: {}\n", tracks.len()));
                    }
                }
                ExpectedContent::Track { artist, title } => {
                    prompt.push_str("Type: Single Track\n");
                    if let Some(artist) = artist {
                        prompt.push_str(&format!("Artist: {}\n", artist));
                    }
                    prompt.push_str(&format!("Track: {}\n", title));
                }
                ExpectedContent::Movie { title, year } => {
                    prompt.push_str("Type: Movie\n");
                    prompt.push_str(&format!("Title: {}\n", title));
                    if let Some(year) = year {
                        prompt.push_str(&format!("Year: {}\n", year));
                    }
                }
                ExpectedContent::TvEpisode { series, season, episodes } => {
                    prompt.push_str("Type: TV Episode\n");
                    prompt.push_str(&format!("Series: {}\n", series));
                    prompt.push_str(&format!("Season: {}\n", season));
                    prompt.push_str(&format!("Episodes: {:?}\n", episodes));
                }
            }
        }

        prompt.push_str("\nCANDIDATES TO SCORE:\n");

        for (i, candidate) in candidates.iter().enumerate() {
            prompt.push_str(&format!(
                "\n[{}] Title: {}\n    Size: {} MB, Seeders: {}\n",
                i,
                candidate.title,
                candidate.size_bytes / (1024 * 1024),
                candidate.seeders
            ));

            // Include file list if available (very helpful for matching)
            if let Some(files) = &candidate.files {
                let file_names: Vec<&str> = files.iter()
                    .take(10) // Limit to 10 files
                    .map(|f| f.path.as_str())
                    .collect();
                if !file_names.is_empty() {
                    prompt.push_str(&format!("    Files: {}\n", file_names.join(", ")));
                }
                if files.len() > 10 {
                    prompt.push_str(&format!("    ... and {} more files\n", files.len() - 10));
                }
            }
        }

        prompt.push_str("\nScore each candidate from 0.0 to 1.0 based on match quality.");
        prompt
    }

    /// Parse the LLM response into scored candidates.
    fn parse_response(
        &self,
        text: &str,
        candidates: &[TorrentCandidate],
        usage: LlmUsage,
    ) -> Result<MatchResult, TextBrainError> {
        // Extract JSON from response
        let json_str = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else {
                text
            }
        } else {
            text
        };

        let parsed: LlmScoreResponse = serde_json::from_str(json_str)
            .map_err(|e| TextBrainError::LlmError(format!("Failed to parse LLM response: {} - Response: {}", e, text)))?;

        // Build scored candidates
        let mut scored: Vec<ScoredCandidate> = Vec::new();

        for score_item in parsed.scores {
            if score_item.index < candidates.len() {
                let candidate = &candidates[score_item.index];
                scored.push(ScoredCandidate {
                    candidate: candidate.clone(),
                    score: score_item.score.clamp(0.0, 1.0),
                    reasoning: score_item.reasoning.unwrap_or_else(|| "LLM scored".to_string()),
                    file_mappings: Vec::new(), // LLM doesn't do file mapping yet
                });
            }
        }

        // Add any candidates not scored by LLM with default low score
        for (i, candidate) in candidates.iter().enumerate() {
            if !scored.iter().any(|s| s.candidate.info_hash == candidate.info_hash) {
                scored.push(ScoredCandidate {
                    candidate: candidate.clone(),
                    score: 0.3, // Default uncertain score
                    reasoning: format!("Not scored by LLM (index {})", i),
                    file_mappings: Vec::new(),
                });
            }
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(MatchResult {
            candidates: scored,
            method: "llm".to_string(),
            llm_usage: Some(usage),
        })
    }
}

/// Expected JSON response from the LLM for scoring.
#[derive(Debug, Deserialize, Serialize)]
struct LlmScoreResponse {
    scores: Vec<LlmScoreItem>,
}

/// Individual score item from LLM.
#[derive(Debug, Deserialize, Serialize)]
struct LlmScoreItem {
    index: usize,
    score: f32,
    reasoning: Option<String>,
}

#[async_trait]
impl<C: LlmClient + 'static> CandidateMatcher for LlmMatcher<C> {
    fn name(&self) -> &str {
        "llm"
    }

    async fn score_candidates(
        &self,
        context: &QueryContext,
        candidates: &[TorrentCandidate],
    ) -> Result<MatchResult, TextBrainError> {
        if candidates.is_empty() {
            return Ok(MatchResult {
                candidates: Vec::new(),
                method: "llm".to_string(),
                llm_usage: None,
            });
        }

        // Limit candidates to reduce token usage
        let candidates_to_score: Vec<&TorrentCandidate> = candidates
            .iter()
            .take(self.config.max_candidates)
            .collect();

        // Build a slice of references for the prompt builder
        let candidates_slice: Vec<TorrentCandidate> = candidates_to_score
            .iter()
            .map(|c| (*c).clone())
            .collect();

        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(context, &candidates_slice);

        let request = CompletionRequest::new(user_prompt)
            .with_system(system_prompt)
            .with_max_tokens(self.config.max_tokens)
            .with_temperature(self.config.temperature);

        let response = self.client.complete(request).await
            .map_err(|e| TextBrainError::LlmError(e.to_string()))?;

        // Parse scores for the candidates we sent
        let mut result = self.parse_response(&response.text, &candidates_slice, response.usage)?;

        // Add remaining candidates that weren't sent to LLM
        if candidates.len() > self.config.max_candidates {
            for candidate in candidates.iter().skip(self.config.max_candidates) {
                result.candidates.push(ScoredCandidate {
                    candidate: candidate.clone(),
                    score: 0.2, // Low score for non-evaluated
                    reasoning: "Not evaluated by LLM (overflow)".to_string(),
                    file_mappings: Vec::new(),
                });
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::searcher::TorrentSource;
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
                    input_tokens: 200,
                    output_tokens: 100,
                },
                model: "mock-model".to_string(),
            })
        }
    }

    fn make_candidate(title: &str, seeders: u32, size_mb: u64) -> TorrentCandidate {
        TorrentCandidate {
            title: title.to_string(),
            info_hash: format!("hash_{}", title.replace(' ', "_")),
            size_bytes: size_mb * 1024 * 1024,
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

    fn make_context(description: &str, tags: &[&str]) -> QueryContext {
        QueryContext::new(
            tags.iter().map(|s| s.to_string()).collect(),
            description,
        )
    }

    #[tokio::test]
    async fn test_score_candidates_success() {
        let response = r#"{
            "scores": [
                {"index": 0, "score": 0.95, "reasoning": "Excellent match"},
                {"index": 1, "score": 0.72, "reasoning": "Good but lower quality"}
            ]
        }"#;

        let client = Arc::new(MockLlmClient::new(response));
        let matcher = LlmMatcher::new(client);

        let context = make_context("Abbey Road by The Beatles", &["flac"]);
        let candidates = vec![
            make_candidate("The Beatles - Abbey Road [FLAC]", 50, 500),
            make_candidate("Beatles Greatest Hits MP3", 30, 100),
        ];

        let result = matcher.score_candidates(&context, &candidates).await.unwrap();

        assert_eq!(result.candidates.len(), 2);
        assert_eq!(result.method, "llm");
        assert!(result.llm_usage.is_some());

        // Should be sorted by score
        assert!(result.candidates[0].score >= result.candidates[1].score);
        assert_eq!(result.candidates[0].score, 0.95);
        assert_eq!(result.candidates[0].reasoning, "Excellent match");
    }

    #[tokio::test]
    async fn test_score_candidates_empty() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let matcher = LlmMatcher::new(client);

        let context = make_context("test", &[]);
        let result = matcher.score_candidates(&context, &[]).await.unwrap();

        assert!(result.candidates.is_empty());
        assert_eq!(result.method, "llm");
        assert!(result.llm_usage.is_none());
    }

    #[tokio::test]
    async fn test_score_candidates_missing_indices() {
        // LLM only scores some candidates
        let response = r#"{
            "scores": [
                {"index": 0, "score": 0.9, "reasoning": "Good"}
            ]
        }"#;

        let client = Arc::new(MockLlmClient::new(response));
        let matcher = LlmMatcher::new(client);

        let context = make_context("test", &[]);
        let candidates = vec![
            make_candidate("Candidate 1", 10, 100),
            make_candidate("Candidate 2", 20, 200),
        ];

        let result = matcher.score_candidates(&context, &candidates).await.unwrap();

        assert_eq!(result.candidates.len(), 2);
        // First should be the high-scored one
        assert_eq!(result.candidates[0].score, 0.9);
        // Second should have default low score
        assert_eq!(result.candidates[1].score, 0.3);
    }

    #[tokio::test]
    async fn test_score_candidates_clamps_invalid_scores() {
        let response = r#"{
            "scores": [
                {"index": 0, "score": 1.5, "reasoning": "Too high"},
                {"index": 1, "score": -0.5, "reasoning": "Too low"}
            ]
        }"#;

        let client = Arc::new(MockLlmClient::new(response));
        let matcher = LlmMatcher::new(client);

        let context = make_context("test", &[]);
        let candidates = vec![
            make_candidate("Candidate 1", 10, 100),
            make_candidate("Candidate 2", 20, 200),
        ];

        let result = matcher.score_candidates(&context, &candidates).await.unwrap();

        // Scores should be clamped to 0.0-1.0
        assert_eq!(result.candidates[0].score, 1.0);
        assert_eq!(result.candidates[1].score, 0.0);
    }

    #[test]
    fn test_build_user_prompt_with_album() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let matcher = LlmMatcher::new(client);

        let mut context = make_context("Dark Side of the Moon", &["flac"]);
        context.expected = Some(ExpectedContent::Album {
            artist: Some("Pink Floyd".to_string()),
            title: "The Dark Side of the Moon".to_string(),
            tracks: vec![
                ExpectedTrack { number: 1, title: "Speak to Me".to_string(), duration_secs: None, duration_ms: None, disc_number: None },
            ],
        });

        let candidates = vec![make_candidate("Pink Floyd - Dark Side FLAC", 50, 500)];
        let prompt = matcher.build_user_prompt(&context, &candidates);

        assert!(prompt.contains("Pink Floyd"));
        assert!(prompt.contains("Music Album"));
        assert!(prompt.contains("Expected tracks: 1"));
    }

    #[test]
    fn test_build_user_prompt_with_movie() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let matcher = LlmMatcher::new(client);

        let mut context = make_context("Inception", &["1080p"]);
        context.expected = Some(ExpectedContent::Movie {
            title: "Inception".to_string(),
            year: Some(2010),
        });

        let candidates = vec![make_candidate("Inception.2010.1080p.BluRay", 100, 5000)];
        let prompt = matcher.build_user_prompt(&context, &candidates);

        assert!(prompt.contains("Movie"));
        assert!(prompt.contains("Inception"));
        assert!(prompt.contains("2010"));
    }

    #[test]
    fn test_build_user_prompt_with_files() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let matcher = LlmMatcher::new(client);

        let context = make_context("Test", &[]);
        let mut candidate = make_candidate("Test Album", 50, 500);
        candidate.files = Some(vec![
            crate::searcher::TorrentFile { path: "01 - Track One.flac".to_string(), size_bytes: 50_000_000 },
            crate::searcher::TorrentFile { path: "02 - Track Two.flac".to_string(), size_bytes: 50_000_000 },
        ]);

        let prompt = matcher.build_user_prompt(&context, &[candidate]);

        assert!(prompt.contains("Track One"));
        assert!(prompt.contains("Track Two"));
    }

    #[tokio::test]
    async fn test_max_candidates_limit() {
        let response = r#"{
            "scores": [
                {"index": 0, "score": 0.9}
            ]
        }"#;

        let client = Arc::new(MockLlmClient::new(response));
        let config = LlmMatcherConfig {
            max_candidates: 2,
            ..Default::default()
        };
        let matcher = LlmMatcher::with_config(client, config);

        let context = make_context("test", &[]);
        let candidates = vec![
            make_candidate("C1", 10, 100),
            make_candidate("C2", 20, 200),
            make_candidate("C3", 30, 300), // This one exceeds max
        ];

        let result = matcher.score_candidates(&context, &candidates).await.unwrap();

        // All candidates should be in result
        assert_eq!(result.candidates.len(), 3);

        // The overflow candidate should have low score
        let overflow_candidate = result.candidates.iter()
            .find(|c| c.candidate.title == "C3")
            .unwrap();
        assert_eq!(overflow_candidate.score, 0.2);
        assert!(overflow_candidate.reasoning.contains("overflow"));
    }

    #[test]
    fn test_matcher_name() {
        let client = Arc::new(MockLlmClient::new("{}"));
        let matcher = LlmMatcher::new(client);
        assert_eq!(matcher.name(), "llm");
    }
}
