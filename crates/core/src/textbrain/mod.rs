//! TextBrain - Query building and matching intelligence.
//!
//! This module provides the central intelligence for:
//! - Generating search queries from tickets
//! - Scoring search results against tickets
//! - Mapping torrent files to ticket items
//!
//! It coordinates between "dumb" heuristic-based methods and optional LLM-powered intelligence.

mod llm;

pub use llm::{
    AnthropicClient, CompletionRequest, CompletionResponse, LlmClient, LlmError, LlmUsage,
};
