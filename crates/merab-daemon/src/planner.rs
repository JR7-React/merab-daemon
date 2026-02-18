use merab_ai::{AiClient, AiClientConfig, ChatMessage};
use merab_config::MerabConfig;
use merab_core::multi_agent_pipeline::Task;
use jsonrpsee::types::ErrorObjectOwned;

use crate::rpc::server::to_rpc_error;

const PLANNER_SYSTEM_PROMPT: &str = r#"You are an expert Project Manager AI.
Your goal is to analyze a complex user request and decompose it into a structured plan of executable subtasks.

## Personas Available
Assign each subtask to the most appropriate persona:
- **engineer**: General purpose, good for analysis, research, exploration
- **coder**: Writing code, implementing features, fixing bugs
- **reviewer**: Code review, quality assurance, finding issues
- **qa**: Writing tests, verifying functionality, edge case coverage

## Dependencies
Use `depends_on` to declare which subtasks must complete before this one starts.
- If two subtasks are independent, leave `depends_on` empty in both — they will run in parallel.
- If a subtask needs the output of another, list that subtask's ID in `depends_on`.
- Example: Coder depends on Architect, QA depends on Coder.

## Output Format
You must output a strictly valid JSON object matching the `Task` structure.
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
      "description": "Analyze existing code structure",
      "status": "Pending",
      "persona": "engineer",
      "assigned_agent": null,
      "depends_on": [],
      "subtasks": []
    },
    {
      "id": "task-2",
      "description": "Research best practices",
      "status": "Pending",
      "persona": "engineer",
      "assigned_agent": null,
      "depends_on": [],
      "subtasks": []
    },
    {
      "id": "task-3",
      "description": "Implement the feature",
      "status": "Pending",
      "persona": "coder",
      "assigned_agent": null,
      "depends_on": ["task-1", "task-2"],
      "subtasks": []
    },
    {
      "id": "task-4",
      "description": "Review the implementation",
      "status": "Pending",
      "persona": "reviewer",
      "assigned_agent": null,
      "depends_on": ["task-3"],
      "subtasks": []
    },
    {
      "id": "task-5",
      "description": "Write and run tests",
      "status": "Pending",
      "persona": "qa",
      "assigned_agent": null,
      "depends_on": ["task-3"],
      "subtasks": []
    }
  ]
}

## Rules
1. Break down the task into logical subtasks.
2. Assign the most appropriate persona to each subtask.
3. Use `depends_on` to model real dependencies — independent tasks run in parallel.
4. Keep 'status' as "Pending".
5. assigned_agent should be null.
6. Do not include any text outside the JSON block.
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
