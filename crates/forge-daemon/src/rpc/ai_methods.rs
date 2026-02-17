use std::sync::Arc;

use forge_ai::{AiClient, AiClientConfig, AiResponse, ChatMessage};
use forge_config::ForgeConfig;
use forge_core::ForgeError;
use jsonrpsee::types::ErrorObjectOwned;

use super::server::to_rpc_error;
use crate::mcp_manager::McpManager;
use crate::prompts::ENGINEER_SYSTEM_PROMPT;
use forge_core::multi_agent_pipeline::Planner;


/// Build an AiClient from the daemon's config.
pub fn build_ai_client(config: &ForgeConfig) -> Result<AiClient, ErrorObjectOwned> {
    let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);

    let ai_config = AiClientConfig {
        proxy_url,
        model: config.ai.model.clone(),
        system_prompt: Some(ENGINEER_SYSTEM_PROMPT.to_string()),
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

    // Inject Internal Tools
    prompt.push_str("- **core.plan**: Decompose a complex task into a structured plan of subtasks. Use this as the FIRST step for any complex request.\n");
    prompt.push_str("  - `task`: The description of the task to decompose (required)\n");

    for tool in &tools {
        prompt.push_str(&format!("- **{}**: {}\n", tool.name, tool.description));
        if let Some(props) = tool.input_schema.get("properties").and_then(|p| p.as_object()) {
            let params: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
            let required: Vec<&str> = tool.input_schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            for param in &params {
                let req_marker = if required.contains(param) { " (required)" } else { "" };
                let desc = props.get(*param)
                    .and_then(|p| p.get("description"))
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                prompt.push_str(&format!("  - `{}`: {}{}\n", param, desc, req_marker));
            }
        }
    }

    prompt
}

/// Handle `forge.ai.chat` — single-step LLM call.
pub async fn handle_ai_chat(
    config: &ForgeConfig,
    mcp_manager: &Arc<McpManager>,
    message: String,
    context: Vec<ChatMessage>,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;

    let dynamic_prompt = build_dynamic_system_prompt(ENGINEER_SYSTEM_PROMPT, mcp_manager).await;
    client.set_system_prompt(dynamic_prompt);

    let mut messages = context;
    messages.push(ChatMessage::user(message));

    client
        .chat(messages)
        .await
        .map_err(|e| to_rpc_error(ForgeError::AiError(e.to_string())))
}

/// Handle `forge.ai.executeTool` — execute a tool by name via McpManager.
pub async fn handle_execute_tool(
    mcp_manager: &Arc<McpManager>,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let mcp_client = mcp_manager
        .find_agent_for_tool(&tool_name)
        .await
        .ok_or_else(|| to_rpc_error(ForgeError::ToolNotFound(tool_name.clone())))?;

    let args = match arguments {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    };

    let tool_result = mcp_client
        .call_tool(tool_name.clone(), args)
        .await
        .map_err(|e| {
            to_rpc_error(ForgeError::AiError(format!(
                "tool '{}' failed: {}",
                tool_name, e
            )))
        })?;

    serde_json::to_value(&tool_result)
        .map_err(|e| to_rpc_error(ForgeError::Internal(e.to_string())))
}

/// Handle `forge.ai.orchestrate` — multi-step orchestration loop.
pub async fn handle_ai_orchestrate(
    config: &ForgeConfig,
    mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;
    let max_steps = config.ai.max_orchestration_steps;

    let dynamic_prompt = build_dynamic_system_prompt(ENGINEER_SYSTEM_PROMPT, mcp_manager).await;
    client.set_system_prompt(dynamic_prompt);

    let mut messages: Vec<ChatMessage> = vec![ChatMessage::user(&task)];

    for step in 0..max_steps {
        tracing::info!(step = step, max = max_steps, "orchestration step");

        let response = client
            .chat(messages.clone())
            .await
            .map_err(|e| to_rpc_error(ForgeError::AiError(e.to_string())))?;

        if response.tool_call.is_none() {
            tracing::info!(steps = step + 1, "orchestration complete");
            return Ok(response);
        }

        let tool_call = response.tool_call.clone().unwrap();
        tracing::info!(
            step = step,
            tool = %tool_call.name,
            "orchestrator: executing tool"
        );

        messages.push(ChatMessage::assistant(&response.content));

        let tool_result_str = execute_tool_call(mcp_manager, &tool_call).await?;

        messages.push(ChatMessage::user(format!(
            "Tool '{}' returned:\n{}",
            tool_call.name, tool_result_str
        )));
    }

    tracing::warn!(max_steps = max_steps, "orchestration hit step limit");

    messages.push(ChatMessage::user(
        "You have reached the maximum number of steps. \
         Please provide a final summary of what was accomplished and what remains.",
    ));

    let final_response = client
        .chat(messages)
        .await
        .map_err(|e| to_rpc_error(ForgeError::AiError(e.to_string())))?;

    Ok(final_response)
}

/// Execute a single tool call via MCP and return the result as a string.
async fn execute_tool_call(
    mcp_manager: &Arc<McpManager>,
    tool_call: &forge_ai::ToolCall,
) -> Result<String, ErrorObjectOwned> {
    // Handle Internal Tools
    if tool_call.name == "core.plan" {
        let task = tool_call.arguments.get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| to_rpc_error(ForgeError::InvalidInput("Missing 'task' argument for core.plan".into())))?;
        
        let planner = Planner::new();
        let plan = planner.decompose(task);
        return serde_json::to_string(&plan)
            .map_err(|e| to_rpc_error(ForgeError::Internal(e.to_string())));
    }

    let mcp_client = mcp_manager
        .find_agent_for_tool(&tool_call.name)
        .await
        .ok_or_else(|| to_rpc_error(ForgeError::ToolNotFound(tool_call.name.clone())))?;

    let args = match &tool_call.arguments {
        serde_json::Value::Object(map) => Some(map.clone()),
        _ => None,
    };

    let tool_result = mcp_client
        .call_tool(tool_call.name.clone(), args)
        .await
        .map_err(|e| {
            to_rpc_error(ForgeError::AiError(format!(
                "tool '{}' failed: {}",
                tool_call.name, e
            )))
        })?;

    serde_json::to_string(&tool_result)
        .map_err(|e| to_rpc_error(ForgeError::Internal(e.to_string())))
}

/// Handle `forge.ai.plan` — decompose a task into a plan.
pub async fn handle_ai_plan(
    _config: &ForgeConfig,
    _mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let planner = Planner::new();
    let plan = planner.decompose(&task);
    
    serde_json::to_value(&plan)
        .map_err(|e| to_rpc_error(ForgeError::Internal(e.to_string())))
}