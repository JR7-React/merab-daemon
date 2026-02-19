use std::collections::HashMap;
use std::sync::Arc;

use merab_ai::{AiClient, AiClientConfig, AiError, ChatMessage, RetryConfig};
use merab_config::MerabConfig;
use merab_core::multi_agent_pipeline::Task;
use merab_core::MerabError;
use merab_store::Database;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;

use crate::artifacts::ArtifactTracker;
use crate::events::EventSink;
use crate::mcp_manager::McpManager;
use crate::planner::PlannerAgent;
use crate::prompts::get_persona_prompt;
use crate::rpc::ai_methods::build_dynamic_system_prompt;
use crate::rpc::server::to_rpc_error;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SubtaskResult {
    pub id: String,
    pub description: String,
    pub persona: String,
    pub model: String,
    pub output: String,
    #[serde(default)]
    pub tokens_input: u64,
    #[serde(default)]
    pub tokens_output: u64,
}

impl SubtaskResult {
    pub fn total_tokens(&self) -> u64 {
        self.tokens_input + self.tokens_output
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutionResult {
    pub plan_id: String,
    pub plan_description: String,
    pub subtasks_executed: usize,
    pub results: Vec<SubtaskResult>,
    #[serde(default)]
    pub tokens_input: u64,
    #[serde(default)]
    pub tokens_output: u64,
}

impl ExecutionResult {
    pub fn total_tokens(&self) -> u64 {
        self.tokens_input + self.tokens_output
    }
}

/// Ejecuta el plan como un DAG: subtasks independientes corren en paralelo,
/// las dependientes esperan a que sus predecesoras terminen.
pub async fn execute_plan_dag(
    config: &Arc<MerabConfig>,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    plan: &Task,
    tracker: Option<ArtifactTracker>,
    event_sink: Option<EventSink>,
) -> Result<Vec<SubtaskResult>, ErrorObjectOwned> {
    let max_parallel = config.ai.max_parallel_tasks as usize;
    let mut completed: HashMap<String, SubtaskResult> = HashMap::new();
    let mut remaining: Vec<Task> = plan.subtasks.clone();

    while !remaining.is_empty() {
        // Separar subtasks listas (todas sus deps ya completadas) de las que no
        let (mut ready, not_ready): (Vec<Task>, Vec<Task>) = remaining
            .into_iter()
            .partition(|t| t.depends_on.iter().all(|dep| completed.contains_key(dep)));

        if ready.is_empty() {
            return Err(to_rpc_error(MerabError::TaskFailed(
                "dependencia circular o depends_on inválido en el plan".into(),
            )));
        }

        // Respetar max_parallel: diferir el exceso a la siguiente ola
        let wave: Vec<Task> = if ready.len() > max_parallel {
            let deferred = ready.split_off(max_parallel);
            remaining = not_ready;
            remaining.extend(deferred);
            ready
        } else {
            remaining = not_ready;
            ready
        };

        if wave.len() > 1 {
            tracing::info!(
                count = wave.len(),
                tasks = ?wave.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
                "[DAG] Ejecutando subtasks en paralelo"
            );
        } else {
            tracing::info!(
                id = %wave[0].id,
                persona = %wave[0].persona,
                "[DAG] Ejecutando subtask"
            );
        }

        // Spawn todas las subtasks de la ola en paralelo
        let handles: Vec<_> = wave
            .iter()
            .map(|subtask| {
                let cfg = config.clone();
                let mcp = mcp_manager.clone();
                let db = db.clone();
                let subtask = subtask.clone();
                let dep_ctx = build_dep_context(&completed, &subtask.depends_on);
                let trk = tracker.clone();
                let sink = event_sink.clone();
                tokio::spawn(async move {
                    execute_subtask(cfg, mcp, db, subtask, dep_ctx, trk, sink).await
                })
            })
            .collect();

        // Recoger resultados
        for (subtask, handle) in wave.into_iter().zip(handles) {
            let result = handle
                .await
                .map_err(|e| {
                    to_rpc_error(MerabError::Internal(format!("subtask panicked: {}", e)))
                })??;

            tracing::info!(
                id = %subtask.id,
                persona = %subtask.persona,
                "[DAG] Subtask completada"
            );
            completed.insert(subtask.id.clone(), result);
        }
    }

    Ok(completed.into_values().collect())
}

async fn execute_subtask(
    config: Arc<MerabConfig>,
    mcp_manager: Arc<McpManager>,
    db: Arc<Mutex<Database>>,
    subtask: Task,
    dep_context: String,
    tracker: Option<ArtifactTracker>,
    event_sink: Option<EventSink>,
) -> Result<SubtaskResult, ErrorObjectOwned> {
    let max_steps = config.ai.max_orchestration_steps;
    let persona_prompt = get_persona_prompt(subtask.persona);
    let dynamic_prompt = build_dynamic_system_prompt(persona_prompt, &mcp_manager, &db, Some(&subtask.description)).await;
    let persona_model = config.ai.get_model_for_persona(subtask.persona.as_str());

    if let Some(ref sink) = event_sink {
        sink.emit_subtask_start(subtask.persona.as_str(), &subtask.description);
    }

    tracing::info!(
        id = %subtask.id,
        persona = ?subtask.persona,
        model = %persona_model,
        "[DAG] Iniciando subtask"
    );

    let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);
    let persona_api_key = config.ai.get_api_key_for_persona(subtask.persona.as_str());
    let retry_cfg = RetryConfig::new(config.ai.max_retry_attempts, config.ai.retry_base_delay_ms);
    let ai_config = AiClientConfig {
        proxy_url: proxy_url.clone(),
        model: persona_model.clone(),
        system_prompt: Some(dynamic_prompt.clone()),
        max_tokens: config.ai.max_tokens,
        temperature: config.ai.temperature,
        api_key: persona_api_key.clone(),
        retry: retry_cfg,
    };
    let client = AiClient::new(ai_config.clone());

    // Fallback keys for key rotation: all unique persona keys except the current one.
    let fallback_keys: Vec<String> = {
        let mut keys: Vec<String> = config
            .ai
            .personas
            .values()
            .filter_map(|p| p.api_key.clone())
            .collect();
        if let Some(ref proxy_key) = config.proxy.api_key {
            keys.push(proxy_key.clone());
        }
        if let Some(ref curr) = persona_api_key {
            keys.retain(|k| k != curr);
        }
        keys.dedup();
        keys
    };

    let mut messages: Vec<ChatMessage> = Vec::new();
    if !dep_context.is_empty() {
        messages.push(ChatMessage::user(format!(
            "Context from previous work:\n{}\n\nNow proceed with your task.",
            dep_context
        )));
    }
    messages.push(ChatMessage::user(subtask.description.clone()));

    let mut output = String::new();
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;

    for _step in 0..max_steps {
        let chat_result = client.chat(messages.clone()).await;

        // On RateLimited, attempt key rotation before giving up.
        let response = match chat_result {
            Ok(r) => r,
            Err(AiError::RateLimited { attempts, ref message }) => {
                tracing::warn!(
                    persona = %subtask.persona,
                    attempts = attempts,
                    "rate limited — attempting key rotation"
                );

                let mut rotated = false;
                let mut rotated_response = None;

                for (i, fallback_key) in fallback_keys.iter().enumerate() {
                    if let Some(ref sink) = event_sink {
                        sink.emit_retrying(subtask.persona.as_str(), i as u32 + 1, 0);
                    }
                    let fallback_client = AiClient::new(AiClientConfig {
                        api_key: Some(fallback_key.clone()),
                        retry: RetryConfig::new(1, config.ai.retry_base_delay_ms),
                        ..ai_config.clone()
                    });
                    if let Ok(r) = fallback_client.chat(messages.clone()).await {
                        tracing::info!(persona = %subtask.persona, "key rotation succeeded");
                        rotated = true;
                        rotated_response = Some(r);
                        break;
                    }
                }

                if rotated {
                    rotated_response.unwrap()
                } else {
                    return Err(to_rpc_error(MerabError::AiError(format!(
                        "Rate limited after {} attempts, key rotation failed: {}",
                        attempts, message
                    ))));
                }
            }
            Err(e) => return Err(to_rpc_error(MerabError::AiError(e.to_string()))),
        };

        if let Some(ref usage) = response.usage {
            total_input += usage.input;
            total_output += usage.output;
        }

        if response.tool_call.is_none() {
            output = response.content.clone();
            break;
        }

        let tool_call = response.tool_call.clone().unwrap();
        messages.push(ChatMessage::assistant(&response.content));

        if let Some(ref sink) = event_sink {
            sink.emit_step(subtask.persona.as_str(), &format!("Executing tool: {}", tool_call.name));
        }

        let result = execute_tool_call(&config, &mcp_manager, &tool_call, tracker.as_ref()).await?;
        messages.push(ChatMessage::user(format!(
            "Tool '{}' returned:\n{}",
            tool_call.name, result
        )));
    }

    if let Some(ref sink) = event_sink {
        sink.emit_subtask_done(subtask.persona.as_str(), &subtask.description);
    }

    Ok(SubtaskResult {
        id: subtask.id.clone(),
        description: subtask.description.clone(),
        persona: subtask.persona.to_string(),
        model: persona_model,
        output,
        tokens_input: total_input,
        tokens_output: total_output,
    })
}

/// Construye el contexto para una subtask a partir de los resultados de sus dependencias.
fn build_dep_context(completed: &HashMap<String, SubtaskResult>, deps: &[String]) -> String {
    if deps.is_empty() {
        return String::new();
    }
    deps.iter()
        .filter_map(|dep_id| completed.get(dep_id))
        .map(|r| format!("[{}] {}\n{}", r.persona.to_uppercase(), r.description, r.output))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Ejecuta un tool call via MCP y retorna el resultado como String.
pub(crate) async fn execute_tool_call(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    tool_call: &merab_ai::ToolCall,
    tracker: Option<&ArtifactTracker>,
) -> Result<String, ErrorObjectOwned> {
    // Tool interno: core.plan
    if tool_call.name == "core.plan" {
        let task_desc = tool_call
            .arguments
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                to_rpc_error(MerabError::InvalidInput(
                    "Missing 'task' argument for core.plan".into(),
                ))
            })?;

        let mut planner = PlannerAgent::new(config);
        let plan = planner
            .decompose(task_desc)
            .await
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

    let result_value =
        serde_json::to_value(&tool_result).unwrap_or(serde_json::Value::Null);

    if let Some(t) = tracker {
        t.record_tool_result(&tool_call.name, &tool_call.arguments, &result_value);
    }

    serde_json::to_string(&result_value)
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}
