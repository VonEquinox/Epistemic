use serde::{Deserialize, Serialize};

/// OpenAI-style multimodal content: plain string or content parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlBody },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlBody {
    /// `data:image/png;base64,...` or https URL
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn user_with_images(text: impl Into<String>, data_urls: &[String]) -> Self {
        let mut parts = vec![ContentPart::Text {
            text: text.into(),
        }];
        for url in data_urls {
            parts.push(ContentPart::ImageUrl {
                image_url: ImageUrlBody {
                    url: url.clone(),
                    detail: Some("high".into()),
                },
            });
        }
        Self {
            role: "user".into(),
            content: MessageContent::Parts(parts),
        }
    }
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
    /// Some gateways return string; multimodal replies are still text in content.
    #[serde(default, deserialize_with = "deserialize_content_opt")]
    pub content: Option<String>,
}

fn deserialize_content_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Array(parts) => {
            let mut out = String::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(|x| x.as_str()) {
                    out.push_str(t);
                }
            }
            if out.is_empty() {
                Ok(None)
            } else {
                Ok(Some(out))
            }
        }
        other => Ok(Some(other.to_string())),
    }
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
        (1.0, 3.0)
    };
    let input = usage.input_tokens as f64
        + usage.cache_read_input_tokens.unwrap_or(0) as f64 * 0.1
        + usage.cache_creation_input_tokens.unwrap_or(0) as f64 * 1.25;
    (input * in_rate + usage.output_tokens as f64 * out_rate) / 1_000_000.0
}
