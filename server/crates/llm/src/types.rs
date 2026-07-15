use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputConfig {
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub usage: Usage,
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
}

impl MessagesResponse {
    pub fn text(&self) -> Option<String> {
        self.content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.clone())
    }
}

/// Rough cost estimate (USD) based on model tier pricing from DEV.md §8.1.
pub fn estimate_cost_usd(model: &str, usage: &Usage) -> f64 {
    let (in_rate, out_rate) = if model.contains("haiku") {
        (1.0, 5.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else {
        // opus default
        (5.0, 25.0)
    };
    let input = usage.input_tokens as f64
        + usage.cache_read_input_tokens.unwrap_or(0) as f64 * 0.1
        + usage.cache_creation_input_tokens.unwrap_or(0) as f64 * 1.25;
    (input * in_rate + usage.output_tokens as f64 * out_rate) / 1_000_000.0
}
