use std::sync::Arc;

use merab_ai::{AiClient, AiClientConfig, AiResponse, ChatMessage};
use merab_config::MerabConfig;
use merab_core::multi_agent_pipeline::Task;
use merab_core::MerabError;
use merab_store::Database;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;

use super::server::to_rpc_error;
use crate::artifacts::ArtifactTracker;
use crate::mcp_manager::McpManager;
use crate::planner::PlannerAgent;
use crate::prompts::{get_persona_prompt, ENGINEER_SYSTEM_PROMPT};


/// Build an AiClient from the daemon's config.
pub fn build_ai_client(config: &MerabConfig) -> Result<AiClient, ErrorObjectOwned> {
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

/// Build a dynamic system prompt that includes project context and available MCP tools.
pub async fn build_dynamic_system_prompt(
    base_prompt: &str,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
) -> String {
    let mut prompt = base_prompt.to_string();

    // Inject project context from shared memory
    {
        let store = db.lock().await;
        if let Ok(Some(serde_json::Value::String(ctx))) = store.get_memory("project.context") {
            prompt.push_str("\n\n## Project Context\n");
            prompt.push_str(&ctx);
        }
    }

    let tools = mcp_manager.get_all_tools().await;

    if tools.is_empty() {
        return prompt;
    }

    prompt.push_str("\n\n## Available Tools\n\n");
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

/// Handle `merab.ai.chat` — single-step LLM call.
pub async fn handle_ai_chat(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    message: String,
    context: Vec<ChatMessage>,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;

    let dynamic_prompt = build_dynamic_system_prompt(ENGINEER_SYSTEM_PROMPT, mcp_manager, db).await;
    client.set_system_prompt(dynamic_prompt);

    let mut messages = context;
    messages.push(ChatMessage::user(message));

    client
        .chat(messages)
        .await
        .map_err(|e| to_rpc_error(MerabError::AiError(e.to_string())))
}

/// Handle `merab.ai.executeTool` — execute a tool by name via McpManager.
pub async fn handle_execute_tool(
    mcp_manager: &Arc<McpManager>,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let mcp_client = mcp_manager
        .find_agent_for_tool(&tool_name)
        .await
        .ok_or_else(|| to_rpc_error(MerabError::ToolNotFound(tool_name.clone())))?;

    let args = match arguments {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    };

    let tool_result = mcp_client
        .call_tool(tool_name.clone(), args)
        .await
        .map_err(|e| {
            to_rpc_error(MerabError::AiError(format!(
                "tool '{}' failed: {}",
                tool_name, e
            )))
        })?;

    serde_json::to_value(&tool_result)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}

/// Handle `merab.ai.orchestrate` — Auto-Pipeline: Plan → Execute con fallback al loop simple.
pub async fn handle_ai_orchestrate(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    task: String,
) -> Result<AiResponse, ErrorObjectOwned> {
    tracing::info!("[Planner] Descomponiendo tarea...");
    let mut planner = PlannerAgent::new(config);

    let plan = match planner.decompose(&task).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "[Planner] Falló, usando loop simple como fallback");
            return handle_ai_chat_loop(config, mcp_manager, db, task).await;
        }
    };

    if plan.subtasks.is_empty() {
        tracing::warn!("[Planner] Plan sin subtasks, usando loop simple como fallback");
        return handle_ai_chat_loop(config, mcp_manager, db, task).await;
    }

    tracing::info!(
        subtasks = plan.subtasks.len(),
        description = %plan.description,
        "[Pipeline] Plan generado, ejecutando subtasks"
    );

    let plan_json = serde_json::to_string(&plan)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;

    let tracker = ArtifactTracker::new();
    let result = handle_ai_execute_plan(config, mcp_manager, db, plan_json, Some(&tracker)).await?;
    let artifacts = tracker.finish();

    tracing::info!("[Pipeline] Completado");

    let content = format_execution_result(&result);

    Ok(AiResponse {
        content,
        model: Some(config.ai.model.clone()),
        tool_call: None,
        artifacts: Some(artifacts),
    })
}

/// Fallback: loop single-LLM con tools. Usado por `merab.ai.chat` y como fallback de orchestrate.
async fn handle_ai_chat_loop(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    task: String,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;
    let max_steps = config.ai.max_orchestration_steps;

    let dynamic_prompt = build_dynamic_system_prompt(ENGINEER_SYSTEM_PROMPT, mcp_manager, db).await;
    client.set_system_prompt(dynamic_prompt);

    let mut messages: Vec<ChatMessage> = vec![ChatMessage::user(&task)];

    for step in 0..max_steps {
        tracing::info!(step = step, max = max_steps, "chat loop step");

        let response = client
            .chat(messages.clone())
            .await
            .map_err(|e| to_rpc_error(MerabError::AiError(e.to_string())))?;

        if response.tool_call.is_none() {
            tracing::info!(steps = step + 1, "chat loop complete");
            return Ok(response);
        }

        let tool_call = response.tool_call.clone().unwrap();
        tracing::info!(
            step = step,
            tool = %tool_call.name,
            "chat loop: executing tool"
        );

        messages.push(ChatMessage::assistant(&response.content));

        let tool_result_str = execute_tool_call(config, mcp_manager, &tool_call, None).await?;

        messages.push(ChatMessage::user(format!(
            "Tool '{}' returned:\n{}",
            tool_call.name, tool_result_str
        )));
    }

    tracing::warn!(max_steps = max_steps, "chat loop hit step limit");

    messages.push(ChatMessage::user(
        "You have reached the maximum number of steps. \
         Please provide a final summary of what was accomplished and what remains.",
    ));

    let final_response = client
        .chat(messages)
        .await
        .map_err(|e| to_rpc_error(MerabError::AiError(e.to_string())))?;

    Ok(final_response)
}

/// Formatea el resultado de ejecución del pipeline en texto legible.
fn format_execution_result(result: &serde_json::Value) -> String {
    let mut output = String::new();

    if let Some(desc) = result.get("plan_description").and_then(|v| v.as_str()) {
        output.push_str(&format!("## Plan: {}\n\n", desc));
    }

    if let Some(count) = result.get("subtasks_executed").and_then(|v| v.as_u64()) {
        output.push_str(&format!("Subtasks completadas: {}\n\n", count));
    }

    if let Some(results) = result.get("results").and_then(|v| v.as_array()) {
        for subtask in results {
            let persona = subtask.get("persona").and_then(|v| v.as_str()).unwrap_or("unknown");
            let desc = subtask.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let out = subtask.get("output").and_then(|v| v.as_str()).unwrap_or("");

            output.push_str(&format!("### [{}] {}\n{}\n\n", persona.to_uppercase(), desc, out));
        }
    }

    if output.is_empty() {
        output = "Pipeline completado exitosamente.".to_string();
    }

    output
}

/// Execute a single tool call via MCP and return the result as a string.
async fn execute_tool_call(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    tool_call: &merab_ai::ToolCall,
    tracker: Option<&ArtifactTracker>,
) -> Result<String, ErrorObjectOwned> {
    // Handle Internal Tools
    if tool_call.name == "core.plan" {
        let task_desc = tool_call.arguments.get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| to_rpc_error(MerabError::InvalidInput("Missing 'task' argument for core.plan".into())))?;

        let mut planner = PlannerAgent::new(config);
        let plan = planner.decompose(task_desc).await
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;

        return serde_json::to_string(&plan)
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())));
    }

    let mcp_client = mcp_manager
        .find_agent_for_tool(&tool_call.name)
        .await
        .ok_or_else(|| to_rpc_error(MerabError::ToolNotFound(tool_call.name.clone())))?;

    let args = match &tool_call.arguments {
        serde_json::Value::Object(map) => Some(map.clone()),
        _ => None,
    };

    let tool_result = mcp_client
        .call_tool(tool_call.name.clone(), args)
        .await
        .map_err(|e| {
            to_rpc_error(MerabError::AiError(format!(
                "tool '{}' failed: {}",
                tool_call.name, e
            )))
        })?;

    let result_value = serde_json::to_value(&tool_result)
        .unwrap_or(serde_json::Value::Null);

    if let Some(t) = tracker {
        t.record_tool_result(&tool_call.name, &tool_call.arguments, &result_value);
    }

    serde_json::to_string(&result_value)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}

/// Handle `merab.ai.plan` — decompose a task into a plan.
pub async fn handle_ai_plan(
    _config: &MerabConfig,
    _mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let mut planner = PlannerAgent::new(_config);
    let plan = planner.decompose(&task).await
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
    
    serde_json::to_value(&plan)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}

/// Handle `merab.ai.executePlan` — execute a plan with persona-based subtasks.
pub async fn handle_ai_execute_plan(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    plan_json: String,
    tracker: Option<&ArtifactTracker>,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let plan: Task = serde_json::from_str(&plan_json)
        .map_err(|e| to_rpc_error(MerabError::InvalidInput(format!("Invalid plan JSON: {}", e))))?;

    let max_steps = config.ai.max_orchestration_steps;
    let mut results: Vec<SubtaskResult> = Vec::new();
    let mut global_context: String = String::new();

    for subtask in &plan.subtasks {
        tracing::info!(
            subtask_id = %subtask.id,
            persona = ?subtask.persona,
            description = %subtask.description,
            "executing subtask"
        );

        let persona_prompt = get_persona_prompt(subtask.persona);
        let dynamic_prompt = build_dynamic_system_prompt(persona_prompt, mcp_manager, db).await;
        
        let persona_model = config.ai.get_model_for_persona(subtask.persona.as_str());
        tracing::info!(
            persona = ?subtask.persona,
            model = %persona_model,
            "using model for persona"
        );

        let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);
        let ai_config = AiClientConfig {
            proxy_url,
            model: persona_model.clone(),
            system_prompt: Some(dynamic_prompt),
            max_tokens: config.ai.max_tokens,
            temperature: config.ai.temperature,
        };
        let client = AiClient::new(ai_config);

        let mut messages: Vec<ChatMessage> = Vec::new();

        if !global_context.is_empty() {
            messages.push(ChatMessage::user(format!(
                "Previous work context:\n{}\n\nNow proceed with your task.",
                global_context
            )));
        }

        messages.push(ChatMessage::user(subtask.description.clone()));

        let mut subtask_output = String::new();

        for _step in 0..max_steps {
            let response = client
                .chat(messages.clone())
                .await
                .map_err(|e| to_rpc_error(MerabError::AiError(e.to_string())))?;

            if response.tool_call.is_none() {
                subtask_output = response.content.clone();
                break;
            }

            let tool_call = response.tool_call.clone().unwrap();
            messages.push(ChatMessage::assistant(&response.content));

            let tool_result = execute_tool_call(config, mcp_manager, &tool_call, tracker).await?;
            messages.push(ChatMessage::user(format!(
                "Tool '{}' returned:\n{}",
                tool_call.name, tool_result
            )));
        }

        global_context = format!(
            "{}\n\n## Subtask: {} (Persona: {})\nResult: {}",
            global_context,
            subtask.description,
            subtask.persona,
            subtask_output
        );

        results.push(SubtaskResult {
            id: subtask.id.clone(),
            description: subtask.description.clone(),
            persona: subtask.persona.to_string(),
            model: persona_model,
            output: subtask_output,
        });
    }

    let execution_result = ExecutionResult {
        plan_id: plan.id,
        plan_description: plan.description,
        subtasks_executed: results.len(),
        results,
    };

    serde_json::to_value(&execution_result)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}

#[derive(Debug, serde::Serialize)]
struct SubtaskResult {
    id: String,
    description: String,
    persona: String,
    model: String,
    output: String,
}

#[derive(Debug, serde::Serialize)]
struct ExecutionResult {
    plan_id: String,
    plan_description: String,
    subtasks_executed: usize,
    results: Vec<SubtaskResult>,
}