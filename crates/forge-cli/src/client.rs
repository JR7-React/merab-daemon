use anyhow::Result;
use forge_core::{AgentManifest, AgentRecord, AgentSummary, Message};
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
}
