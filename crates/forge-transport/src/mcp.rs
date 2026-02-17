use anyhow::Result;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type AgentId = Uuid;

/// Simplified tool info for JSON-RPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl From<Tool> for McpToolInfo {
    fn from(t: Tool) -> Self {
        Self {
            name: t.name.to_string(),
            description: t.description.unwrap_or_default().to_string(),
            input_schema: serde_json::to_value(&t.input_schema).unwrap_or_default(),
        }
    }
}

/// MCP Client wrapping an rmcp RunningService.
pub struct McpClient {
    service: RunningService<rmcp::RoleClient, ()>,
    agent_id: AgentId,
}

impl McpClient {
    /// Connect to an MCP server by spawning the given command.
    /// The command must have stdin/stdout piped (set by caller).
    pub async fn connect(agent_id: AgentId, command: tokio::process::Command) -> Result<Self> {
        let transport = TokioChildProcess::new(command)?;
        let service = ().serve(transport).await?;

        tracing::info!(
            id = %agent_id,
            server_info = ?service.peer_info(),
            "MCP client connected"
        );

        Ok(Self { service, agent_id })
    }

    /// List all tools exposed by the MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>> {
        let tools_result = self.service.list_tools(Default::default()).await?;
        let tools = tools_result
            .tools
            .into_iter()
            .map(McpToolInfo::from)
            .collect();
        Ok(tools)
    }

    /// Call a tool on the MCP server.
    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult> {
        let result = self
            .service
            .call_tool(CallToolRequestParams {
                meta: None,
                name: name.into(),
                arguments,
                task: None,
            })
            .await?;
        Ok(result)
    }

    /// Gracefully shut down the MCP connection.
    pub async fn shutdown(self) {
        if let Err(e) = self.service.cancel().await {
            tracing::warn!(id = %self.agent_id, error = %e, "error shutting down MCP client");
        }
    }

    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }
}
