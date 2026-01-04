//! TextBrain configuration types.

use serde::{Deserialize, Serialize};

use crate::searcher::FileEnricherConfig;

/// TextBrain coordination mode.
///
/// Determines how dumb (heuristic) and LLM methods are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextBrainMode {
    /// Use only heuristic methods, no LLM.
    /// Fastest, no API costs, works offline.
    #[default]
    DumbOnly,

    /// Try heuristics first, use LLM only if confidence is low.
    /// Good balance of speed and accuracy.
    DumbFirst,

    /// Use LLM first, fall back to heuristics on error.
    /// Best accuracy, higher latency and cost.
    LlmFirst,

    /// Use only LLM, fail if unavailable.
    /// Maximum accuracy, requires LLM configuration.
    LlmOnly,
}

impl TextBrainMode {
    /// Returns true if this mode requires LLM to be configured.
    pub fn requires_llm(&self) -> bool {
        matches!(self, TextBrainMode::LlmOnly)
    }

    /// Returns true if this mode can use LLM when available.
    pub fn can_use_llm(&self) -> bool {
        !matches!(self, TextBrainMode::DumbOnly)
    }

    /// Returns true if this mode can use dumb methods.
    pub fn can_use_dumb(&self) -> bool {
        !matches!(self, TextBrainMode::LlmOnly)
    }
}

/// LLM provider type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    /// Anthropic Claude API.
    Anthropic,
    /// OpenAI API (GPT models).
    OpenAi,
    /// Local Ollama instance.
    Ollama,
    /// Custom HTTP endpoint (must be OpenAI-compatible).
    Custom,
}

/// LLM client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// LLM provider.
    pub provider: LlmProvider,
    /// Model name/identifier.
    pub model: String,
    /// API key (can reference env var with ${VAR_NAME}).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Custom API base URL (for proxies or self-hosted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    /// Maximum tokens for completions.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_timeout() -> u32 {
    30
}

fn default_max_tokens() -> u32 {
    1024
}

/// TextBrain configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBrainConfig {
    /// Coordination mode.
    #[serde(default)]
    pub mode: TextBrainMode,
    /// Score threshold for auto-approval (0.0-1.0).
    /// Candidates with score >= threshold are auto-approved.
    #[serde(default = "default_auto_approve_threshold")]
    pub auto_approve_threshold: f32,
    /// Confidence threshold for fallback to LLM (in dumb-first mode).
    /// If dumb confidence < threshold, try LLM.
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,
    /// Maximum queries to try before giving up.
    #[serde(default = "default_max_queries")]
    pub max_queries: u32,
    /// LLM configuration (required for modes that use LLM).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmConfig>,
    /// File enrichment configuration.
    /// Controls fetching and caching of torrent file listings.
    #[serde(default)]
    pub file_enrichment: FileEnricherConfig,
}

fn default_auto_approve_threshold() -> f32 {
    0.85
}

fn default_confidence_threshold() -> f32 {
    0.7
}

fn default_max_queries() -> u32 {
    5
}

impl Default for TextBrainConfig {
    fn default() -> Self {
        Self {
            mode: TextBrainMode::default(),
            auto_approve_threshold: default_auto_approve_threshold(),
            confidence_threshold: default_confidence_threshold(),
            max_queries: default_max_queries(),
            llm: None,
            file_enrichment: FileEnricherConfig::default(),
        }
    }
}

impl TextBrainConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), String> {
        // Check threshold ranges
        if !(0.0..=1.0).contains(&self.auto_approve_threshold) {
            return Err(format!(
                "auto_approve_threshold must be between 0.0 and 1.0, got {}",
                self.auto_approve_threshold
            ));
        }
        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            return Err(format!(
                "confidence_threshold must be between 0.0 and 1.0, got {}",
                self.confidence_threshold
            ));
        }

        // Check LLM requirement
        if self.mode.requires_llm() && self.llm.is_none() {
            return Err(format!(
                "Mode {:?} requires LLM configuration, but none provided",
                self.mode
            ));
        }

        // Validate LLM config if present
        if let Some(llm) = &self.llm {
            if llm.model.is_empty() {
                return Err("LLM model name cannot be empty".to_string());
            }
            // API key is optional for some providers (e.g., local Ollama)
            if llm.provider != LlmProvider::Ollama && llm.api_key.is_none() {
                // Check if api_base is set (might be using a proxy that doesn't need key)
                if llm.api_base.is_none() {
                    return Err(format!(
                        "LLM provider {:?} requires api_key or api_base",
                        llm.provider
                    ));
                }
            }
        }

        // Validate file enrichment config
        if !(0.0..=1.0).contains(&self.file_enrichment.min_score_threshold) {
            return Err(format!(
                "file_enrichment.min_score_threshold must be between 0.0 and 1.0, got {}",
                self.file_enrichment.min_score_threshold
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_defaults_to_dumb_only() {
        let config = TextBrainConfig::default();
        assert_eq!(config.mode, TextBrainMode::DumbOnly);
    }

    #[test]
    fn test_mode_requires_llm() {
        assert!(!TextBrainMode::DumbOnly.requires_llm());
        assert!(!TextBrainMode::DumbFirst.requires_llm());
        assert!(!TextBrainMode::LlmFirst.requires_llm());
        assert!(TextBrainMode::LlmOnly.requires_llm());
    }

    #[test]
    fn test_mode_can_use_llm() {
        assert!(!TextBrainMode::DumbOnly.can_use_llm());
        assert!(TextBrainMode::DumbFirst.can_use_llm());
        assert!(TextBrainMode::LlmFirst.can_use_llm());
        assert!(TextBrainMode::LlmOnly.can_use_llm());
    }

    #[test]
    fn test_mode_can_use_dumb() {
        assert!(TextBrainMode::DumbOnly.can_use_dumb());
        assert!(TextBrainMode::DumbFirst.can_use_dumb());
        assert!(TextBrainMode::LlmFirst.can_use_dumb());
        assert!(!TextBrainMode::LlmOnly.can_use_dumb());
    }

    #[test]
    fn test_config_validation_valid() {
        let config = TextBrainConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_threshold() {
        let config = TextBrainConfig {
            auto_approve_threshold: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_llm_required() {
        let config = TextBrainConfig {
            mode: TextBrainMode::LlmOnly,
            llm: None,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_llm_with_api_base() {
        let config = TextBrainConfig {
            mode: TextBrainMode::LlmFirst,
            llm: Some(LlmConfig {
                provider: LlmProvider::Anthropic,
                model: "claude-3-haiku".to_string(),
                api_key: None,
                api_base: Some("http://localhost:5000".to_string()),
                timeout_secs: 30,
                max_tokens: 1024,
            }),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let toml = r#"
mode = "dumb_first"
auto_approve_threshold = 0.9

[llm]
provider = "anthropic"
model = "claude-3-haiku-20240307"
api_key = "sk-test"
"#;
        let config: TextBrainConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.mode, TextBrainMode::DumbFirst);
        assert_eq!(config.auto_approve_threshold, 0.9);
        assert!(config.llm.is_some());

        let llm = config.llm.unwrap();
        assert_eq!(llm.provider, LlmProvider::Anthropic);
        assert_eq!(llm.model, "claude-3-haiku-20240307");
    }

    #[test]
    fn test_ollama_no_api_key_required() {
        let config = TextBrainConfig {
            mode: TextBrainMode::LlmFirst,
            llm: Some(LlmConfig {
                provider: LlmProvider::Ollama,
                model: "llama2".to_string(),
                api_key: None,
                api_base: None,
                timeout_secs: 60,
                max_tokens: 2048,
            }),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}
