use std::collections::HashMap;
use std::sync::Arc;

use forge_core::AgentId;
use forge_transport::mcp::McpClient;
use tokio::sync::Mutex;

/// Manages active MCP client connections, one per MCP agent.
pub struct McpManager {
    clients: Mutex<HashMap<AgentId, Arc<McpClient>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// Register an MCP client for the given agent.
    pub async fn add_client(&self, id: AgentId, client: McpClient) {
        let mut map = self.clients.lock().await;
        map.insert(id, Arc::new(client));
        tracing::info!(id = %id, "MCP client registered in manager");
    }

    /// Get the MCP client for an agent, if it exists.
    pub async fn get_client(&self, id: AgentId) -> Option<Arc<McpClient>> {
        let map = self.clients.lock().await;
        map.get(&id).cloned()
    }

    /// Shut down and remove the MCP client for an agent.
    pub async fn remove_client(&self, id: AgentId) {
        let client = {
            let mut map = self.clients.lock().await;
            map.remove(&id)
        };
        if let Some(client) = client {
            // Try to unwrap the Arc; if other references exist, just log a warning
            match Arc::try_unwrap(client) {
                Ok(c) => c.shutdown().await,
                Err(_) => {
                    tracing::warn!(id = %id, "MCP client still has references, skipping graceful shutdown");
                }
            }
        }
    }

    /// Shut down all MCP clients. Called on daemon exit.
    pub async fn shutdown_all(&self) {
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
