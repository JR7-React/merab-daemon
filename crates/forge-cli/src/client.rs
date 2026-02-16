use anyhow::Result;
use forge_core::{AgentManifest, AgentRecord, AgentSummary};
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
}
