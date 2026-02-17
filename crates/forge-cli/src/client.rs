use anyhow::Result;
use forge_core::{AgentManifest, AgentRecord, AgentSummary, Message};
use forge_transport::a2a::{AgentCard, TaskResponse, TaskDetails};
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::params::ObjectParams;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

pub struct ForgeClient {
    client: HttpClient,
}

impl ForgeClient {
    pub fn new(url: &str) -> Result<Self> {
        let client = HttpClientBuilder::default().build(url)?;
        Ok(Self { client })
    }

    pub async fn ping(&self) -> Result<String> {
        let result: String = self.client.request("forge.ping", ObjectParams::new()).await?;
        Ok(result)
    }

    pub async fn register_agent(&self, manifest: AgentManifest) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("manifest", &manifest)?;
        let result: AgentRecord = self.client.request("forge.registerAgent", params).await?;
        Ok(result)
    }

    pub async fn list_agents(&self) -> Result<Vec<AgentSummary>> {
        let result: Vec<AgentSummary> = self
            .client
            .request("forge.listAgents", ObjectParams::new())
            .await?;
        Ok(result)
    }

    pub async fn get_agent(&self, id: &str) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: AgentRecord = self.client.request("forge.getAgent", params).await?;
        Ok(result)
    }

    pub async fn start_agent(&self, id: &str) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: AgentRecord = self.client.request("forge.startAgent", params).await?;
        Ok(result)
    }

    pub async fn stop_agent(&self, id: &str) -> Result<AgentRecord> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: AgentRecord = self.client.request("forge.stopAgent", params).await?;
        Ok(result)
    }

    pub async fn unregister_agent(&self, id: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("id", id)?;
        let result: bool = self.client.request("forge.unregisterAgent", params).await?;
        Ok(result)
    }

    pub async fn send_message(&self, from: &str, to: &str, content: &str) -> Result<Message> {
        let mut params = ObjectParams::new();
        params.insert("from", from)?;
        params.insert("to", to)?;
        params.insert("content", content)?;
        let result: Message = self.client.request("forge.sendMessage", params).await?;
        Ok(result)
    }

    pub async fn broadcast_message(&self, from: &str, content: &str) -> Result<Message> {
        let mut params = ObjectParams::new();
        params.insert("from", from)?;
        params.insert("content", content)?;
        let result: Message = self.client.request("forge.broadcastMessage", params).await?;
        Ok(result)
    }

    pub async fn get_messages(&self, agent_id: &str) -> Result<Vec<Message>> {
        let mut params = ObjectParams::new();
        params.insert("agent_id", agent_id)?;
        let result: Vec<Message> = self.client.request("forge.getMessages", params).await?;
        Ok(result)
    }

    pub async fn ack_message(&self, message_id: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("message_id", message_id)?;
        let result: bool = self.client.request("forge.ackMessage", params).await?;
        Ok(result)
    }

    pub async fn list_tools(&self, agent_id: &str) -> Result<Vec<serde_json::Value>> {
        let mut params = ObjectParams::new();
        params.insert("agent_id", agent_id)?;
        let result: Vec<serde_json::Value> =
            self.client.request("forge.listTools", params).await?;
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
        let result: serde_json::Value = self.client.request("forge.callTool", params).await?;
        Ok(result)
    }

    pub async fn a2a_discover(&self, url: &str) -> Result<AgentCard> {
        let mut params = ObjectParams::new();
        params.insert("url", url)?;
        let result: AgentCard = self.client.request("forge.a2aDiscover", params).await?;
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
        let result: TaskResponse = self.client.request("forge.a2aSend", params).await?;
        Ok(result)
    }

    pub async fn a2a_get_task(&self, url: &str, task_id: &str) -> Result<forge_transport::a2a::TaskDetails> {
        let mut params = ObjectParams::new();
        params.insert("url", url)?;
        params.insert("task_id", task_id)?;
        let result: forge_transport::a2a::TaskDetails = self.client.request("forge.a2aGetTask", params).await?;
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
        let result: bool = self.client.request("forge.memory.put", params).await?;
        Ok(result)
    }

    pub async fn memory_get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let mut params = ObjectParams::new();
        params.insert("key", key)?;
        let result: Option<serde_json::Value> = self.client.request("forge.memory.get", params).await?;
        Ok(result)
    }

    pub async fn memory_delete(&self, key: &str) -> Result<bool> {
        let mut params = ObjectParams::new();
        params.insert("key", key)?;
        let result: bool = self.client.request("forge.memory.delete", params).await?;
        Ok(result)
    }

    pub async fn memory_list(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let mut params = ObjectParams::new();
        params.insert("prefix", prefix)?;
        let result: Vec<String> = self.client.request("forge.memory.list", params).await?;
        Ok(result)
    }

    pub async fn get_system_status(&self) -> Result<forge_core::SystemStatus> {
        let result: forge_core::SystemStatus = self.client.request("forge.getSystemStatus", ObjectParams::new()).await?;
        Ok(result)
    }
}

