//! Anthropic Message Batches API (50% cost) — DEV.md §8.3.
//!
//! Used for bulk DNA extraction / citation classification on import.
//! Interactive single-paper adds still use the sync `messages` endpoint.

use crate::client::{ClaudeClient, LlmError};
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct BatchRequestItem {
    pub custom_id: String,
    pub params: MessagesRequest,
}

#[derive(Debug, Clone, Serialize)]
struct CreateBatchBody {
    requests: Vec<BatchRequestItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchHandle {
    pub id: String,
    pub processing_status: String,
    #[serde(default)]
    pub request_counts: Option<serde_json::Value>,
    #[serde(default)]
    pub results_url: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchResultLine {
    pub custom_id: String,
    pub result: BatchResultBody,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchResultBody {
    #[serde(rename = "type")]
    pub result_type: String,
    #[serde(default)]
    pub message: Option<MessagesResponse>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}

impl ClaudeClient {
    /// Submit a batch of structured JSON requests. `custom_id` should be job id.
    pub async fn create_batch(
        &self,
        items: Vec<BatchRequestItem>,
    ) -> Result<BatchHandle, LlmError> {
        if items.is_empty() {
            return Err(LlmError::Other(anyhow::anyhow!("empty batch")));
        }
        if items.len() > 100_000 {
            return Err(LlmError::Other(anyhow::anyhow!("batch exceeds 100k cap")));
        }
        let body = CreateBatchBody { requests: items };
        let resp = self
            .http()
            .post(format!("{}/v1/messages/batches", self.base_url()))
            .header("x-api-key", self.api_key())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp.json().await?)
    }

    pub async fn get_batch(&self, batch_id: &str) -> Result<BatchHandle, LlmError> {
        let resp = self
            .http()
            .get(format!("{}/v1/messages/batches/{batch_id}", self.base_url()))
            .header("x-api-key", self.api_key())
            .header("anthropic-version", "2023-06-01")
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp.json().await?)
    }

    /// Download JSONL results; returns one entry per custom_id.
    pub async fn batch_results(
        &self,
        batch_id: &str,
    ) -> Result<Vec<BatchResultLine>, LlmError> {
        let resp = self
            .http()
            .get(format!(
                "{}/v1/messages/batches/{batch_id}/results",
                self.base_url()
            ))
            .header("x-api-key", self.api_key())
            .header("anthropic-version", "2023-06-01")
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }
        let text = resp.text().await?;
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let row: BatchResultLine = serde_json::from_str(line)
                .map_err(|e| LlmError::Other(anyhow::anyhow!("batch jsonl: {e}; line={line}")))?;
            out.push(row);
        }
        Ok(out)
    }

    /// Poll until ended or timeout. Default poll every 30s, max ~2h.
    pub async fn wait_batch(
        &self,
        batch_id: &str,
        poll_every: Duration,
        max_wait: Duration,
    ) -> Result<BatchHandle, LlmError> {
        let start = std::time::Instant::now();
        loop {
            let h = self.get_batch(batch_id).await?;
            if h.processing_status == "ended" {
                return Ok(h);
            }
            if start.elapsed() > max_wait {
                return Err(LlmError::Other(anyhow::anyhow!(
                    "batch {batch_id} timed out after {:?}",
                    max_wait
                )));
            }
            tracing::info!(
                batch_id,
                status = %h.processing_status,
                "waiting for batch"
            );
            tokio::time::sleep(poll_every).await;
        }
    }

    /// Build a structured JSON MessagesRequest (shared by sync + batch).
    pub fn json_request(
        &self,
        system: &str,
        user: &str,
        schema: serde_json::Value,
        max_tokens: u32,
    ) -> MessagesRequest {
        MessagesRequest {
            model: self.model().to_string(),
            max_tokens,
            system: Some(serde_json::json!(system)),
            messages: vec![Message {
                role: "user".into(),
                content: user.into(),
            }],
            output_config: Some(OutputConfig {
                format: OutputFormat {
                    format_type: "json_schema".into(),
                    schema,
                },
            }),
        }
    }
}
