use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// OpenAI Chat Completions request body (`POST /v1/chat/completions`).
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<JsonSchemaSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSchemaSpec {
    pub name: String,
    pub strict: bool,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub message: ChoiceMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChoiceMessage {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
}

/// Token usage — accepts both OpenAI (`prompt_tokens`) and legacy Anthropic names.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Usage {
    #[serde(default, alias = "prompt_tokens")]
    pub input_tokens: u64,
    #[serde(default, alias = "completion_tokens")]
    pub output_tokens: u64,
    #[serde(default, alias = "total_tokens")]
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
}

impl ChatCompletionResponse {
    pub fn text(&self) -> Option<String> {
        self.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .filter(|s| !s.is_empty())
    }
}

/// Normalized completion result used by batch apply path.
#[derive(Debug, Clone)]
pub struct CompletionMessage {
    pub model: String,
    pub usage: Usage,
    pub content: String,
}

impl CompletionMessage {
    pub fn text(&self) -> Option<String> {
        if self.content.is_empty() {
            None
        } else {
            Some(self.content.clone())
        }
    }

    pub fn from_chat(resp: ChatCompletionResponse) -> Option<Self> {
        let content = resp.text()?;
        Some(Self {
            model: resp.model,
            usage: resp.usage,
            content,
        })
    }
}

/// Rough cost estimate (USD). OpenAI-class defaults; override with real billing later.
pub fn estimate_cost_usd(model: &str, usage: &Usage) -> f64 {
    let m = model.to_ascii_lowercase();
    let (in_rate, out_rate) = if m.contains("gpt-4o-mini") || m.contains("mini") {
        (0.15, 0.60)
    } else if m.contains("gpt-4o") || m.contains("gpt-4.1") {
        (2.50, 10.0)
    } else if m.contains("haiku") {
        (1.0, 5.0)
    } else if m.contains("sonnet") {
        (3.0, 15.0)
    } else if m.contains("opus") {
        (5.0, 25.0)
    } else {
        // generic chat model default
        (1.0, 3.0)
    };
    let input = usage.input_tokens as f64
        + usage.cache_read_input_tokens.unwrap_or(0) as f64 * 0.1
        + usage.cache_creation_input_tokens.unwrap_or(0) as f64 * 1.25;
    (input * in_rate + usage.output_tokens as f64 * out_rate) / 1_000_000.0
}
