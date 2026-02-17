use std::sync::Arc;

use forge_ai::{AiClient, AiClientConfig, AiResponse, ChatMessage};
use forge_config::ForgeConfig;
use forge_core::ForgeError;
use jsonrpsee::types::ErrorObjectOwned;

use crate::mcp_manager::McpManager;
use super::server::to_rpc_error;

/// Build an AiClient from the daemon's config.
pub fn build_ai_client(config: &ForgeConfig) -> Result<AiClient, ErrorObjectOwned> {
    let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);

    let ai_config = AiClientConfig {
        proxy_url,
        model: config.ai.model.clone(),
        system_prompt: Some(config.ai.system_prompt.clone()),
        max_tokens: config.ai.max_tokens,
        temperature: config.ai.temperature,
    };

    Ok(AiClient::new(ai_config))
}

/// Build a dynamic system prompt that includes available MCP tools.
pub async fn build_dynamic_system_prompt(
    base_prompt: &str,
    mcp_manager: &Arc<McpManager>,
) -> String {
    let tools = mcp_manager.get_all_tools().await;

    if tools.is_empty() {
        return base_prompt.to_string();
    }

    let mut prompt = format!("{}\n\n## Available Tools\n\n", base_prompt);
    prompt.push_str("You can call tools by responding with JSON in this format:\n");
    prompt.push_str(r#"{"tool_call": {"name": "<tool_name>", "arguments": {<args>}}}"#);
    prompt.push_str("\n\nTools:\n");

    for tool in &tools {
        prompt.push_str(&format!("- **{}**: {}\n", tool.name, tool.description));
        prompt.push_str(&format!("  Schema: {}\n", tool.input_schema));
    }

    prompt
}

/// Handle `forge.ai.chat` — simple chat with context.
pub async fn handle_ai_chat(
    config: &ForgeConfig,
    mcp_manager: &Arc<McpManager>,
    message: String,
    context: Vec<ChatMessage>,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;

    let dynamic_prompt = build_dynamic_system_prompt(
        &config.ai.system_prompt,
        mcp_manager,
    )
    .await;
    client.set_system_prompt(dynamic_prompt);

    let mut messages = context;
    messages.push(ChatMessage::user(message));

    client
        .chat(messages)
        .await
        .map_err(|e| to_rpc_error(ForgeError::AiError(e.to_string())))
}

/// Handle `forge.ai.orchestrate` — LLM decides if a tool call is needed,
/// executes it via McpManager, then consolidates the result.
pub async fn handle_ai_orchestrate(
    config: &ForgeConfig,
    mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;

    let dynamic_prompt = build_dynamic_system_prompt(
        &config.ai.system_prompt,
        mcp_manager,
    )
    .await;
    client.set_system_prompt(dynamic_prompt);

    // Step 1: Ask LLM to analyze the task
    let first_response = client
        .ask(&task)
        .await
        .map_err(|e| to_rpc_error(ForgeError::AiError(e.to_string())))?;

    // Step 2: If LLM returned a tool call, execute it
    let tool_call = match &first_response.tool_call {
        Some(tc) => tc.clone(),
        None => return Ok(first_response),
    };

    tracing::info!(
        tool = %tool_call.name,
        "orchestrator: executing tool call"
    );

    let mcp_client = mcp_manager
        .find_agent_for_tool(&tool_call.name)
        .await
        .ok_or_else(|| {
            to_rpc_error(ForgeError::ToolNotFound(tool_call.name.clone()))
        })?;

    let args = match tool_call.arguments {
        serde_json::Value::Object(map) => Some(map),
        serde_json::Value::Null => None,
        _ => None,
    };

    let tool_result = mcp_client
        .call_tool(tool_call.name.clone(), args)
        .await
        .map_err(|e| to_rpc_error(ForgeError::AiError(format!(
            "tool '{}' failed: {}",
            tool_call.name, e
        ))))?;

    let tool_result_str = serde_json::to_string(&tool_result)
        .map_err(|e| to_rpc_error(ForgeError::Internal(e.to_string())))?;

    // Step 3: Send tool result back to LLM for consolidation
    let consolidation_messages = vec![
        ChatMessage::user(&task),
        ChatMessage::assistant(&first_response.content),
        ChatMessage::user(format!(
            "Tool '{}' returned:\n{}",
            tool_call.name, tool_result_str
        )),
    ];

    let final_response = client
        .chat(consolidation_messages)
        .await
        .map_err(|e| to_rpc_error(ForgeError::AiError(e.to_string())))?;

    Ok(final_response)
}
