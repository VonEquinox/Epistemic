//! OpenAI-compatible Chat Completions client + prompt helpers.
//!
//! Protocol: `POST {base}/chat/completions` with Bearer auth.
//! Structured output via `response_format.json_schema`.

pub mod batch;
pub mod client;
pub mod types;

pub use batch::*;
pub use client::LlmClient;
pub use types::*;

/// Backward-compatible alias (pre-Chat Completions rename).
pub type ClaudeClient = LlmClient;
