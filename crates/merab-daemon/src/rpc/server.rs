use std::sync::Arc;
use std::time::Instant;

use merab_ai::{AiResponse, ChatMessage};
use merab_config::MerabConfig;
use merab_core::{
    AgentManifest, AgentRecord, AgentStats, AgentStatus, AgentSummary, AiInfo, MerabError, Message,
    NodeInfo, ProtocolKind, ProxyStats, SystemStatus,
};
use merab_store::Database;
use merab_transport::a2a::{A2aClient, AgentCard, TaskResponse};
use merab_transport::mcp::McpToolInfo;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp_manager::McpManager;
use crate::registry::AgentRegistry;
use crate::rpc::ai_methods;
use crate::supervisor::ProcessSupervisor;

#[rpc(server)]
pub trait MerabApi {
    #[method(name = "merab.ping")]
    async fn ping(&self) -> Result<String, ErrorObjectOwned>;

    #[method(name = "merab.registerAgent")]
    async fn register_agent(
        &self,
        manifest: AgentManifest,
    ) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "merab.listAgents")]
    async fn list_agents(&self) -> Result<Vec<AgentSummary>, ErrorObjectOwned>;

    #[method(name = "merab.getAgent")]
    async fn get_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "merab.startAgent")]
    async fn start_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "merab.stopAgent")]
    async fn stop_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned>;

    #[method(name = "merab.unregisterAgent")]
    async fn unregister_agent(&self, id: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.sendMessage")]
    async fn send_message(
        &self,
        from: String,
        to: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned>;

    #[method(name = "merab.broadcastMessage")]
    async fn broadcast_message(
        &self,
        from: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned>;

    #[method(name = "merab.getMessages")]
    async fn get_messages(&self, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned>;

    #[method(name = "merab.ackMessage")]
    async fn ack_message(&self, message_id: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.listTools")]
    async fn list_tools(&self, agent_id: String) -> Result<Vec<McpToolInfo>, ErrorObjectOwned>;

    #[method(name = "merab.callTool")]
    async fn call_tool(
        &self,
        agent_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.a2aDiscover")]
    async fn a2a_discover(&self, url: String) -> Result<AgentCard, ErrorObjectOwned>;

    #[method(name = "merab.a2aSend")]
    async fn a2a_send(
        &self,
        url: String,
        skill: String,
        input: serde_json::Value,
    ) -> Result<TaskResponse, ErrorObjectOwned>;

    #[method(name = "merab.a2aGetTask")]
    async fn a2a_get_task(
        &self,
        url: String,
        task_id: String,
    ) -> Result<merab_transport::a2a::TaskDetails, ErrorObjectOwned>;

    #[method(name = "merab.memory.put")]
    async fn memory_put(
        &self,
        key: String,
        value: serde_json::Value,
        ttl_seconds: Option<u64>,
    ) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.memory.get")]
    async fn memory_get(&self, key: String) -> Result<Option<serde_json::Value>, ErrorObjectOwned>;

    #[method(name = "merab.memory.delete")]
    async fn memory_delete(&self, key: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.memory.list")]
    async fn memory_list(&self, prefix: Option<String>) -> Result<Vec<String>, ErrorObjectOwned>;

    #[method(name = "merab.getSystemStatus")]
    async fn get_system_status(&self) -> Result<SystemStatus, ErrorObjectOwned>;

    // AI methods
    #[method(name = "merab.ai.chat")]
    async fn ai_chat(
        &self,
        message: String,
        context_json: String,
    ) -> Result<AiResponse, ErrorObjectOwned>;

    #[method(name = "merab.ai.orchestrate")]
    async fn ai_orchestrate(&self, task: String) -> Result<AiResponse, ErrorObjectOwned>;

    #[method(name = "merab.ai.executeTool")]
    async fn ai_execute_tool(
        &self,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.ai.plan")]
    async fn ai_plan(&self, task: String) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.ai.executePlan")]
    async fn ai_execute_plan(&self, plan_json: String) -> Result<serde_json::Value, ErrorObjectOwned>;
}

pub struct MerabRpc {
    pub registry: Arc<AgentRegistry>,
    pub db: Arc<Mutex<Database>>,
    pub supervisor: Arc<ProcessSupervisor>,
    pub mcp_manager: Arc<McpManager>,
    pub config: Arc<MerabConfig>,
    pub start_time: Instant,
}

pub(crate) fn to_rpc_error(e: MerabError) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(e.code(), e.to_string(), None::<()>)
}

fn parse_id(id: &str) -> Result<Uuid, ErrorObjectOwned> {
    Uuid::parse_str(id)
        .map_err(|_| to_rpc_error(MerabError::InvalidManifest("invalid agent id".into())))
}

#[async_trait]
impl MerabApiServer for MerabRpc {
    async fn ping(&self) -> Result<String, ErrorObjectOwned> {
        Ok("pong".to_string())
    }

    async fn register_agent(
        &self,
        manifest: AgentManifest,
    ) -> Result<AgentRecord, ErrorObjectOwned> {
        let record = AgentRecord::new(manifest);
        if self.registry.get(record.id).await.is_ok() {
            return Err(to_rpc_error(MerabError::AlreadyExists(
                record.id.to_string(),
            )));
        }
        {
            let db = self.db.lock().await;
            db.insert_agent(&record)
                .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        }
        self.registry
            .register(record.clone())
            .await
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        tracing::info!(id = %record.id, name = %record.manifest.name, "agent registered");
        Ok(record)
    }

    async fn list_agents(&self) -> Result<Vec<AgentSummary>, ErrorObjectOwned> {
        Ok(self.registry.list().await)
    }

    async fn get_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        self.registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))
    }

    async fn start_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self
            .registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
        if record.status == AgentStatus::Running {
            return Err(to_rpc_error(MerabError::AlreadyRunning(uuid)));
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
            let mcp_client = merab_transport::mcp::McpClient::connect(uuid, cmd)
                .await
                .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))?;
            self.registry
                .update_status(uuid, AgentStatus::Running, None)
                .await
                .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
            {
                let db = self.db.lock().await;
                let _ = db.update_agent_status(uuid, AgentStatus::Running, None);
            }
            self.mcp_manager.add_client(uuid, mcp_client).await;
            tracing::info!(id = %uuid, "MCP agent started");
        } else {
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
            let job_object = match merab_sandbox::JobObject::new() {
                Ok(job) => {
                    if self.config.sandbox.enabled {
                        let limit_bytes = self.config.sandbox.memory_limit_mb * 1024 * 1024;
                        if let Err(e) = job.set_memory_limit(limit_bytes) {
                            tracing::warn!(id = %uuid, error = %e, "failed to set memory limit");
                        }
                    }
                    Some(job)
                }
                Err(e) => {
                    tracing::warn!(id = %uuid, error = %e, "failed to create job object");
                    None
                }
            };
            let child = cmd.spawn().map_err(|e| {
                to_rpc_error(MerabError::Internal(format!(
                    "failed to start process: {}",
                    e
                )))
            })?;
            let pid = child.id();
            if let (Some(pid), Some(job)) = (pid, &job_object) {
                if let Err(e) = job.assign_process(pid) {
                    tracing::warn!(id = %uuid, error = %e, "failed to assign to sandbox");
                }
            }
            self.registry
                .update_status(uuid, AgentStatus::Running, pid)
                .await
                .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
            {
                let db = self.db.lock().await;
                let _ = db.update_agent_status(uuid, AgentStatus::Running, pid);
            }
            self.supervisor
                .start_monitoring(uuid, child, record.manifest.clone(), job_object)
                .await;
            tracing::info!(id = %uuid, pid = ?pid, "agent started");
        }
        self.registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))
    }

    async fn stop_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self
            .registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(MerabError::NotRunning(uuid)));
        }
        if record.manifest.protocol == ProtocolKind::Mcp {
            self.mcp_manager.remove_client(uuid).await;
        }
        self.supervisor.stop_agent(uuid).await;
        self.registry
            .update_status(uuid, AgentStatus::Stopped, None)
            .await
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        {
            let db = self.db.lock().await;
            let _ = db.update_agent_status(uuid, AgentStatus::Stopped, None);
        }
        tracing::info!(id = %uuid, "agent stopped");
        self.registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))
    }

    async fn unregister_agent(&self, id: String) -> Result<bool, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self
            .registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
        if record.status == AgentStatus::Running {
            self.stop_agent(id).await?;
        }
        self.mcp_manager.remove_client(uuid).await;
        self.registry
            .remove(uuid)
            .await
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        {
            let db = self.db.lock().await;
            db.delete_agent_messages(uuid)
                .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
            db.delete_agent(uuid)
                .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        }
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
        self.registry
            .get(from_id)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(from_id)))?;
        self.registry
            .get(to_id)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(to_id)))?;
        let msg = Message::new(from_id, Some(to_id), content);
        {
            let db = self.db.lock().await;
            db.insert_message(&msg)
                .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        }
        Ok(msg)
    }

    async fn broadcast_message(
        &self,
        from: String,
        content: String,
    ) -> Result<Message, ErrorObjectOwned> {
        let from_id = parse_id(&from)?;
        self.registry
            .get(from_id)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(from_id)))?;
        let msg = Message::new(from_id, None, content);
        {
            let db = self.db.lock().await;
            db.insert_message(&msg)
                .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        }
        Ok(msg)
    }

    async fn get_messages(&self, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned> {
        let uuid = parse_id(&agent_id)?;
        let db = self.db.lock().await;
        db.get_messages_for(uuid)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn ack_message(&self, message_id: String) -> Result<bool, ErrorObjectOwned> {
        let uuid = parse_id(&message_id)?;
        let db = self.db.lock().await;
        let updated = db
            .acknowledge_message(uuid)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        if !updated {
            return Err(to_rpc_error(MerabError::MessageNotFound(uuid)));
        }
        Ok(true)
    }

    async fn list_tools(&self, agent_id: String) -> Result<Vec<McpToolInfo>, ErrorObjectOwned> {
        let uuid = parse_id(&agent_id)?;
        let record = self
            .registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
        if record.manifest.protocol != ProtocolKind::Mcp {
            return Err(to_rpc_error(MerabError::InvalidManifest(
                "not an MCP agent".into(),
            )));
        }
        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(MerabError::NotRunning(uuid)));
        }
        let client = self.mcp_manager.get_client(uuid).await.ok_or_else(|| {
            to_rpc_error(MerabError::Internal(
                "MCP client not found despite agent running".into(),
            ))
        })?;
        client
            .list_tools()
            .await
            .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
    }

    async fn call_tool(
        &self,
        agent_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ErrorObjectOwned> {
        let uuid = parse_id(&agent_id)?;
        let record = self
            .registry
            .get(uuid)
            .await
            .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(MerabError::NotRunning(uuid)));
        }
        let client = self.mcp_manager.get_client(uuid).await.ok_or_else(|| {
            to_rpc_error(MerabError::Internal(
                "MCP client not found despite agent running".into(),
            ))
        })?;
        let args = match arguments {
            serde_json::Value::Object(map) => Some(map),
            serde_json::Value::Null => None,
            _ => {
                return Err(to_rpc_error(MerabError::InvalidManifest(
                    "args must be object".into(),
                )));
            }
        };
        let result = client
            .call_tool(tool_name, args)
            .await
            .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))?;
        serde_json::to_value(&result).map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
    }

    async fn a2a_discover(&self, url: String) -> Result<AgentCard, ErrorObjectOwned> {
        A2aClient::fetch_card(&url)
            .await
            .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
    }

    async fn a2a_send(
        &self,
        url: String,
        skill: String,
        input: serde_json::Value,
    ) -> Result<TaskResponse, ErrorObjectOwned> {
        A2aClient::send_task(&url, &skill, input)
            .await
            .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
    }

    async fn a2a_get_task(
        &self,
        url: String,
        task_id: String,
    ) -> Result<merab_transport::a2a::TaskDetails, ErrorObjectOwned> {
        A2aClient::get_task_status(&url, &task_id)
            .await
            .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
    }

    async fn memory_put(
        &self,
        key: String,
        value: serde_json::Value,
        ttl_seconds: Option<u64>,
    ) -> Result<bool, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.put_memory(&key, &value, None, ttl_seconds)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        let _ = db.cleanup_expired_memory();
        Ok(true)
    }

    async fn memory_get(&self, key: String) -> Result<Option<serde_json::Value>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.get_memory(&key)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn memory_delete(&self, key: String) -> Result<bool, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.delete_memory(&key)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn memory_list(&self, prefix: Option<String>) -> Result<Vec<String>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.list_memory_keys(prefix.as_deref())
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn get_system_status(&self) -> Result<SystemStatus, ErrorObjectOwned> {
        let agents_summary = self.registry.list().await;
        let mut agents_stats = Vec::new();

        for agent in agents_summary {
            let memory_usage = self
                .supervisor
                .get_agent_memory(agent.id)
                .await
                .unwrap_or(0);
            agents_stats.push(AgentStats {
                id: agent.id,
                name: agent.name,
                status: agent.status,
                memory_usage_bytes: memory_usage,
                memory_limit_bytes: self.config.sandbox.memory_limit_mb * 1024 * 1024,
            });
        }

        let db = self.db.lock().await;
        let (hits, total) = db
            .get_proxy_stats()
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;

        Ok(SystemStatus {
            agents: agents_stats,
            proxy: ProxyStats {
                cache_hits: hits,
                total_requests: total,
            },
            node_info: NodeInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: self.start_time.elapsed().as_secs(),
                rpc_port: self.config.daemon.rpc_port,
                a2a_port: self.config.daemon.a2a_port,
                proxy_port: self.config.proxy.port,
            },
            ai: AiInfo {
                model: self.config.ai.model.clone(),
                max_tokens: self.config.ai.max_tokens,
                temperature: self.config.ai.temperature,
            },
        })
    }

    async fn ai_chat(
        &self,
        message: String,
        context_json: String,
    ) -> Result<AiResponse, ErrorObjectOwned> {
        let ctx: Vec<ChatMessage> = serde_json::from_str(&context_json)
            .map_err(|e| to_rpc_error(MerabError::InvalidManifest(e.to_string())))?;
        ai_methods::handle_ai_chat(&self.config, &self.mcp_manager, &self.db, message, ctx).await
    }

    async fn ai_orchestrate(&self, task: String) -> Result<AiResponse, ErrorObjectOwned> {
        ai_methods::handle_ai_orchestrate(&self.config, &self.mcp_manager, &self.db, task).await
    }

    async fn ai_execute_tool(
        &self,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ErrorObjectOwned> {
        ai_methods::handle_execute_tool(&self.mcp_manager, tool_name, arguments).await
    }

    async fn ai_plan(&self, task: String) -> Result<serde_json::Value, ErrorObjectOwned> {
        ai_methods::handle_ai_plan(&self.config, &self.mcp_manager, task).await
    }

    async fn ai_execute_plan(&self, plan_json: String) -> Result<serde_json::Value, ErrorObjectOwned> {
        ai_methods::handle_ai_execute_plan(&self.config, &self.mcp_manager, &self.db, plan_json, None).await
    }
}
