use crate::types::*;
use reqwest::Client;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api {status}: {body}")]
    Api { status: u16, body: String },
    #[error("no text content in response")]
    EmptyContent,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct ClaudeClient {
    http: Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl ClaudeClient {
    pub fn new(api_key: impl Into<String>, default_model: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("reqwest client");
        Self {
            http,
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com".into(),
            default_model: default_model.into(),
        }
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
        let model =
            std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-opus-4-8".into());
        Ok(Self::new(key, model))
    }

    pub fn model(&self) -> &str {
        &self.default_model
    }

    pub(crate) fn http(&self) -> &Client {
        &self.http
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn messages(&self, req: MessagesRequest) -> Result<MessagesResponse, LlmError> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let resp = self
                .http
                .post(format!("{}/v1/messages", self.base_url))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&req)
                .send()
                .await?;

            let status = resp.status();
            if status.as_u16() == 429 || status.is_server_error() {
                if attempt >= 5 {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(LlmError::Api {
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
                tracing::warn!(attempt, retry_after, %status, "LLM retry");
                tokio::time::sleep(Duration::from_secs(retry_after.min(60))).await;
                continue;
            }

            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(LlmError::Api {
                    status: status.as_u16(),
                    body,
                });
            }

            return Ok(resp.json().await?);
        }
    }

    /// Convenience: structured JSON output via output_config.format json_schema.
    pub async fn complete_json(
        &self,
        system: &str,
        user: &str,
        schema: serde_json::Value,
        max_tokens: u32,
    ) -> Result<(serde_json::Value, Usage, String), LlmError> {
        let req = MessagesRequest {
            model: self.default_model.clone(),
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
        };
        let resp = self.messages(req).await?;
        let text = resp.text().ok_or(LlmError::EmptyContent)?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| LlmError::Other(anyhow::anyhow!("json parse: {e}; text={text}")))?;
        Ok((value, resp.usage, resp.model))
    }
}
