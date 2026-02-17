use std::sync::Arc;

use forge_core::{AgentManifest, AgentRecord, AgentStatus, AgentSummary, Message, ProtocolKind};
use forge_store::Database;
use forge_transport::mcp::McpToolInfo;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp_manager::McpManager;
use crate::registry::AgentRegistry;
use crate::supervisor::ProcessSupervisor;

#[rpc(server)]
pub trait ForgeApi {
    #[method(name = "forge.ping")]
    async fn ping(&self) -> Result<String, ErrorObjectOwned>;

    #[method(name = "forge.registerAgent")]
    async fn register_agent(
        &self,
        manifest: AgentManifest,
    ) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "forge.listAgents")]
    async fn list_agents(&self) -> Result<Vec<AgentSummary>, ErrorObjectOwned>;

    #[method(name = "forge.getAgent")]
    async fn get_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "forge.startAgent")]
    async fn start_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "forge.stopAgent")]
    async fn stop_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "forge.unregisterAgent")]
    async fn unregister_agent(&self, id: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "forge.sendMessage")]
    async fn send_message(
        &self,
        from: String,
        to: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned>;

    #[method(name = "forge.broadcastMessage")]
    async fn broadcast_message(
        &self,
        from: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned>;

    #[method(name = "forge.getMessages")]
    async fn get_messages(&self, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned>;

    #[method(name = "forge.ackMessage")]
    async fn ack_message(&self, message_id: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "forge.listTools")]
    async fn list_tools(&self, agent_id: String) -> Result<Vec<McpToolInfo>, ErrorObjectOwned>;

    #[method(name = "forge.callTool")]
    async fn call_tool(
        &self,
        agent_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ErrorObjectOwned>;
}

pub struct ForgeRpc {
    pub registry: Arc<AgentRegistry>,
    pub db: Arc<Mutex<Database>>,
    pub supervisor: Arc<ProcessSupervisor>,
    pub mcp_manager: Arc<McpManager>,
}

fn to_rpc_error(e: impl std::fmt::Display) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)
}

fn parse_id(id: &str) -> Result<Uuid, ErrorObjectOwned> {
    Uuid::parse_str(id).map_err(|_| ErrorObjectOwned::owned(-32602, "invalid agent id", None::<()>))
}

#[async_trait]
impl ForgeApiServer for ForgeRpc {
    async fn ping(&self) -> Result<String, ErrorObjectOwned> {
        Ok("pong".to_string())
    }

    async fn register_agent(
        &self,
        manifest: AgentManifest,
    ) -> Result<AgentRecord, ErrorObjectOwned> {
        let record = AgentRecord::new(manifest);
        {
            let db = self.db.lock().await;
            db.insert_agent(&record).map_err(to_rpc_error)?;
        }
        self.registry.register(record.clone()).await.map_err(to_rpc_error)?;
        tracing::info!(id = %record.id, name = %record.manifest.name, "agent registered");
        Ok(record)
    }

    async fn list_agents(&self) -> Result<Vec<AgentSummary>, ErrorObjectOwned> {
        Ok(self.registry.list().await)
    }

    async fn get_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        self.registry.get(uuid).await.map_err(to_rpc_error)
    }

    async fn start_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self.registry.get(uuid).await.map_err(to_rpc_error)?;

        if record.status == AgentStatus::Running {
            return Err(to_rpc_error(format!("agent already running: {uuid}")));
        }

        let manifest = &record.manifest;
        let is_mcp = manifest.protocol == ProtocolKind::Mcp;

        let mut cmd = tokio::process::Command::new(&manifest.command);
        cmd.args(&manifest.args);
        if let Some(dir) = &manifest.working_dir {
            cmd.current_dir(dir);
        }

        if is_mcp {
            // MCP agents need piped stdin/stdout for JSON-RPC communication
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::null());

            // Connect MCP client (this spawns the process and does the handshake)
            let mcp_client =
                forge_transport::mcp::McpClient::connect(uuid, cmd)
                    .await
                    .map_err(|e| to_rpc_error(format!("MCP handshake failed: {e}")))?;

            // We don't get the child directly from rmcp, so pid is unknown
            self.registry
                .update_status(uuid, AgentStatus::Running, None)
                .await
                .map_err(to_rpc_error)?;

            {
                let db = self.db.lock().await;
                let _ = db.update_agent_status(uuid, AgentStatus::Running, None);
            }

            self.mcp_manager.add_client(uuid, mcp_client).await;

            tracing::info!(id = %uuid, "MCP agent started");
        } else {
            // Non-MCP agents: null stdio, spawn and hand to supervisor
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            let child = cmd.spawn().map_err(|e| {
                to_rpc_error(format!("failed to start agent process: {e}"))
            })?;

            let pid = child.id();
            self.registry
                .update_status(uuid, AgentStatus::Running, pid)
                .await
                .map_err(to_rpc_error)?;

            {
                let db = self.db.lock().await;
                let _ = db.update_agent_status(uuid, AgentStatus::Running, pid);
            }

            self.supervisor
                .start_monitoring(uuid, child, record.manifest.clone())
                .await;

            tracing::info!(id = %uuid, pid = ?pid, "agent started");
        }

        self.registry.get(uuid).await.map_err(to_rpc_error)
    }

    async fn stop_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self.registry.get(uuid).await.map_err(to_rpc_error)?;

        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(format!("agent not running: {uuid}")));
        }

        let is_mcp = record.manifest.protocol == ProtocolKind::Mcp;

        if is_mcp {
            // Graceful MCP shutdown
            self.mcp_manager.remove_client(uuid).await;
        }

        // Delegate to supervisor (kills child + waits) for non-MCP
        let stopped = self.supervisor.stop_agent(uuid).await;

        if !is_mcp && !stopped {
            // Fallback: agent not in supervisor
            if let Some(pid) = record.pid {
                #[cfg(unix)]
                {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
                #[cfg(windows)]
                {
                    let _ = tokio::process::Command::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/F"])
                        .output()
                        .await;
                }
            }
        }

        self.registry
            .update_status(uuid, AgentStatus::Stopped, None)
            .await
            .map_err(to_rpc_error)?;

        {
            let db = self.db.lock().await;
            let _ = db.update_agent_status(uuid, AgentStatus::Stopped, None);
        }

        tracing::info!(id = %uuid, "agent stopped");
        self.registry.get(uuid).await.map_err(to_rpc_error)
    }

    async fn unregister_agent(&self, id: String) -> Result<bool, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self.registry.get(uuid).await.map_err(to_rpc_error)?;

        // Stop if running
        if record.status == AgentStatus::Running {
            self.stop_agent(id).await?;
        }

        // Clean up MCP client if any
        self.mcp_manager.remove_client(uuid).await;

        self.registry.remove(uuid).await.map_err(to_rpc_error)?;

        {
            let db = self.db.lock().await;
            db.delete_agent_messages(uuid).map_err(to_rpc_error)?;
            db.delete_agent(uuid).map_err(to_rpc_error)?;
        }

        tracing::info!(id = %uuid, "agent unregistered");
        Ok(true)
    }

    async fn send_message(
        &self,
        from: String,
        to: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned> {
        let from_id = parse_id(&from)?;
        let to_id = parse_id(&to)?;

        self.registry.get(from_id).await.map_err(to_rpc_error)?;
        self.registry.get(to_id).await.map_err(to_rpc_error)?;

        let msg = Message::new(from_id, Some(to_id), content);
        {
            let db = self.db.lock().await;
            db.insert_message(&msg).map_err(to_rpc_error)?;
        }
        tracing::info!(id = %msg.id, from = %from_id, to = %to_id, "message sent");
        Ok(msg)
    }

    async fn broadcast_message(
        &self,
        from: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned> {
        let from_id = parse_id(&from)?;

        self.registry.get(from_id).await.map_err(to_rpc_error)?;

        let msg = Message::new(from_id, None, content);
        {
            let db = self.db.lock().await;
            db.insert_message(&msg).map_err(to_rpc_error)?;
        }
        tracing::info!(id = %msg.id, from = %from_id, "broadcast sent");
        Ok(msg)
    }

    async fn get_messages(&self, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned> {
        let uuid = parse_id(&agent_id)?;
        let db = self.db.lock().await;
        let messages = db.get_messages_for(uuid).map_err(to_rpc_error)?;
        Ok(messages)
    }

    async fn ack_message(&self, message_id: String) -> Result<bool, ErrorObjectOwned> {
        let uuid = parse_id(&message_id)?;
        let db = self.db.lock().await;
        let updated = db.acknowledge_message(uuid).map_err(to_rpc_error)?;
        if !updated {
            return Err(ErrorObjectOwned::owned(-32000, format!("message not found: {uuid}"), None::<()>));
        }
        tracing::info!(id = %uuid, "message acknowledged");
        Ok(true)
    }

    async fn list_tools(&self, agent_id: String) -> Result<Vec<McpToolInfo>, ErrorObjectOwned> {
        let uuid = parse_id(&agent_id)?;
        let record = self.registry.get(uuid).await.map_err(to_rpc_error)?;

        if record.manifest.protocol != ProtocolKind::Mcp {
            return Err(to_rpc_error(format!("agent {uuid} is not an MCP agent")));
        }
        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(format!("agent {uuid} is not running")));
        }

        let client = self.mcp_manager.get_client(uuid).await.ok_or_else(|| {
            to_rpc_error(format!("no MCP client found for agent {uuid}"))
        })?;

        client.list_tools().await.map_err(to_rpc_error)
    }

    async fn call_tool(
        &self,
        agent_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ErrorObjectOwned> {
        let uuid = parse_id(&agent_id)?;
        let record = self.registry.get(uuid).await.map_err(to_rpc_error)?;

        if record.manifest.protocol != ProtocolKind::Mcp {
            return Err(to_rpc_error(format!("agent {uuid} is not an MCP agent")));
        }
        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(format!("agent {uuid} is not running")));
        }

        let client = self.mcp_manager.get_client(uuid).await.ok_or_else(|| {
            to_rpc_error(format!("no MCP client found for agent {uuid}"))
        })?;

        let args = match arguments {
            serde_json::Value::Object(map) => Some(map),
            serde_json::Value::Null => None,
            _ => return Err(to_rpc_error("arguments must be a JSON object or null")),
        };

        let result = client
            .call_tool(tool_name, args)
            .await
            .map_err(to_rpc_error)?;

        serde_json::to_value(&result).map_err(to_rpc_error)
    }
}
