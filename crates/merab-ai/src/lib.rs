pub mod client;
pub mod error;
pub mod retry;
mod tests;
pub mod types;

pub use client::AiClient;
pub use error::AiError;
pub use retry::RetryConfig;
pub use types::{
    AiClientConfig, AiResponse, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
    ChatRole, ToolCall,
};
