use anyhow::Result;
use merab_ai::{AiResponse, ChatMessage};
use merab_core::{AgentManifest, AgentRecord, AgentSummary, Message, ProjectInfo, Session};
use merab_store::{ConvMessage, ConvSummary};
use merab_transport::a2a::{AgentCard, TaskResponse};
use serde::{Deserialize, Serialize};
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::params::ObjectParams;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: String,
    pub task: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub duration_secs: Option<u64>,
}

pub struct MerabClient {
    client: HttpClient,
    url: String,
}

impl MerabClient {
    pub fn new(url: &str) -> Result<Self> {
        let client = HttpClientBuilder::default()
            .request_timeout(std::time::Duration::from_secs(600))
            .build(url)?;
        Ok(Self {
            client,
            url: url.to_string(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the current project path: active-project override or CWD.
    pub fn current_project_path() -> String {
        // Check override file first
        if let Some(config_dir) = dirs::config_dir() {
            let override_file = config_dir.join("merab").join("active-project");
            if let Ok(content) = std::fs::read_to_string(&override_file) {
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    }

    pub async fn ping(&self) -> Result<String> {
        let result: String = self
            .client
            .request("merab.ping", ObjectParams::new())
            .await?;
        Ok(result)
    }

    pub async fn register_agent(&self, manifest: AgentManifest) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("manifest", &manifest)?;
        let result: AgentRecord = self.client.request("merab.registerAgent", params).await?;
        Ok(result)
    }

    pub async fn list_agents(&self) -> Result<Vec<AgentSummary>> {
        let result: Vec<AgentSummary> = self
            .client
            .request("merab.listAgents", ObjectParams::new())
            .await?;
        Ok(result)
    }

    pub async fn get_agent(&self, id: &str) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: AgentRecord = self.client.request("merab.getAgent", params).await?;
        Ok(result)
    }

    pub async fn start_agent(&self, id: &str) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: AgentRecord = self.client.request("merab.startAgent", params).await?;
        Ok(result)
    }

    pub async fn stop_agent(&self, id: &str) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: AgentRecord = self.client.request("merab.stopAgent", params).await?;
        Ok(result)
    }

    pub async fn unregister_agent(&self, id: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: bool = self.client.request("merab.unregisterAgent", params).await?;
        Ok(result)
    }

    pub async fn send_message(&self, from: &str, to: &str, content: &str) -> Result<Message> {
        let mut params = ObjectParams::new();
        params.insert("from", from)?;
        params.insert("to", to)?;
        params.insert("content", content)?;
        let result: Message = self.client.request("merab.sendMessage", params).await?;
        Ok(result)
    }

    pub async fn broadcast_message(&self, from: &str, content: &str) -> Result<Message> {
        let mut params = ObjectParams::new();
        params.insert("from", from)?;
        params.insert("content", content)?;
        let result: Message = self
            .client
            .request("merab.broadcastMessage", params)
            .await?;
        Ok(result)
    }

    pub async fn get_messages(&self, agent_id: &str) -> Result<Vec<Message>> {
        let mut params = ObjectParams::new();
        params.insert("agent_id", agent_id)?;
        let result: Vec<Message> = self.client.request("merab.getMessages", params).await?;
        Ok(result)
    }

    pub async fn ack_message(&self, message_id: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("message_id", message_id)?;
        let result: bool = self.client.request("merab.ackMessage", params).await?;
        Ok(result)
    }

    pub async fn list_tools(&self, agent_id: &str) -> Result<Vec<serde_json::Value>> {
        let mut params = ObjectParams::new();
        params.insert("agent_id", agent_id)?;
        let result: Vec<serde_json::Value> = self.client.request("merab.listTools", params).await?;
        Ok(result)
    }

    pub async fn call_tool(
        &self,
        agent_id: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut params = ObjectParams::new();
        params.insert("agent_id", agent_id)?;
        params.insert("tool_name", tool_name)?;
        params.insert("arguments", &arguments)?;
        let result: serde_json::Value = self.client.request("merab.callTool", params).await?;
        Ok(result)
    }

    pub async fn a2a_discover(&self, url: &str) -> Result<AgentCard> {
        let mut params = ObjectParams::new();
        params.insert("url", url)?;
        let result: AgentCard = self.client.request("merab.a2aDiscover", params).await?;
        Ok(result)
    }

    pub async fn a2a_send(
        &self,
        url: &str,
        skill: &str,
        input: serde_json::Value,
    ) -> Result<TaskResponse> {
        let mut params = ObjectParams::new();
        params.insert("url", url)?;
        params.insert("skill", skill)?;
        params.insert("input", &input)?;
        let result: TaskResponse = self.client.request("merab.a2aSend", params).await?;
        Ok(result)
    }

    pub async fn a2a_get_task(
        &self,
        url: &str,
        task_id: &str,
    ) -> Result<merab_transport::a2a::TaskDetails> {
        let mut params = ObjectParams::new();
        params.insert("url", url)?;
        params.insert("task_id", task_id)?;
        let result: merab_transport::a2a::TaskDetails =
            self.client.request("merab.a2aGetTask", params).await?;
        Ok(result)
    }

    pub async fn memory_put(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl_seconds: Option<u64>,
    ) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("key", key)?;
        params.insert("value", &value)?;
        params.insert("ttl_seconds", ttl_seconds)?;
        let result: bool = self.client.request("merab.memory.put", params).await?;
        Ok(result)
    }

    pub async fn memory_get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let mut params = ObjectParams::new();
        params.insert("key", key)?;
        let result: Option<serde_json::Value> =
            self.client.request("merab.memory.get", params).await?;
        Ok(result)
    }

    pub async fn memory_delete(&self, key: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("key", key)?;
        let result: bool = self.client.request("merab.memory.delete", params).await?;
        Ok(result)
    }

    pub async fn memory_list(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let mut params = ObjectParams::new();
        params.insert("prefix", prefix)?;
        let result: Vec<String> = self.client.request("merab.memory.list", params).await?;
        Ok(result)
    }

    pub async fn get_system_status(&self) -> Result<merab_core::SystemStatus> {
        let result: merab_core::SystemStatus = self
            .client
            .request("merab.getSystemStatus", ObjectParams::new())
            .await?;
        Ok(result)
    }

    pub async fn ai_chat(&self, message: &str, context: Vec<ChatMessage>) -> Result<AiResponse> {
        let mut params = ObjectParams::new();
        params.insert("message", message)?;
        let context_json = serde_json::to_string(&context)?;
        params.insert("context_json", context_json)?;
        let result: AiResponse = self.client.request("merab.ai.chat", params).await?;
        Ok(result)
    }

    pub async fn ai_orchestrate_stream(&self, task: &str, event_file: &str) -> Result<AiResponse> {
        let mut params = ObjectParams::new();
        params.insert("task", task)?;
        params.insert("event_file", event_file)?;
        let result: AiResponse = self.client.request("merab.ai.orchestrate.stream", params).await?;
        Ok(result)
    }

    pub async fn ai_orchestrate_with_tests(&self, task: &str, event_file: &str) -> Result<AiResponse> {
        let mut params = ObjectParams::new();
        params.insert("task", task)?;
        params.insert("event_file", event_file)?;
        let result: AiResponse = self.client.request("merab.ai.orchestrate.withTests", params).await?;
        Ok(result)
    }

    pub async fn ai_execute_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut params = ObjectParams::new();
        params.insert("tool_name", tool_name)?;
        params.insert("arguments", &arguments)?;
        let result: serde_json::Value = self
            .client
            .request("merab.ai.executeTool", params)
            .await?;
        Ok(result)
    }

    pub async fn ai_plan(&self, task: &str) -> Result<serde_json::Value> {
        let mut params = ObjectParams::new();
        params.insert("task", task)?;
        let result: serde_json::Value = self.client.request("merab.ai.plan", params).await?;
        Ok(result)
    }

    pub async fn ai_execute_plan(&self, plan_json: &str) -> Result<serde_json::Value> {
        let mut params = ObjectParams::new();
        params.insert("plan_json", plan_json)?;
        let result: serde_json::Value = self.client.request("merab.ai.executePlan", params).await?;
        Ok(result)
    }

    pub async fn session_get_last(&self, project_path: &str) -> Result<Option<Session>> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        let result: Option<Session> =
            self.client.request("merab.session.getLast", params).await?;
        Ok(result)
    }

    pub async fn session_list(&self, project_path: &str, limit: u32) -> Result<Vec<Session>> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        params.insert("limit", limit)?;
        let result: Vec<Session> = self.client.request("merab.session.list", params).await?;
        Ok(result)
    }

    pub async fn get_project_stats(&self, project_path: &str) -> Result<merab_core::ProjectStats> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        let result: merab_core::ProjectStats = self.client.request("merab.session.stats", params).await?;
        Ok(result)
    }

    pub async fn conv_create(&self, project_path: &str) -> Result<ConvSummary> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        let result: ConvSummary = self.client.request("merab.conv.create", params).await?;
        Ok(result)
    }

    pub async fn conv_add_message(&self, conv_id: &str, role: &str, content: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("conv_id", conv_id)?;
        params.insert("role", role)?;
        params.insert("content", content)?;
        let result: bool = self.client.request("merab.conv.addMessage", params).await?;
        Ok(result)
    }

    pub async fn conv_get_messages(&self, conv_id: &str) -> Result<Vec<ConvMessage>> {
        let mut params = ObjectParams::new();
        params.insert("conv_id", conv_id)?;
        let result: Vec<ConvMessage> = self.client.request("merab.conv.getMessages", params).await?;
        Ok(result)
    }

    pub async fn conv_list(&self, project_path: &str, limit: u32) -> Result<Vec<ConvSummary>> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        params.insert("limit", limit)?;
        let result: Vec<ConvSummary> = self.client.request("merab.conv.list", params).await?;
        Ok(result)
    }

    pub async fn conv_get_last(&self, project_path: &str) -> Result<Option<ConvSummary>> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        let result: Option<ConvSummary> = self.client.request("merab.conv.getLast", params).await?;
        Ok(result)
    }

    pub async fn job_submit(&self, task: &str) -> Result<String> {
        let mut params = ObjectParams::new();
        params.insert("task", task)?;
        let result: String = self.client.request("merab.job.submit", params).await?;
        Ok(result)
    }

    pub async fn job_status(&self, job_id: &str) -> Result<Option<JobSummary>> {
        let mut params = ObjectParams::new();
        params.insert("job_id", job_id)?;
        let result: Option<JobSummary> = self.client.request("merab.job.status", params).await?;
        Ok(result)
    }

    pub async fn job_log(&self, job_id: &str) -> Result<String> {
        let mut params = ObjectParams::new();
        params.insert("job_id", job_id)?;
        let result: String = self.client.request("merab.job.log", params).await?;
        Ok(result)
    }

    pub async fn job_list(&self, limit: Option<u32>) -> Result<Vec<JobSummary>> {
        let mut params = ObjectParams::new();
        if let Some(l) = limit {
            params.insert("limit", l)?;
        }
        let result: Vec<JobSummary> = self.client.request("merab.job.list", params).await?;
        Ok(result)
    }

    pub async fn job_cancel(&self, job_id: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("job_id", job_id)?;
        let result: bool = self.client.request("merab.job.cancel", params).await?;
        Ok(result)
    }

    pub async fn index_build(&self, project_path: String) -> Result<u64> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        let result: u64 = self.client.request("merab.index.build", params).await?;
        Ok(result)
    }

    pub async fn index_search(
        &self,
        project_path: String,
        query: String,
        kind: Option<String>,
        limit: usize,
    ) -> Result<Vec<merab_store::IndexedSymbol>> {
        let mut params = ObjectParams::new();
        params.insert("project_path", project_path)?;
        params.insert("query", query)?;
        params.insert("kind", kind)?;
        params.insert("limit", limit)?;
        let result: Vec<merab_store::IndexedSymbol> =
            self.client.request("merab.index.search", params).await?;
        Ok(result)
    }

    pub async fn project_list(&self, limit: Option<u32>) -> Result<Vec<ProjectInfo>> {
        let mut params = ObjectParams::new();
        params.insert("limit", limit)?;
        let result: Vec<ProjectInfo> = self.client.request("merab.project.list", params).await?;
        Ok(result)
    }

    pub async fn project_touch(&self, path: &str) -> Result<()> {
        let mut params = ObjectParams::new();
        params.insert("path", path)?;
        let _: () = self.client.request("merab.project.touch", params).await?;
        Ok(())
    }
}
