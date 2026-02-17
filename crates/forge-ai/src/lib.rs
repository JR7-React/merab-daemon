pub mod types;
pub mod error;
pub mod client;
mod tests;

pub use client::AiClient;
pub use error::AiError;
pub use types::{
    AiClientConfig, AiResponse, ChatCompletionRequest, ChatCompletionResponse,
    ChatMessage, ChatRole, ToolCall,
};
