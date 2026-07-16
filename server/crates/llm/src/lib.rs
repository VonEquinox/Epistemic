//! OpenAI-compatible Chat Completions client + prompt helpers.
//!
//! Protocol: `POST {base}/chat/completions` with Bearer auth.
//! Multimodal: user message content parts may include `image_url` (VLM).
//! Structured output via `response_format.json_schema`.

pub mod batch;
pub mod client;
pub mod embed;
pub mod types;

pub use batch::*;
pub use client::{image_data_url, LlmClient};
pub use embed::{vector_literal, EmbeddingClient};
pub use types::*;

/// Backward-compatible alias (pre-Chat Completions rename).
pub type ClaudeClient = LlmClient;
