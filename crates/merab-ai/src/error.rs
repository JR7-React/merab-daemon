use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("invalid response from LLM: {0}")]
    InvalidResponse(String),

    #[error("no content in LLM response")]
    EmptyResponse,

    #[error("serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("AI client not configured: {0}")]
    NotConfigured(String),

    #[error("tool call failed: {0}")]
    ToolCallFailed(String),

    #[error("orchestration exceeded max steps ({0})")]
    MaxStepsExceeded(u32),
}
