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

/// OpenAI-compatible Chat Completions client (`/v1/chat/completions`).
///
/// Works with OpenAI, Azure OpenAI-compatible gateways, vLLM, OpenRouter, etc.
/// Env: `OPENAI_API_KEY` (or `LLM_API_KEY`), optional `OPENAI_BASE_URL` / `OPENAI_MODEL`.
#[derive(Clone)]
pub struct LlmClient {
    http: Client,
    api_key: String,
    /// Base including `/v1`, e.g. `https://api.openai.com/v1`
    base_url: String,
    default_model: String,
}

impl LlmClient {
    pub fn new(
        api_key: impl Into<String>,
        default_model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("reqwest client");
        let base = normalize_base_url(base_url.into());
        Self {
            http,
            api_key: api_key.into(),
            base_url: base,
            default_model: default_model.into(),
        }
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("LLM_API_KEY"))
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY (or LLM_API_KEY) not set"))?;
        let model = std::env::var("OPENAI_MODEL")
            .or_else(|_| std::env::var("LLM_MODEL"))
            .unwrap_or_else(|_| "gpt-4o".into());
        let base = std::env::var("OPENAI_BASE_URL")
            .or_else(|_| std::env::var("LLM_BASE_URL"))
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        Ok(Self::new(key, model, base))
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

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub async fn chat_completions(
        &self,
        req: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, LlmError> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let resp = self
                .http
                .post(format!("{}/chat/completions", self.base_url))
                .header("Authorization", self.auth_header())
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

    /// Structured JSON via OpenAI `response_format: json_schema` (strict).
    /// Falls back to parsing free-form JSON from content if the gateway
    /// returns plain text (some compatible proxies strip response_format).
    pub async fn complete_json(
        &self,
        system: &str,
        user: &str,
        schema: serde_json::Value,
        max_tokens: u32,
    ) -> Result<(serde_json::Value, Usage, String), LlmError> {
        let req = self.json_request(system, user, schema, max_tokens);
        let resp = self.chat_completions(req).await?;
        let text = resp.text().ok_or(LlmError::EmptyContent)?;
        let value: serde_json::Value = parse_json_lenient(&text).map_err(|e| {
            LlmError::Other(anyhow::anyhow!("json parse: {e}; text={text}"))
        })?;
        Ok((value, resp.usage, resp.model))
    }

    /// Build a Chat Completions request with json_schema response_format.
    pub fn json_request(
        &self,
        system: &str,
        user: &str,
        schema: serde_json::Value,
        max_tokens: u32,
    ) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.default_model.clone(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: system.into(),
                },
                Message {
                    role: "user".into(),
                    content: user.into(),
                },
            ],
            max_tokens: Some(max_tokens),
            temperature: None,
            response_format: Some(ResponseFormat {
                format_type: "json_schema".into(),
                json_schema: Some(JsonSchemaSpec {
                    name: "epistemic_structured".into(),
                    strict: true,
                    schema,
                }),
            }),
        }
    }
}

/// Ensure base URL has no trailing slash and ends with `/v1`.
fn normalize_base_url(raw: String) -> String {
    let mut s = raw.trim().trim_end_matches('/').to_string();
    if s.is_empty() {
        return "https://api.openai.com/v1".into();
    }
    // Accept both `https://api.openai.com` and `.../v1`
    if !s.ends_with("/v1") {
        s.push_str("/v1");
    }
    s
}

/// Parse JSON, stripping optional markdown fences.
fn parse_json_lenient(text: &str) -> Result<serde_json::Value, serde_json::Error> {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("```json") {
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        let rest = rest.strip_suffix("```").unwrap_or(rest).trim();
        return serde_json::from_str(rest);
    }
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        let rest = rest.strip_suffix("```").unwrap_or(rest).trim();
        return serde_json::from_str(rest);
    }
    serde_json::from_str(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_v1() {
        assert_eq!(
            normalize_base_url("https://api.openai.com".into()),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/".into()),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn parse_fenced_json() {
        let v = parse_json_lenient("```json\n{\"a\":1}\n```").unwrap();
        assert_eq!(v["a"], 1);
    }
}
