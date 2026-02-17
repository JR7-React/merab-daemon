pub mod client;
pub mod error;
mod tests;
pub mod types;

pub use client::AiClient;
pub use error::AiError;
pub use types::{
    AiClientConfig, AiResponse, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
    ChatRole, ToolCall,
};
