//! TextBrain - Query building, searching, and matching intelligence.
//!
//! This module provides the central intelligence for torrent acquisition:
//! - Generating search queries from tickets
//! - Scoring search results against ticket requirements
//! - Mapping torrent files to ticket items (future)
//!
//! It coordinates between "dumb" heuristic-based methods and optional LLM-powered intelligence.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                          TextBrain                               │
//! │                                                                  │
//! │  Config: mode = dumb-only | dumb-first | llm-first | llm-only   │
//! │                                                                  │
//! │  ┌─────────────────────────────────────────────────────────────┐│
//! │  │                    QueryBuilder Trait                        ││
//! │  │  ┌─────────────────┐        ┌─────────────────┐             ││
//! │  │  │ DumbQueryBuilder│        │  LlmQueryBuilder│             ││
//! │  │  └─────────────────┘        └─────────────────┘             ││
//! │  └─────────────────────────────────────────────────────────────┘│
//! │                                                                  │
//! │  ┌─────────────────────────────────────────────────────────────┐│
//! │  │                   CandidateMatcher Trait                     ││
//! │  │  ┌─────────────────┐        ┌─────────────────┐             ││
//! │  │  │   DumbMatcher   │        │   LlmMatcher    │             ││
//! │  │  └─────────────────┘        └─────────────────┘             ││
//! │  └─────────────────────────────────────────────────────────────┘│
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Modes
//!
//! - **DumbOnly**: Use only heuristic methods, no LLM. Fastest, works offline.
//! - **DumbFirst**: Try heuristics, use LLM if confidence is low. Good balance.
//! - **LlmFirst**: Use LLM first, fall back to heuristics on error. Best accuracy.
//! - **LlmOnly**: Use only LLM, fail if unavailable. Maximum accuracy.
//!
//! # Example
//!
//! ```ignore
//! use torrentino_core::textbrain::{TextBrain, TextBrainConfig, TextBrainMode};
//!
//! let config = TextBrainConfig {
//!     mode: TextBrainMode::DumbFirst,
//!     auto_approve_threshold: 0.85,
//!     ..Default::default()
//! };
//!
//! let brain = TextBrain::new(config)
//!     .with_dumb_query_builder(my_query_builder)
//!     .with_dumb_matcher(my_matcher);
//!
//! let result = brain.acquire(&query_context, &searcher).await?;
//! if result.auto_approved {
//!     // Start download
//! } else {
//!     // Queue for manual approval
//! }
//! ```

mod config;
mod coordinator;
mod dumb_query_builder;
mod llm;
mod traits;
mod types;

// LLM client types
pub use llm::{
    AnthropicClient, CompletionRequest, CompletionResponse, LlmClient, LlmError, LlmUsage,
};

// Configuration types
pub use config::{LlmConfig, LlmProvider, TextBrainConfig, TextBrainMode};

// Core traits
pub use traits::{CandidateMatcher, QueryBuilder, TextBrainError};

// Dumb implementations
pub use dumb_query_builder::{DumbQueryBuilder, DumbQueryBuilderConfig};

// Result types
pub use types::{
    AcquisitionResult, FileMapping, MatchResult, QueryBuildResult, ScoredCandidate,
    ScoredCandidateSummary,
};

// The coordinator
pub use coordinator::TextBrain;
