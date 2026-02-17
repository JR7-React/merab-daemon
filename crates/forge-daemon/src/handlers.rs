use std::sync::Arc;

use forge_core::{AgentManifest, AgentRecord, AgentStatus, AgentSummary, Message, ProtocolKind};
use forge_store::Database;
use forge_transport::a2a::{A2aClient, AgentCard, TaskResponse};
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

    #[method(name = "forge.a2aDiscover")]
    async fn a2a_discover(&self, url: String) -> Result<AgentCard, ErrorObjectOwned>;

    #[method(name = "forge.a2aSend")]
    async fn a2a_send(
        &self,
        url: String,
        skill: String,
        input: serde_json::Value,
    ) -> Result<TaskResponse, ErrorObjectOwned>;

    #[method(name = "forge.memory.put")]
    async fn memory_put(
        &self,
        key: String,
        value: serde_json::Value,
        ttl_seconds: Option<u64>,
    ) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "forge.memory.get")]
    async fn memory_get(&self, key: String) -> Result<Option<serde_json::Value>, ErrorObjectOwned>;

    #[method(name = "forge.memory.delete")]
    async fn memory_delete(&self, key: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "forge.memory.list")]
    async fn memory_list(&self, prefix: Option<String>) -> Result<Vec<String>, ErrorObjectOwned>;
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
    
    // ... startAgent, stopAgent, unregisterAgent, send, broadcast, getMessages, ack, listTools, callTool are same ...
    // I need to include them to keep the file valid, but 'replace' tool works with exact matching.
    // Since I'm replacing the trait definition and adding imports, I should check if I can just replace the trait and impl block beginning/end or if I need to provide full content.
    // The previous read showed full content. I'll just append the new methods to the impl block if I can match a large chunk, or better, I will assume the previous 'read_file' output is accurate and I can just replace the whole file content to be safe and ensure all methods are present.
    // Actually, 'replace' tool is best for surgical changes. I'll replace the trait definition and then append the new methods to the impl block.
    // But adding methods to the trait requires changing the trait definition block.
    // And adding methods to the impl requires changing the impl block.
    // I'll do two replacements or one big one. Since the file is < 400 lines, I'll rewrite the imports and trait, and then I'll use a second call to add the impl methods? No, tool allows only one replace per call unless expected_replacements is used.
    // I'll rewrite the whole file with the new content, filling in the missing parts from my memory of the read file.
    
    // Wait, rewriting the whole file is risky if I miss something.
    // I'll use 'replace' to update the imports and trait first.
    // Then another 'replace' to add the impl methods.
    
    // Imports and Trait:
    // ...
    
    // Actually, I can just replace the whole file content since I have it from 'read_file'. I just need to be careful to copy everything correctly.
    // Let's try to be smart. I'll replace imports first.
    
    // Then I'll replace the end of the trait.
    
    // Then I'll replace the end of the impl.
    
    // Let's do it in chunks.
    
    // Chunk 1: Imports.
    // Chunk 2: Trait definition (add new methods).
    // Chunk 3: Impl definition (add new methods).
    
    // I will try to do it in one go if I can match the context.
    
    // Actually, I'll just write the whole file content. I have the output of `read_file` just above. I will copy-paste the existing code and add the new parts.
    
    // ... (copying existing code) ...
    
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
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::null());

            let mcp_client =
                forge_transport::mcp::McpClient::connect(uuid, cmd)
                    .await
                    .map_err(|e| to_rpc_error(format!("MCP handshake failed: {e}")))?;

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
            self.mcp_manager.remove_client(uuid).await;
        }

        let stopped = self.supervisor.stop_agent(uuid).await;

        if !is_mcp && !stopped {
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

        if record.status == AgentStatus::Running {
            self.stop_agent(id).await?;
        }

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

    async fn a2a_discover(&self, url: String) -> Result<AgentCard, ErrorObjectOwned> {
        A2aClient::fetch_card(&url).await.map_err(to_rpc_error)
    }

    async fn a2a_send(
        &self,
        url: String,
        skill: String,
        input: serde_json::Value,
    ) -> Result<TaskResponse, ErrorObjectOwned> {
        A2aClient::send_task(&url, &skill, input)
            .await
            .map_err(to_rpc_error)
    }

    async fn memory_put(
        &self,
        key: String,
        value: serde_json::Value,
        ttl_seconds: Option<u64>,
    ) -> Result<bool, ErrorObjectOwned> {
        let db = self.db.lock().await;
        // Assume global scope for now (agent_id = None)
        db.put_memory(&key, &value, None, ttl_seconds).map_err(to_rpc_error)?;
        Ok(true)
    }

    async fn memory_get(&self, key: String) -> Result<Option<serde_json::Value>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.get_memory(&key).map_err(to_rpc_error)
    }

    async fn memory_delete(&self, key: String) -> Result<bool, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.delete_memory(&key).map_err(to_rpc_error)
    }

    async fn memory_list(&self, prefix: Option<String>) -> Result<Vec<String>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.list_memory_keys(prefix.as_deref()).map_err(to_rpc_error)
    }
}

