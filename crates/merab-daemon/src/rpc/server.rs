use std::sync::Arc;
use std::time::Instant;

use merab_ai::{AiResponse, ChatMessage};
use merab_config::MerabConfig;
use merab_core::{
    AgentManifest, AgentRecord, AgentStats, AgentSummary, AiInfo, MerabError, Message,
    NodeInfo, ProjectInfo, ProxyStats, Session, SystemStatus, ProjectStats,
};
use merab_store::Database;
use merab_transport::a2a::{AgentCard, TaskResponse};
use merab_transport::mcp::McpToolInfo;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp_manager::McpManager;
use crate::registry::AgentRegistry;
use crate::rpc::{agent_impls, ai_methods, job_impls, project_impls, store_impls};
use crate::supervisor::ProcessSupervisor;

#[rpc(server)]
pub trait MerabApi {
    #[method(name = "merab.ping")]
    async fn ping(&self) -> Result<String, ErrorObjectOwned>;

    #[method(name = "merab.registerAgent")]
    async fn register_agent(&self, manifest: AgentManifest) -> Result<AgentRecord, ErrorObjectOwned>;

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
    async fn send_message(&self, from: String, to: String, content: String) -> Result<Message, ErrorObjectOwned>;

    #[method(name = "merab.broadcastMessage")]
    async fn broadcast_message(&self, from: String, content: String) -> Result<Message, ErrorObjectOwned>;

    #[method(name = "merab.getMessages")]
    async fn get_messages(&self, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned>;

    #[method(name = "merab.ackMessage")]
    async fn ack_message(&self, message_id: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.listTools")]
    async fn list_tools(&self, agent_id: String) -> Result<Vec<McpToolInfo>, ErrorObjectOwned>;

    #[method(name = "merab.callTool")]
    async fn call_tool(&self, agent_id: String, tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.a2aDiscover")]
    async fn a2a_discover(&self, url: String) -> Result<AgentCard, ErrorObjectOwned>;

    #[method(name = "merab.a2aSend")]
    async fn a2a_send(&self, url: String, skill: String, input: serde_json::Value) -> Result<TaskResponse, ErrorObjectOwned>;

    #[method(name = "merab.a2aGetTask")]
    async fn a2a_get_task(&self, url: String, task_id: String) -> Result<merab_transport::a2a::TaskDetails, ErrorObjectOwned>;

    #[method(name = "merab.memory.put")]
    async fn memory_put(&self, key: String, value: serde_json::Value, ttl_seconds: Option<u64>) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.memory.get")]
    async fn memory_get(&self, key: String) -> Result<Option<serde_json::Value>, ErrorObjectOwned>;

    #[method(name = "merab.memory.delete")]
    async fn memory_delete(&self, key: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.memory.list")]
    async fn memory_list(&self, prefix: Option<String>) -> Result<Vec<String>, ErrorObjectOwned>;

    #[method(name = "merab.getSystemStatus")]
    async fn get_system_status(&self) -> Result<SystemStatus, ErrorObjectOwned>;

    #[method(name = "merab.ai.chat")]
    async fn ai_chat(&self, message: String, context_json: String) -> Result<AiResponse, ErrorObjectOwned>;

    #[method(name = "merab.ai.orchestrate")]
    async fn ai_orchestrate(&self, task: String) -> Result<AiResponse, ErrorObjectOwned>;

    #[method(name = "merab.ai.orchestrate.stream")]
    async fn ai_orchestrate_stream(&self, task: String, event_file: String) -> Result<AiResponse, ErrorObjectOwned>;

    #[method(name = "merab.ai.orchestrate.withTests")]
    async fn ai_orchestrate_with_tests(&self, task: String, event_file: String) -> Result<AiResponse, ErrorObjectOwned>;

    #[method(name = "merab.ai.executeTool")]
    async fn ai_execute_tool(&self, tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.ai.plan")]
    async fn ai_plan(&self, task: String) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.ai.executePlan")]
    async fn ai_execute_plan(&self, plan_json: String) -> Result<serde_json::Value, ErrorObjectOwned>;

    #[method(name = "merab.session.getLast")]
    async fn session_get_last(&self, project_path: String) -> Result<Option<Session>, ErrorObjectOwned>;

    #[method(name = "merab.session.list")]
    async fn session_list(&self, project_path: String, limit: u32) -> Result<Vec<Session>, ErrorObjectOwned>;

    #[method(name = "merab.session.stats")]
    async fn session_stats(&self, project_path: String) -> Result<ProjectStats, ErrorObjectOwned>;

    #[method(name = "merab.conv.create")]
    async fn conv_create(&self, project_path: String) -> Result<merab_store::ConvSummary, ErrorObjectOwned>;

    #[method(name = "merab.conv.addMessage")]
    async fn conv_add_message(&self, conv_id: String, role: String, content: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.conv.getMessages")]
    async fn conv_get_messages(&self, conv_id: String) -> Result<Vec<merab_store::ConvMessage>, ErrorObjectOwned>;

    #[method(name = "merab.conv.list")]
    async fn conv_list(&self, project_path: String, limit: u32) -> Result<Vec<merab_store::ConvSummary>, ErrorObjectOwned>;

    #[method(name = "merab.conv.getLast")]
    async fn conv_get_last(&self, project_path: String) -> Result<Option<merab_store::ConvSummary>, ErrorObjectOwned>;

    #[method(name = "merab.job.submit")]
    async fn job_submit(&self, task: String) -> Result<String, ErrorObjectOwned>;

    #[method(name = "merab.job.status")]
    async fn job_status(&self, job_id: String) -> Result<Option<crate::jobs::JobSummary>, ErrorObjectOwned>;

    #[method(name = "merab.job.log")]
    async fn job_log(&self, job_id: String) -> Result<String, ErrorObjectOwned>;

    #[method(name = "merab.job.list")]
    async fn job_list(&self, limit: Option<u32>) -> Result<Vec<crate::jobs::JobSummary>, ErrorObjectOwned>;

    #[method(name = "merab.job.cancel")]
    async fn job_cancel(&self, job_id: String) -> Result<bool, ErrorObjectOwned>;

    #[method(name = "merab.index.build")]
    async fn index_build(&self, project_path: String) -> Result<u64, ErrorObjectOwned>;

    #[method(name = "merab.index.search")]
    async fn index_search(&self, project_path: String, query: String, kind: Option<String>, limit: usize) -> Result<Vec<merab_store::IndexedSymbol>, ErrorObjectOwned>;

    #[method(name = "merab.project.list")]
    async fn project_list(&self, limit: Option<u32>) -> Result<Vec<ProjectInfo>, ErrorObjectOwned>;

    #[method(name = "merab.project.touch")]
    async fn project_touch(&self, path: String) -> Result<(), ErrorObjectOwned>;
}

pub struct MerabRpc {
    pub registry: Arc<AgentRegistry>,
    pub db: Arc<Mutex<Database>>,
    pub supervisor: Arc<ProcessSupervisor>,
    pub mcp_manager: Arc<McpManager>,
    pub config: Arc<MerabConfig>,
    pub start_time: Instant,
    pub job_manager: Arc<crate::jobs::JobManager>,
}

pub(crate) fn to_rpc_error(e: MerabError) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(e.code(), e.to_string(), None::<()>)
}

pub(crate) fn parse_id(id: &str) -> Result<Uuid, ErrorObjectOwned> {
    Uuid::parse_str(id)
        .map_err(|_| to_rpc_error(MerabError::InvalidManifest("invalid agent id".into())))
}

#[async_trait]
impl MerabApiServer for MerabRpc {
    async fn ping(&self) -> Result<String, ErrorObjectOwned> {
        Ok("pong".to_string())
    }

    async fn register_agent(&self, manifest: AgentManifest) -> Result<AgentRecord, ErrorObjectOwned> {
        agent_impls::register_agent(self, manifest).await
    }

    async fn list_agents(&self) -> Result<Vec<AgentSummary>, ErrorObjectOwned> {
        agent_impls::list_agents(self).await
    }

    async fn get_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        agent_impls::get_agent(self, id).await
    }

    async fn start_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        agent_impls::start_agent(self, id).await
    }

    async fn stop_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        agent_impls::stop_agent(self, id).await
    }

    async fn unregister_agent(&self, id: String) -> Result<bool, ErrorObjectOwned> {
        agent_impls::unregister_agent(self, id).await
    }

    async fn send_message(&self, from: String, to: String, content: String) -> Result<Message, ErrorObjectOwned> {
        store_impls::send_message(self, from, to, content).await
    }

    async fn broadcast_message(&self, from: String, content: String) -> Result<Message, ErrorObjectOwned> {
        store_impls::broadcast_message(self, from, content).await
    }

    async fn get_messages(&self, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned> {
        store_impls::get_messages(self, agent_id).await
    }

    async fn ack_message(&self, message_id: String) -> Result<bool, ErrorObjectOwned> {
        store_impls::ack_message(self, message_id).await
    }

    async fn list_tools(&self, agent_id: String) -> Result<Vec<McpToolInfo>, ErrorObjectOwned> {
        agent_impls::list_tools(self, agent_id).await
    }

    async fn call_tool(&self, agent_id: String, tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, ErrorObjectOwned> {
        agent_impls::call_tool(self, agent_id, tool_name, arguments).await
    }

    async fn a2a_discover(&self, url: String) -> Result<AgentCard, ErrorObjectOwned> {
        agent_impls::a2a_discover(self, url).await
    }

    async fn a2a_send(&self, url: String, skill: String, input: serde_json::Value) -> Result<TaskResponse, ErrorObjectOwned> {
        agent_impls::a2a_send(self, url, skill, input).await
    }

    async fn a2a_get_task(&self, url: String, task_id: String) -> Result<merab_transport::a2a::TaskDetails, ErrorObjectOwned> {
        agent_impls::a2a_get_task(self, url, task_id).await
    }

    async fn memory_put(&self, key: String, value: serde_json::Value, ttl_seconds: Option<u64>) -> Result<bool, ErrorObjectOwned> {
        store_impls::memory_put(self, key, value, ttl_seconds).await
    }

    async fn memory_get(&self, key: String) -> Result<Option<serde_json::Value>, ErrorObjectOwned> {
        store_impls::memory_get(self, key).await
    }

    async fn memory_delete(&self, key: String) -> Result<bool, ErrorObjectOwned> {
        store_impls::memory_delete(self, key).await
    }

    async fn memory_list(&self, prefix: Option<String>) -> Result<Vec<String>, ErrorObjectOwned> {
        store_impls::memory_list(self, prefix).await
    }

    async fn get_system_status(&self) -> Result<SystemStatus, ErrorObjectOwned> {
        let agents_summary = self.registry.list().await;
        let mut agents_stats = Vec::new();
        for agent in agents_summary {
            let memory_usage = self.supervisor.get_agent_memory(agent.id).await.unwrap_or(0);
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
        let db_path = db.path.to_string_lossy().to_string();
        let db_size_mb = std::fs::metadata(&db.path)
            .map(|m| m.len() / 1024 / 1024)
            .unwrap_or(0);
        let project_path = db.get_memory("project.root_path")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string());
        let session_count = db.get_project_stats(&project_path)
            .map(|s| s.total_sessions)
            .unwrap_or(0);
        Ok(SystemStatus {
            agents: agents_stats,
            proxy: ProxyStats { cache_hits: hits, total_requests: total },
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
            db_path,
            db_size_mb,
            session_count,
        })
    }

    async fn ai_chat(&self, message: String, context_json: String) -> Result<AiResponse, ErrorObjectOwned> {
        let ctx: Vec<ChatMessage> = serde_json::from_str(&context_json)
            .map_err(|e| to_rpc_error(MerabError::InvalidManifest(e.to_string())))?;
        ai_methods::handle_ai_chat(&self.config, &self.mcp_manager, &self.db, message, ctx).await
    }

    async fn ai_orchestrate(&self, task: String) -> Result<AiResponse, ErrorObjectOwned> {
        ai_methods::handle_ai_orchestrate(&self.config, &self.mcp_manager, &self.db, task, None, false).await
    }

    async fn ai_orchestrate_stream(&self, task: String, event_file: String) -> Result<AiResponse, ErrorObjectOwned> {
        let path = std::path::PathBuf::from(event_file);
        ai_methods::handle_ai_orchestrate(&self.config, &self.mcp_manager, &self.db, task, Some(path), false).await
    }

    async fn ai_orchestrate_with_tests(&self, task: String, event_file: String) -> Result<AiResponse, ErrorObjectOwned> {
        let path = std::path::PathBuf::from(event_file);
        ai_methods::handle_ai_orchestrate(&self.config, &self.mcp_manager, &self.db, task, Some(path), true).await
    }

    async fn ai_execute_tool(&self, tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, ErrorObjectOwned> {
        ai_methods::handle_execute_tool(&self.mcp_manager, tool_name, arguments).await
    }

    async fn ai_plan(&self, task: String) -> Result<serde_json::Value, ErrorObjectOwned> {
        ai_methods::handle_ai_plan(&self.config, &self.mcp_manager, task).await
    }

    async fn ai_execute_plan(&self, plan_json: String) -> Result<serde_json::Value, ErrorObjectOwned> {
        ai_methods::handle_ai_execute_plan(&self.config, &self.mcp_manager, &self.db, plan_json, None).await
    }

    async fn session_get_last(&self, project_path: String) -> Result<Option<Session>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.get_last_session(&project_path)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn session_list(&self, project_path: String, limit: u32) -> Result<Vec<Session>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.list_sessions(&project_path, limit)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn session_stats(&self, project_path: String) -> Result<ProjectStats, ErrorObjectOwned> {
        let db = self.db.lock().await;
        let stats = db.get_project_stats(&project_path)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        Ok(ProjectStats {
            total_sessions: stats.total_sessions,
            total_tokens_input: stats.total_tokens_input,
            total_tokens_output: stats.total_tokens_output,
            total_cost_usd: stats.total_cost_usd,
        })
    }

    async fn conv_create(&self, project_path: String) -> Result<merab_store::ConvSummary, ErrorObjectOwned> {
        let id = uuid::Uuid::new_v4().to_string();
        let db = self.db.lock().await;
        db.create_conversation(&id, &project_path)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        db.get_last_conversation(&project_path)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?
            .ok_or_else(|| to_rpc_error(MerabError::Store("Failed to get created conversation".to_string())))
    }

    async fn conv_add_message(&self, conv_id: String, role: String, content: String) -> Result<bool, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.add_message(&conv_id, &role, &content)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        
        if role == "user" {
            let title = content.chars().take(50).collect::<String>();
            let _ = db.update_conversation_title(&conv_id, &title);
        }
        
        Ok(true)
    }

    async fn conv_get_messages(&self, conv_id: String) -> Result<Vec<merab_store::ConvMessage>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.get_conversation_messages(&conv_id)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn conv_list(&self, project_path: String, limit: u32) -> Result<Vec<merab_store::ConvSummary>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.list_conversations(&project_path, limit)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn conv_get_last(&self, project_path: String) -> Result<Option<merab_store::ConvSummary>, ErrorObjectOwned> {
        let db = self.db.lock().await;
        db.get_last_conversation(&project_path)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
    }

    async fn job_submit(&self, task: String) -> Result<String, ErrorObjectOwned> {
        job_impls::job_submit(self, task).await
    }

    async fn job_status(&self, job_id: String) -> Result<Option<crate::jobs::JobSummary>, ErrorObjectOwned> {
        job_impls::job_status(self, job_id).await
    }

    async fn job_log(&self, job_id: String) -> Result<String, ErrorObjectOwned> {
        job_impls::job_log(self, job_id).await
    }

    async fn job_list(&self, limit: Option<u32>) -> Result<Vec<crate::jobs::JobSummary>, ErrorObjectOwned> {
        job_impls::job_list(self, limit).await
    }

    async fn job_cancel(&self, job_id: String) -> Result<bool, ErrorObjectOwned> {
        job_impls::job_cancel(self, job_id).await
    }

    async fn index_build(&self, project_path: String) -> Result<u64, ErrorObjectOwned> {
        crate::rpc::index_impls::index_build(self, project_path).await
    }

    async fn index_search(&self, project_path: String, query: String, kind: Option<String>, limit: usize) -> Result<Vec<merab_store::IndexedSymbol>, ErrorObjectOwned> {
        crate::rpc::index_impls::index_search(self, project_path, query, kind, limit).await
    }

    async fn project_list(&self, limit: Option<u32>) -> Result<Vec<ProjectInfo>, ErrorObjectOwned> {
        project_impls::project_list(self, limit).await
    }

    async fn project_touch(&self, path: String) -> Result<(), ErrorObjectOwned> {
        project_impls::project_touch(self, path).await
    }
}
