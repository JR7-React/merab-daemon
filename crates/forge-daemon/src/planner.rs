use forge_ai::{AiClient, AiClientConfig, ChatMessage};
use forge_config::ForgeConfig;
use forge_core::multi_agent_pipeline::Task;
use jsonrpsee::types::ErrorObjectOwned;

use crate::rpc::server::to_rpc_error;

const PLANNER_SYSTEM_PROMPT: &str = r#"You are an expert Project Manager AI.
Your goal is to analyze a complex user request and decompose it into a structured plan of executable subtasks.

Output Format:
You must output a strictly valid JSON object matching the `Task` structure.
Example:
{
  "id": "generated-uuid",
  "description": "Main task description",
  "status": "Pending",
  "assigned_agent": null,
  "subtasks": [
    {
      "id": "generated-uuid-1",
      "description": "Subtask 1 description",
      "status": "Pending",
      "assigned_agent": null,
      "subtasks": []
    }
  ]
}

Rules:
1. Break down the task into logical, sequential steps.
2. Use descriptive names for tasks.
3. Keep 'status' as "Pending".
4. assigned_agent should be null for now.
5. Do not include any text outside the JSON block.
"#;

pub struct PlannerAgent {
    client: AiClient,
}

impl PlannerAgent {
    pub fn new(config: &ForgeConfig) -> Self {
         let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);
         let ai_config = AiClientConfig {
            proxy_url,
            model: config.ai.model.clone(),
            system_prompt: Some(PLANNER_SYSTEM_PROMPT.to_string()),
            max_tokens: config.ai.max_tokens,
            temperature: 0.3, // Lower temp for structured output
        };
        Self {
            client: AiClient::new(ai_config),
        }
    }

    pub async fn decompose(&mut self, task_description: &str) -> Result<Task, ErrorObjectOwned> {
        let message = ChatMessage::user(task_description.to_string());
        
        let response = self.client.chat(vec![message]).await
            .map_err(|e| to_rpc_error(forge_core::ForgeError::AiError(e.to_string())))?;

        let content = response.content.trim();
        
        // Simple JSON extraction attempt
        let cleaned = if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                &content[start..=end]
            } else {
                content
            }
        } else {
            content
        };

        let task: Task = serde_json::from_str(cleaned)
             .map_err(|e| to_rpc_error(forge_core::ForgeError::AiError(format!("Failed to parse planner JSON: {}. Content: {}", e, cleaned))))?;
             
        // Ensure IDs are generated if LLM was lazy (though instruct said generate)
        // Ideally we trust LLM or regenerate IDs here. For now, trust LLM.
        
        Ok(task)
    }
}
