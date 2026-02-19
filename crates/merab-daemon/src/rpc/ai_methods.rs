use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use merab_ai::{AiClient, AiClientConfig, AiResponse, ChatMessage, RetryConfig};
use merab_config::MerabConfig;
use merab_core::multi_agent_pipeline::Task;
use merab_core::session::{Session, SessionStatus};
use merab_core::{estimate_cost, ArtifactLog, MerabError, TokenUsage};
use merab_store::Database;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;

use super::server::to_rpc_error;
use crate::artifacts::ArtifactTracker;
use crate::dag::{execute_plan_dag, ExecutionResult};
use crate::events::EventSink;
use crate::mcp_manager::McpManager;
use crate::planner::PlannerAgent;
use crate::prompts::ENGINEER_SYSTEM_PROMPT;
use crate::test_runner::run_tests;


pub fn build_ai_client(config: &MerabConfig) -> Result<AiClient, ErrorObjectOwned> {
    let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);

    let ai_config = AiClientConfig {
        proxy_url,
        model: config.ai.model.clone(),
        system_prompt: Some(ENGINEER_SYSTEM_PROMPT.to_string()),
        max_tokens: config.ai.max_tokens,
        temperature: config.ai.temperature,
        api_key: None,
        retry: RetryConfig::new(config.ai.max_retry_attempts, config.ai.retry_base_delay_ms),
    };

    Ok(AiClient::new(ai_config))
}

pub async fn build_dynamic_system_prompt(
    base_prompt: &str,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    task: Option<&str>,
) -> String {
    let mut prompt = base_prompt.to_string();

    {
        let store = db.lock().await;
        if let Ok(Some(serde_json::Value::String(ctx))) = store.get_memory("project.context") {
            prompt.push_str("\n\n## Project Context\n");
            prompt.push_str(&ctx);
        }

        if let Ok(Some(serde_json::Value::String(instr))) = store.get_memory("project.instructions") {
            prompt.push_str("\n\n## Project Instructions\n");
            prompt.push_str("The following instructions are set by the project owner and MUST be followed:\n\n");
            prompt.push_str(&instr);
        }

        // Inject relevant symbols from codebase index
        if let Some(task_str) = task {
            if let Ok(Some(serde_json::Value::String(project_path))) = store.get_memory("project.root_path") {
                let keywords: Vec<&str> = task_str
                    .split_whitespace()
                    .filter(|w| w.len() > 4)
                    .take(5)
                    .collect();
                let mut index_context = String::new();
                for kw in keywords {
                    if let Ok(symbols) = store.index_search(&project_path, kw, None, 5) {
                        for s in symbols {
                            index_context.push_str(&format!(
                                "  {} {} @ {}:{}\n",
                                s.kind, s.name, s.file, s.line
                            ));
                        }
                    }
                }
                if !index_context.is_empty() {
                    prompt.push_str("\n## Relevant Project Symbols\n");
                    prompt.push_str(&index_context);
                }
            }
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

pub async fn handle_ai_chat(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    message: String,
    context: Vec<ChatMessage>,
) -> Result<AiResponse, ErrorObjectOwned> {
    let mut client = build_ai_client(config)?;

    let dynamic_prompt = build_dynamic_system_prompt(ENGINEER_SYSTEM_PROMPT, mcp_manager, db, Some(&message)).await;
    client.set_system_prompt(dynamic_prompt);

    let mut messages = context;
    messages.push(ChatMessage::user(message));

    client
        .chat(messages)
        .await
        .map_err(|e| to_rpc_error(MerabError::AiError(e.to_string())))
}

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

pub async fn handle_ai_orchestrate(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    task: String,
    event_file: Option<PathBuf>,
    enable_test_loop: bool,
) -> Result<AiResponse, ErrorObjectOwned> {
    let created_at = Utc::now();

    let event_sink = match &event_file {
        Some(path) => EventSink::from_file(path).map_err(|e| {
            to_rpc_error(MerabError::Internal(format!("Failed to create event file: {}", e)))
        })?,
        None => EventSink::new(),
    };

    let project_path = {
        let store = db.lock().await;
        match store.get_memory("project.root_path") {
            Ok(Some(serde_json::Value::String(p))) => PathBuf::from(p),
            _ => PathBuf::from("."),
        }
    };

    let (content, artifacts, tokens_input, tokens_output) = 
        run_pipeline_with_fix_loop(config, mcp_manager, &task, &project_path, &event_sink, enable_test_loop).await?;

    let project_path_str = project_path.to_string_lossy().to_string();
    let cost_usd = estimate_cost(&config.ai.model, tokens_input, tokens_output).unwrap_or(0.0);

    let summary = if content.len() > 500 {
        format!("{}...", &content[..500])
    } else {
        content.clone()
    };

    let session = Session {
        id: Uuid::new_v4().to_string(),
        project_path: project_path_str,
        task: task.clone(),
        summary,
        artifacts: artifacts.clone(),
        status: SessionStatus::Completed,
        created_at,
        completed_at: Some(Utc::now()),
        tokens_input,
        tokens_output,
        cost_usd,
    };

    {
        let store = db.lock().await;
        if let Err(e) = store.save_session(&session) {
            tracing::warn!(error = %e, "Failed to save session");
        }
    }

    event_sink.emit_done();

    Ok(AiResponse {
        content,
        model: Some(config.ai.model.clone()),
        tool_call: None,
        artifacts: Some(artifacts),
        usage: Some(TokenUsage::new(tokens_input, tokens_output, config.ai.model.clone())),
    })
}

async fn run_pipeline_with_fix_loop(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    task: &str,
    project_path: &Path,
    event_sink: &EventSink,
    enable_test_loop: bool,
) -> Result<(String, ArtifactLog, u64, u64), ErrorObjectOwned> {
    let max_fix_cycles = config.ai.max_fix_cycles;
    
    let mut current_task = task.to_string();
    let mut all_artifacts = Vec::new();
    let mut total_tokens_input: u64 = 0;
    let mut total_tokens_output: u64 = 0;
    let mut final_content = String::new();
    
    for cycle in 0..max_fix_cycles {
        event_sink.emit_planning();
        tracing::info!("[Planner] Descomponiendo tarea...");
        let mut planner = PlannerAgent::new(config);

        let plan = match planner.decompose(&current_task).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "[Planner] Falló");
                return Err(to_rpc_error(MerabError::Internal(format!("Planner failed: {}", e))));
            }
        };

        if plan.subtasks.is_empty() {
            return Err(to_rpc_error(MerabError::Internal("Empty plan".to_string())));
        }

        event_sink.emit_plan_ready(plan.subtasks.len());
        
        let _plan_json = serde_json::to_string(&plan)
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;

        let tracker = ArtifactTracker::new();
        let config_arc = Arc::new(config.clone());
        
        // Create a dummy in-memory DB for the plan execution (not used for storage)
        let dummy_db = Arc::new(Mutex::new(
            Database::open_in_memory().map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?
        ));
        
        let results = execute_plan_dag(&config_arc, mcp_manager, &dummy_db, &plan, Some(tracker.clone()), None).await?;
        let artifacts = tracker.finish();
        all_artifacts.push(artifacts);

        for r in &results {
            total_tokens_input += r.tokens_input;
            total_tokens_output += r.tokens_output;
        }

        final_content = format_pipeline_results(&plan, &results);
        
        if !enable_test_loop {
            let merged_artifacts = merge_artifacts(&all_artifacts);
            return Ok((final_content, merged_artifacts, total_tokens_input, total_tokens_output));
        }

        event_sink.emit_step("test_runner", "Running tests...");
        
        let test_result = run_tests(project_path).await;
        
        if test_result.success {
            event_sink.emit_step("test_runner", &format!("All tests passed ({} passed)", test_result.passed));
            let merged_artifacts = merge_artifacts(&all_artifacts);
            return Ok((final_content, merged_artifacts, total_tokens_input, total_tokens_output));
        }

        event_sink.emit_step("test_runner", &format!("Tests failed: {} failed", test_result.failed));

        if cycle < max_fix_cycles - 1 {
            current_task = format!(
                "The previous implementation has failing tests. Fix the code to make these tests pass:\n\nTest Output:\n{}\n\nOriginal task: {}",
                test_result.output, task
            );
            
            event_sink.emit_step("fix_loop", &format!("Fix cycle {}/{}", cycle + 1, max_fix_cycles));
        }
    }

    final_content = format!(
        "{} \n\n⚠️ Tests still failing after {} fix cycles. Manual intervention required.",
        final_content, max_fix_cycles
    );
    let merged_artifacts = merge_artifacts(&all_artifacts);
    Ok((final_content, merged_artifacts, total_tokens_input, total_tokens_output))
}

fn merge_artifacts(artifacts_list: &[ArtifactLog]) -> ArtifactLog {
    let mut files_created = Vec::new();
    let mut files_modified = Vec::new();
    let mut commands = Vec::new();
    
    for artifacts in artifacts_list {
        files_created.extend(artifacts.files_created.clone());
        files_modified.extend(artifacts.files_modified.clone());
        commands.extend(artifacts.commands.clone());
    }
    
    ArtifactLog {
        files_created,
        files_modified,
        commands,
    }
}

fn format_pipeline_results(plan: &Task, results: &[crate::dag::SubtaskResult]) -> String {
    let mut output = String::new();

    output.push_str(&format!("## Plan: {}\n\n", plan.description));
    output.push_str(&format!("Subtasks completadas: {}\n\n", results.len()));

    for (i, subtask) in plan.subtasks.iter().enumerate() {
        let result = results.get(i);
        let out = result.map(|r| r.output.as_str()).unwrap_or("");
        
        output.push_str(&format!("### [{}] {}\n{}\n\n", 
            subtask.persona, subtask.description, out));
    }

    if output.is_empty() {
        output = "Pipeline completado exitosamente.".to_string();
    }

    output
}

pub async fn handle_ai_plan(
    config: &MerabConfig,
    _mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let mut planner = PlannerAgent::new(config);
    let plan = planner.decompose(&task).await
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
    
    serde_json::to_value(&plan)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}

pub async fn handle_ai_execute_plan(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    plan_json: String,
    tracker: Option<&ArtifactTracker>,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let plan: Task = serde_json::from_str(&plan_json)
        .map_err(|e| to_rpc_error(MerabError::InvalidInput(format!("Invalid plan JSON: {}", e))))?;

    let config_arc = Arc::new(config.clone());
    let results = execute_plan_dag(&config_arc, mcp_manager, db, &plan, tracker.cloned(), None).await?;

    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    for r in &results {
        total_input += r.tokens_input;
        total_output += r.tokens_output;
    }

    let execution_result = ExecutionResult {
        plan_id: plan.id,
        plan_description: plan.description,
        subtasks_executed: results.len(),
        results,
        tokens_input: total_input,
        tokens_output: total_output,
    };

    serde_json::to_value(&execution_result)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}
