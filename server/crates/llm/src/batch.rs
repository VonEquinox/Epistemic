//! OpenAI Batch API for bulk Chat Completions (typically ~50% cost).
//!
//! Flow: upload JSONL → create batch → poll → download output JSONL.
//! Compatible gateways that only implement `/chat/completions` may not support
//! this path; interactive jobs always use the sync client.

use crate::client::{LlmClient, LlmError};
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct BatchRequestItem {
    pub custom_id: String,
    /// Chat Completions body (placed under JSONL `body`).
    pub params: ChatCompletionRequest,
}

#[derive(Debug, Clone, Serialize)]
struct BatchJsonlRow<'a> {
    custom_id: &'a str,
    method: &'static str,
    url: &'static str,
    body: &'a ChatCompletionRequest,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchHandle {
    pub id: String,
    /// OpenAI: validating | in_progress | finalizing | completed | failed | ...
    /// Normalized: we also accept legacy Anthropic `ended`.
    pub status: String,
    #[serde(default)]
    pub output_file_id: Option<String>,
    #[serde(default)]
    pub error_file_id: Option<String>,
    #[serde(default)]
    pub request_counts: Option<serde_json::Value>,
}

impl BatchHandle {
    /// True when results are ready to download.
    pub fn is_ended(&self) -> bool {
        matches!(
            self.status.as_str(),
            "completed" | "ended" | "failed" | "expired" | "cancelled"
        )
    }

    pub fn is_success_ended(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "ended")
    }

    /// Alias used by older call sites that read `processing_status`.
    pub fn processing_status(&self) -> &str {
        if self.is_success_ended() {
            "ended"
        } else if self.is_ended() {
            self.status.as_str()
        } else {
            self.status.as_str()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileUploadResponse {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateBatchResponse {
    id: String,
    status: String,
    #[serde(default)]
    output_file_id: Option<String>,
    #[serde(default)]
    error_file_id: Option<String>,
    #[serde(default)]
    request_counts: Option<serde_json::Value>,
}

/// Normalized batch result line for apply path.
#[derive(Debug, Clone)]
pub struct BatchResultLine {
    pub custom_id: String,
    pub result: BatchResultBody,
}

#[derive(Debug, Clone)]
pub struct BatchResultBody {
    pub result_type: String,
    pub message: Option<CompletionMessage>,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiBatchOutLine {
    custom_id: String,
    #[serde(default)]
    response: Option<OpenAiBatchResponseEnvelope>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiBatchResponseEnvelope {
    #[serde(default)]
    status_code: u16,
    #[serde(default)]
    body: Option<ChatCompletionResponse>,
}

impl LlmClient {
    /// Submit a batch of structured Chat Completions requests.
    pub async fn create_batch(
        &self,
        items: Vec<BatchRequestItem>,
    ) -> Result<BatchHandle, LlmError> {
        if items.is_empty() {
            return Err(LlmError::Other(anyhow::anyhow!("empty batch")));
        }
        if items.len() > 50_000 {
            return Err(LlmError::Other(anyhow::anyhow!("batch exceeds 50k cap")));
        }

        // 1) Build JSONL
        let mut jsonl = String::new();
        for it in &items {
            let row = BatchJsonlRow {
                custom_id: &it.custom_id,
                method: "POST",
                url: "/v1/chat/completions",
                body: &it.params,
            };
            jsonl.push_str(&serde_json::to_string(&row).map_err(|e| {
                LlmError::Other(anyhow::anyhow!("serialize batch row: {e}"))
            })?);
            jsonl.push('\n');
        }

        // 2) Upload file
        let file_part = reqwest::multipart::Part::bytes(jsonl.into_bytes())
            .file_name("batch_input.jsonl")
            .mime_str("application/jsonl")
            .map_err(|e| LlmError::Other(anyhow::anyhow!("mime: {e}")))?;
        let form = reqwest::multipart::Form::new()
            .text("purpose", "batch")
            .part("file", file_part);

        let upload = self
            .http()
            .post(format!("{}/files", self.base_url()))
            .header("Authorization", format!("Bearer {}", self.api_key()))
            .multipart(form)
            .send()
            .await?;
        let status = upload.status();
        if !status.is_success() {
            let body = upload.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }
        let uploaded: FileUploadResponse = upload.json().await?;

        // 3) Create batch
        let create_body = serde_json::json!({
            "input_file_id": uploaded.id,
            "endpoint": "/v1/chat/completions",
            "completion_window": "24h",
        });
        let resp = self
            .http()
            .post(format!("{}/batches", self.base_url()))
            .header("Authorization", format!("Bearer {}", self.api_key()))
            .header("content-type", "application/json")
            .json(&create_body)
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
        let created: CreateBatchResponse = resp.json().await?;
        Ok(BatchHandle {
            id: created.id,
            status: created.status,
            output_file_id: created.output_file_id,
            error_file_id: created.error_file_id,
            request_counts: created.request_counts,
        })
    }

    pub async fn get_batch(&self, batch_id: &str) -> Result<BatchHandle, LlmError> {
        let resp = self
            .http()
            .get(format!("{}/batches/{batch_id}", self.base_url()))
            .header("Authorization", format!("Bearer {}", self.api_key()))
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
        let created: CreateBatchResponse = resp.json().await?;
        Ok(BatchHandle {
            id: created.id,
            status: created.status,
            output_file_id: created.output_file_id,
            error_file_id: created.error_file_id,
            request_counts: created.request_counts,
        })
    }

    /// Download and parse batch output JSONL into normalized lines.
    pub async fn batch_results(
        &self,
        batch_id: &str,
    ) -> Result<Vec<BatchResultLine>, LlmError> {
        let handle = self.get_batch(batch_id).await?;
        let file_id = handle.output_file_id.ok_or_else(|| {
            LlmError::Other(anyhow::anyhow!(
                "batch {batch_id} has no output_file_id (status={})",
                handle.status
            ))
        })?;

        let resp = self
            .http()
            .get(format!("{}/files/{file_id}/content", self.base_url()))
            .header("Authorization", format!("Bearer {}", self.api_key()))
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
            let row: OpenAiBatchOutLine = serde_json::from_str(line).map_err(|e| {
                LlmError::Other(anyhow::anyhow!("batch jsonl: {e}; line={line}"))
            })?;
            let (result_type, message, error) = if let Some(err) = row.error {
                ("errored".into(), None, Some(err))
            } else if let Some(env) = row.response {
                if env.status_code >= 200 && env.status_code < 300 {
                    let msg = env.body.and_then(CompletionMessage::from_chat);
                    ("succeeded".into(), msg, None)
                } else {
                    (
                        "errored".into(),
                        None,
                        Some(serde_json::json!({ "status_code": env.status_code })),
                    )
                }
            } else {
                ("errored".into(), None, Some(serde_json::json!("empty")))
            };
            out.push(BatchResultLine {
                custom_id: row.custom_id,
                result: BatchResultBody {
                    result_type,
                    message,
                    error,
                },
            });
        }
        Ok(out)
    }

    pub async fn wait_batch(
        &self,
        batch_id: &str,
        poll_every: Duration,
        max_wait: Duration,
    ) -> Result<BatchHandle, LlmError> {
        let start = std::time::Instant::now();
        loop {
            let h = self.get_batch(batch_id).await?;
            if h.is_ended() {
                return Ok(h);
            }
            if start.elapsed() > max_wait {
                return Err(LlmError::Other(anyhow::anyhow!(
                    "batch {batch_id} timed out after {:?}",
                    max_wait
                )));
            }
            tracing::info!(batch_id, status = %h.status, "waiting for batch");
            tokio::time::sleep(poll_every).await;
        }
    }
}
