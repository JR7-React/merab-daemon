use reqwest::Client as HttpClient;
use tracing;

use crate::error::AiError;
use crate::retry::RetryConfig;
use crate::types::{
    AiClientConfig, AiResponse, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
    ToolCall,
};
use merab_core::TokenUsage;

pub struct AiClient {
    http: HttpClient,
    config: AiClientConfig,
    system_prompt: Option<String>,
}

impl AiClient {
    pub fn new(config: AiClientConfig) -> Self {
        let system_prompt = config.system_prompt.clone();
        Self {
            http: HttpClient::new(),
            config,
            system_prompt,
        }
    }

    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<AiResponse, AiError> {
        let max_attempts = self.config.retry.max_attempts;
        let mut last_error = None;

        for attempt in 0..max_attempts {
            match self.chat_once(messages.clone()).await {
                Ok(response) => return Ok(response),
                Err(ref e) if RetryConfig::is_retriable(&e.to_string()) => {
                    let delay = self.config.retry.delay_for_attempt(attempt);
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis(),
                        "rate limited or service unavailable, retrying..."
                    );
                    tokio::time::sleep(delay).await;
                    last_error = Some(e.to_string());
                }
                Err(e) => return Err(e),
            }
        }

        Err(AiError::RateLimited {
            attempts: max_attempts,
            message: last_error.unwrap_or_else(|| "max attempts exceeded".into()),
        })
    }

    /// Single attempt with no retry logic. Returns the raw API result.
    pub async fn chat_once(&self, messages: Vec<ChatMessage>) -> Result<AiResponse, AiError> {
        let mut all_messages = Vec::new();

        if let Some(sys) = &self.system_prompt {
            all_messages.push(ChatMessage::system(sys.clone()));
        }
        all_messages.extend(messages);

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: all_messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };

        let url = format!(
            "{}/v1/chat/completions",
            self.config.proxy_url.trim_end_matches('/')
        );

        tracing::debug!(url = %url, model = %request.model, "sending chat request");

        let mut builder = self.http.post(&url).json(&request);
        if let Some(key) = &self.config.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }
        let resp = builder.send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::InvalidResponse(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let completion: ChatCompletionResponse = resp.json().await?;
        self.parse_response(completion)
    }

    /// One-shot question (convenience wrapper).
    pub async fn ask(&self, question: &str) -> Result<AiResponse, AiError> {
        self.chat(vec![ChatMessage::user(question)]).await
    }

    fn parse_response(&self, resp: ChatCompletionResponse) -> Result<AiResponse, AiError> {
        let choice = resp.choices.first().ok_or(AiError::EmptyResponse)?;
        let content = choice.message.content.clone().unwrap_or_default();

        let tool_call = Self::extract_tool_call(&content);

        let usage = resp.usage.map(|u| {
            TokenUsage::new(
                u.prompt_tokens,
                u.completion_tokens,
                resp.model.clone().unwrap_or_default(),
            )
        });

        Ok(AiResponse {
            content,
            model: resp.model,
            tool_call,
            artifacts: None,
            usage,
        })
    }

    /// Attempt to extract a `{"tool_call": {"name": "...", "arguments": {...}}}` from content.
    fn extract_tool_call(content: &str) -> Option<ToolCall> {
        // Try parsing the entire content as JSON with a tool_call field
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(tc) = val.get("tool_call") {
                if let Ok(tool_call) = serde_json::from_value::<ToolCall>(tc.clone()) {
                    return Some(tool_call);
                }
            }
        }

        // Try finding a JSON block within the content
        if let Some(start) = content.find("{\"tool_call\"") {
            if let Some(end) = content[start..].rfind('}') {
                let json_str = &content[start..start + end + 1];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(tc) = val.get("tool_call") {
                        if let Ok(tool_call) = serde_json::from_value::<ToolCall>(tc.clone()) {
                            return Some(tool_call);
                        }
                    }
                }
            }
        }

        // 1. Try to find a markdown code block with JSON
        if let Some(start) = content.find("```json") {
            if let Some(_end) = content[start..].find("```") {
                // Find the SECOND backtick set (closing)
                 if let Some(close) = content[start+7..].find("```") {
                     let json_str = &content[start+7..start+7+close].trim();
                      if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(tc) = val.get("tool_call") {
                            if let Ok(tool_call) = serde_json::from_value::<ToolCall>(tc.clone()) {
                                return Some(tool_call);
                            }
                        }
                    }
                 }
            }
        }

        // 2. Try parsing the entire content as JSON
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(tc) = val.get("tool_call") {
                if let Ok(tool_call) = serde_json::from_value::<ToolCall>(tc.clone()) {
                    return Some(tool_call);
                }
            }
        }

        // 3. Try finding a raw JSON block within the content (fallback)
        if let Some(start) = content.find("{\"tool_call\"") {
            if let Some(end) = content[start..].rfind('}') {
                let json_str = &content[start..start + end + 1];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(tc) = val.get("tool_call") {
                        if let Ok(tool_call) = serde_json::from_value::<ToolCall>(tc.clone()) {
                            return Some(tool_call);
                        }
                    }
                }
            }
        }

        None
    }
}
