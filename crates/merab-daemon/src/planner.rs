use merab_ai::{AiClient, AiClientConfig, ChatMessage};
use merab_config::MerabConfig;
use merab_core::multi_agent_pipeline::Task;
use jsonrpsee::types::ErrorObjectOwned;

use crate::rpc::server::to_rpc_error;

const PLANNER_SYSTEM_PROMPT: &str = r#"You are an Elite AI Agent Architect orchestrating a team of autonomous agents.
Your goal is to translate user requirements into a precisely-tuned, high-performance plan of executable subtasks.

## Personas Available
Assign each subtask to the most appropriate persona based strictly on the required action:
- **engineer**: For analysis, reading files, searching the codebase, and planning architecture.
- **coder**: For writing new files, modifying existing code, and executing build commands.
- **reviewer**: For reading diffs/code and finding bugs or style issues.
- **qa**: For writing tests and verifying functionality.

## Execution Rules
1. **Concrete Actions Only**: Subtasks must be actionable. "Research UI patterns" is BAD. "Search for 'TaskBoard' in src/ components" is GOOD.
2. **Atomic Steps**: Each subtask should ideally represent 1-3 tool calls. Break down complex work.
3. **Sequential by Default**: If a subtask modifies a file that the next subtask needs, use `depends_on`.
4. **Tool-Oriented Descriptions**: Frame descriptions around the tools the agent will need to use (e.g., "Use fs.read to examine...).
5. **Built-in QA Mechanisms**: When assigning a 'coder' task, add a self-correction instruction like: "Write the code, then run `cargo check` to verify it compiles before finishing".

## Guardrails
- NEVER assign a subtask to 'coder' if no files are being modified. Use 'engineer' instead.
- NEVER create vague subtasks like "Implement feature". Always specify exactly which files and components.
- Do not add text outside the JSON block.

## Dependencies
Use `depends_on` to declare which subtasks must complete before this one starts.
- If two subtasks are independent, leave `depends_on` empty in both — they will run in parallel.
- If a subtask needs the output of another, list that subtask's ID in `depends_on`.

## Output Format
You must output a strictly valid JSON object matching the `Task` structure. Do not include markdown codeblocks around the JSON.
Example:
{
  "id": "root",
  "description": "Main task description",
  "status": "Pending",
  "persona": "engineer",
  "assigned_agent": null,
  "depends_on": [],
  "subtasks": [
    {
      "id": "task-1",
      "description": "Use fs.list and fs.read to locate and read the Task definition interface",
      "status": "Pending",
      "persona": "engineer",
      "assigned_agent": null,
      "depends_on": [],
      "subtasks": []
    },
    {
      "id": "task-2",
      "description": "Use fs.patch to add 'status' to the Task interface in src/types/Task.ts",
      "status": "Pending",
      "persona": "coder",
      "assigned_agent": null,
      "depends_on": ["task-1"],
      "subtasks": []
    }
  ]
}
"#;

pub struct PlannerAgent {
    client: AiClient,
}

impl PlannerAgent {
    pub fn new(config: &MerabConfig) -> Self {
         let proxy_url = format!("http://{}:{}", config.daemon.host, config.proxy.port);
         let ai_config = AiClientConfig {
            proxy_url,
            model: config.ai.model.clone(),
            system_prompt: Some(PLANNER_SYSTEM_PROMPT.to_string()),
            max_tokens: config.ai.max_tokens,
            temperature: 0.3, // Lower temp for structured output
            api_key: None,
            retry: merab_ai::RetryConfig::new(
                config.ai.max_retry_attempts,
                config.ai.retry_base_delay_ms,
            ),
        };
        Self {
            client: AiClient::new(ai_config),
        }
    }

    pub async fn decompose(&mut self, task_description: &str) -> Result<Task, ErrorObjectOwned> {
        let message = ChatMessage::user(task_description.to_string());
        
        let response = self.client.chat(vec![message]).await
            .map_err(|e| to_rpc_error(merab_core::MerabError::AiError(e.to_string())))?;

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
             .map_err(|e| to_rpc_error(merab_core::MerabError::AiError(format!("Failed to parse planner JSON: {}. Content: {}", e, cleaned))))?;
             
        // Ensure IDs are generated if LLM was lazy (though instruct said generate)
        // Ideally we trust LLM or regenerate IDs here. For now, trust LLM.
        
        Ok(task)
    }
}
