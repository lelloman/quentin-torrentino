//! LLM client abstraction and implementations.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::time::Duration;

/// Error type for LLM operations.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("JSON error: {0}")]
    Json(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Not configured")]
    NotConfigured,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Request for a completion.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// System prompt (instructions for the model)
    pub system: Option<String>,
    /// User message
    pub prompt: String,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Temperature (0.0 = deterministic, 1.0 = creative)
    pub temperature: f32,
}

impl CompletionRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            system: None,
            prompt: prompt.into(),
            max_tokens: 1024,
            temperature: 0.0, // Deterministic by default for matching tasks
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }
}

/// Response from a completion.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// The generated text
    pub text: String,
    /// Token usage
    pub usage: LlmUsage,
    /// Model used
    pub model: String,
}

/// Trait for LLM clients.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Provider name (e.g., "anthropic", "openai", "ollama")
    fn provider(&self) -> &str;

    /// Model name (e.g., "claude-3-haiku-20240307")
    fn model(&self) -> &str;

    /// Send a completion request and get a text response.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// Send a completion request and parse the response as JSON.
    async fn complete_json<T: DeserializeOwned>(
        &self,
        request: CompletionRequest,
    ) -> Result<(T, LlmUsage), LlmError> {
        let response = self.complete(request).await?;
        let parsed: T = serde_json::from_str(&response.text)
            .map_err(|e| LlmError::Json(format!("{}: {}", e, response.text)))?;
        Ok((parsed, response.usage))
    }
}

// ============================================================================
// Anthropic Implementation
// ============================================================================

/// Anthropic API client.
pub struct AnthropicClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    api_base: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            api_base: "https://api.anthropic.com".to_string(),
        }
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicError {
    error: AnthropicErrorDetail,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorDetail {
    message: String,
}

#[async_trait]
impl LlmClient for AnthropicClient {
    fn provider(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let anthropic_request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens,
            system: request.system,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: request.prompt,
            }],
            temperature: if request.temperature == 0.0 {
                None // Anthropic treats 0 as default, so omit for deterministic
            } else {
                Some(request.temperature)
            },
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.api_base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        let status = response.status().as_u16();

        if status != 200 {
            let error_text = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<AnthropicError>(&error_text)
                .map(|e| e.error.message)
                .unwrap_or(error_text);
            return Err(LlmError::Api { status, message });
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Json(e.to_string()))?;

        let text = anthropic_response
            .content
            .into_iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(CompletionResponse {
            text,
            usage: LlmUsage {
                input_tokens: anthropic_response.usage.input_tokens,
                output_tokens: anthropic_response.usage.output_tokens,
            },
            model: anthropic_response.model,
        })
    }
}

// ============================================================================
// Ollama Implementation
// ============================================================================

/// Ollama API client for local LLM inference.
///
/// Connects to a local Ollama server (default: http://localhost:11434).
/// No API key required.
pub struct OllamaClient {
    client: reqwest::Client,
    model: String,
    api_base: String,
}

impl OllamaClient {
    /// Create a new Ollama client with the specified model.
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "llama3", "mistral", "codellama")
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.into(),
            api_base: "http://localhost:11434".to_string(),
        }
    }

    /// Set a custom API base URL.
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    response: String,
    #[serde(default)]
    #[allow(dead_code)]
    done: bool,
    /// Number of tokens in the response
    #[serde(default)]
    eval_count: u32,
    /// Number of tokens in the prompt
    #[serde(default)]
    prompt_eval_count: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorResponse {
    error: String,
}

#[async_trait]
impl LlmClient for OllamaClient {
    fn provider(&self) -> &str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let ollama_request = OllamaRequest {
            model: self.model.clone(),
            prompt: request.prompt,
            system: request.system,
            stream: false,
            options: Some(OllamaOptions {
                temperature: if request.temperature == 0.0 {
                    Some(0.0) // Ollama needs explicit 0 for deterministic
                } else {
                    Some(request.temperature)
                },
                num_predict: Some(request.max_tokens),
            }),
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.api_base))
            .header("content-type", "application/json")
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        let status = response.status().as_u16();

        if status != 200 {
            let error_text = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<OllamaErrorResponse>(&error_text)
                .map(|e| e.error)
                .unwrap_or(error_text);
            return Err(LlmError::Api { status, message });
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Json(e.to_string()))?;

        Ok(CompletionResponse {
            text: ollama_response.response,
            usage: LlmUsage {
                input_tokens: ollama_response.prompt_eval_count,
                output_tokens: ollama_response.eval_count,
            },
            model: ollama_response.model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request_builder() {
        let request = CompletionRequest::new("Hello")
            .with_system("You are helpful")
            .with_max_tokens(100)
            .with_temperature(0.5);

        assert_eq!(request.prompt, "Hello");
        assert_eq!(request.system, Some("You are helpful".to_string()));
        assert_eq!(request.max_tokens, 100);
        assert_eq!(request.temperature, 0.5);
    }

    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::new("llama3");
        assert_eq!(client.provider(), "ollama");
        assert_eq!(client.model(), "llama3");
    }

    #[test]
    fn test_ollama_client_custom_base() {
        let client = OllamaClient::new("mistral").with_api_base("http://remote-server:11434");
        assert_eq!(client.api_base, "http://remote-server:11434");
    }

    #[test]
    fn test_ollama_request_serialization() {
        let request = OllamaRequest {
            model: "llama3".to_string(),
            prompt: "Hello".to_string(),
            system: Some("Be helpful".to_string()),
            stream: false,
            options: Some(OllamaOptions {
                temperature: Some(0.7),
                num_predict: Some(100),
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"llama3\""));
        assert!(json.contains("\"stream\":false"));
        assert!(json.contains("\"temperature\":0.7"));
    }
}
