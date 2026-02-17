use std::collections::HashMap;
use std::sync::Arc;

use forge_core::AgentId;
use forge_transport::mcp::{McpClient, McpToolInfo};
use tokio::sync::Mutex;

/// Manages active MCP client connections and indexes their capabilities.
pub struct McpManager {
    clients: Mutex<HashMap<AgentId, Arc<McpClient>>>,
    // Map tool_name -> (AgentId, ToolInfo)
    tool_index: Mutex<HashMap<String, (AgentId, McpToolInfo)>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            tool_index: Mutex::new(HashMap::new()),
        }
    }

    /// Register an MCP client and index its tools.
    pub async fn add_client(&self, id: AgentId, client: McpClient) {
        // List tools immediately to populate index
        let tools = match client.list_tools().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(id = %id, error = %e, "failed to list tools during registration, tools will not be discoverable via A2A");
                Vec::new()
            }
        };

        let client_arc = Arc::new(client);

        {
            let mut clients = self.clients.lock().await;
            clients.insert(id, client_arc.clone());
        }

        {
            let mut index = self.tool_index.lock().await;
            for tool in tools {
                index.insert(tool.name.clone(), (id, tool.clone()));
                tracing::debug!(tool = %tool.name, agent = %id, "indexed tool");
            }
        }

        tracing::info!(id = %id, "MCP client registered and indexed");
    }

    /// Get the MCP client for an agent.
    pub async fn get_client(&self, id: AgentId) -> Option<Arc<McpClient>> {
        let map = self.clients.lock().await;
        map.get(&id).cloned()
    }

    /// Find which agent provides a specific tool.
    pub async fn find_agent_for_tool(&self, tool_name: &str) -> Option<Arc<McpClient>> {
        let agent_id = {
            let index = self.tool_index.lock().await;
            index.get(tool_name).map(|(id, _)| *id)
        };

        if let Some(id) = agent_id {
            self.get_client(id).await
        } else {
            None
        }
    }

    /// Get all registered MCP clients.
    pub async fn get_all_clients(&self) -> Vec<(AgentId, Arc<McpClient>)> {
        let map = self.clients.lock().await;
        map.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// Get all indexed tools info.
    pub async fn get_all_tools(&self) -> Vec<McpToolInfo> {
        let index = self.tool_index.lock().await;
        index.values().map(|(_, info)| info.clone()).collect()
    }

    /// Shut down and remove the MCP client for an agent.
    pub async fn remove_client(&self, id: AgentId) {
        let client = {
            let mut map = self.clients.lock().await;
            map.remove(&id)
        };

        if let Some(client) = client {
            // Remove tools owned by this agent from index
            {
                let mut index = self.tool_index.lock().await;
                index.retain(|_, (agent_id, _)| *agent_id != id);
            }

            match Arc::try_unwrap(client) {
                Ok(c) => c.shutdown().await,
                Err(_) => {
                    tracing::warn!(id = %id, "MCP client still has references, skipping graceful shutdown");
                }
            }
        }
    }

    /// Shut down all MCP clients.
    pub async fn shutdown_all(&self) {
        // Clear index
        {
            let mut index = self.tool_index.lock().await;
            index.clear();
        }

        let clients: Vec<(AgentId, Arc<McpClient>)> = {
            let mut map = self.clients.lock().await;
            map.drain().collect()
        };
        for (id, client) in clients {
            tracing::info!(id = %id, "shutting down MCP client");
            match Arc::try_unwrap(client) {
                Ok(c) => c.shutdown().await,
                Err(_) => {
                    tracing::warn!(id = %id, "MCP client still has references during shutdown_all");
                }
            }
        }
    }
}
