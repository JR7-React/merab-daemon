use serde::{Deserialize, Serialize};

/// MCP Tool Definition — describes a tool an agent exposes.
/// Based on Anthropic's Model Context Protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// MCP Resource — a piece of context an agent can provide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub mime_type: Option<String>,
}
