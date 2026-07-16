//! OpenAI-compatible Embeddings client (`POST /v1/embeddings`).
//!
//! SiliconFlow / OpenAI / vLLM gateways: Bearer auth, float vectors.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api {status}: {body}")]
    Api { status: u16, body: String },
    #[error("empty embedding response")]
    Empty,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct EmbeddingClient {
    http: Client,
    api_key: String,
    /// Base including `/v1`
    base_url: String,
    model: String,
    /// Expected output dim (passed as `dimensions` when set > 0)
    dimensions: Option<u32>,
}

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: EmbedInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding_format: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum EmbedInput {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Debug, Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

impl EmbeddingClient {
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
        dimensions: Option<u32>,
    ) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("reqwest");
        let mut base = base_url.into().trim().trim_end_matches('/').to_string();
        if !base.ends_with("/v1") {
            base.push_str("/v1");
        }
        Self {
            http,
            api_key: api_key.into(),
            base_url: base,
            model: model.into(),
            dimensions,
        }
    }

    /// Env:
    /// - `EMBEDDING_API_KEY` (fallback: `SILICONFLOW_API_KEY`)
    /// - `EMBEDDING_BASE_URL` (default `https://api.siliconflow.cn/v1`)
    /// - `EMBEDDING_MODEL` (default `Qwen/Qwen3-Embedding-8B`)
    /// - `EMBEDDING_DIM` expected length check (default unset)
    /// - `EMBEDDING_SEND_DIM=1` also pass `dimensions` in the request body
    pub fn from_env() -> anyhow::Result<Self> {
        let key = std::env::var("EMBEDDING_API_KEY")
            .or_else(|_| std::env::var("SILICONFLOW_API_KEY"))
            .map_err(|_| anyhow::anyhow!("EMBEDDING_API_KEY (or SILICONFLOW_API_KEY) not set"))?;
        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "Qwen/Qwen3-Embedding-8B".into());
        let base = std::env::var("EMBEDDING_BASE_URL")
            .unwrap_or_else(|_| "https://api.siliconflow.cn/v1".into());
        // Expected dim for local checks / docs. Only sent to the API when
        // EMBEDDING_SEND_DIM=1 (some gateways reject `dimensions`).
        let expected = std::env::var("EMBEDDING_DIM")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&d: &u32| d > 0);
        let send = std::env::var("EMBEDDING_SEND_DIM")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let dimensions = if send { expected } else { None };
        let mut client = Self::new(key, model, base, dimensions);
        // Keep expected dim for validation even when not sent.
        if !send {
            client.dimensions = expected;
        }
        Ok(client)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn dimensions(&self) -> Option<u32> {
        self.dimensions
    }

    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let mut v = self.embed_many(&[text.to_string()]).await?;
        v.pop().ok_or(EmbedError::Empty)
    }

    pub async fn embed_many(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let req = EmbedRequest {
            model: self.model.clone(),
            input: if texts.len() == 1 {
                EmbedInput::One(texts[0].clone())
            } else {
                EmbedInput::Many(texts.to_vec())
            },
            encoding_format: Some("float"),
            dimensions: self.dimensions,
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let resp = self
                .http
                .post(format!("{}/embeddings", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("content-type", "application/json")
                .json(&req)
                .send()
                .await?;

            let status = resp.status();
            if status.as_u16() == 429 || status.is_server_error() {
                if attempt >= 5 {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(EmbedError::Api {
                        status: status.as_u16(),
                        body,
                    });
                }
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2u64.pow(attempt));
                tracing::warn!(attempt, retry_after, %status, "embedding retry");
                tokio::time::sleep(Duration::from_secs(retry_after.min(60))).await;
                continue;
            }

            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(EmbedError::Api {
                    status: status.as_u16(),
                    body,
                });
            }

            let body: EmbedResponse = resp.json().await?;
            if body.data.is_empty() {
                return Err(EmbedError::Empty);
            }
            let mut data = body.data;
            data.sort_by_key(|d| d.index);
            return Ok(data.into_iter().map(|d| d.embedding).collect());
        }
    }
}

/// Format float vec for Postgres `vector` / pgvector literal.
pub fn vector_literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 12 + 2);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // compact but stable
        s.push_str(&format!("{x}"));
    }
    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_shape() {
        let s = vector_literal(&[1.0, -0.5]);
        assert_eq!(s, "[1,-0.5]");
    }
}
