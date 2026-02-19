use merab_core::artifact::ArtifactLog;
use serde::{Deserialize, Serialize};

/// Role in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// Configuration for the AI client.
#[derive(Debug, Clone)]
pub struct AiClientConfig {
    pub proxy_url: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    /// Si se configura, se envía como `Authorization: Bearer <key>` al proxy.
    /// El proxy lo pasa al upstream en vez de usar su propia key global.
    pub api_key: Option<String>,
}

/// Request body sent to the LLM proxy (OpenAI-compatible).
#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// A single choice returned by the LLM.
#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatChoiceMessage,
}

/// The message content within a choice.
#[derive(Debug, Deserialize)]
pub struct ChatChoiceMessage {
    pub role: String,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
    pub model: Option<String>,
    pub usage: Option<ApiResponseUsage>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponseUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: Option<u64>,
}

/// Parsed tool call extracted from LLM response content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub content: String,
    pub model: Option<String>,
    pub tool_call: Option<ToolCall>,
    #[serde(default)]
    pub artifacts: Option<ArtifactLog>,
    #[serde(default)]
    pub usage: Option<merab_core::TokenUsage>,
}
