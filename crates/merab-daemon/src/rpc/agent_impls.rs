use jsonrpsee::types::ErrorObjectOwned;
use merab_core::{
    AgentManifest, AgentRecord, AgentStatus, AgentSummary, MerabError, ProtocolKind,
};
use merab_transport::{
    a2a::{A2aClient, AgentCard, TaskDetails, TaskResponse},
    mcp::McpToolInfo,
};

use super::server::{parse_id, to_rpc_error, MerabRpc};

pub async fn register_agent(
    rpc: &MerabRpc,
    manifest: AgentManifest,
) -> Result<AgentRecord, ErrorObjectOwned> {
    let record = AgentRecord::new(manifest);
    if rpc.registry.get(record.id).await.is_ok() {
        return Err(to_rpc_error(MerabError::AlreadyExists(record.id.to_string())));
    }
    {
        let db = rpc.db.lock().await;
        db.insert_agent(&record)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
    }
    rpc.registry
        .register(record.clone())
        .await
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
    tracing::info!(id = %record.id, name = %record.manifest.name, "agent registered");
    Ok(record)
}

pub async fn list_agents(rpc: &MerabRpc) -> Result<Vec<AgentSummary>, ErrorObjectOwned> {
    Ok(rpc.registry.list().await)
}

pub async fn get_agent(rpc: &MerabRpc, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
    let uuid = parse_id(&id)?;
    rpc.registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))
}

pub async fn start_agent(rpc: &MerabRpc, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
    let uuid = parse_id(&id)?;
    let record = rpc
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
        rpc.registry
            .update_status(uuid, AgentStatus::Running, None)
            .await
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        {
            let db = rpc.db.lock().await;
            let _ = db.update_agent_status(uuid, AgentStatus::Running, None);
        }
        rpc.mcp_manager.add_client(uuid, mcp_client).await;
        tracing::info!(id = %uuid, "MCP agent started");
    } else {
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        let job_object = match merab_sandbox::JobObject::new() {
            Ok(job) => {
                if rpc.config.sandbox.enabled {
                    let limit_bytes = rpc.config.sandbox.memory_limit_mb * 1024 * 1024;
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
            to_rpc_error(MerabError::Internal(format!("failed to start process: {}", e)))
        })?;
        let pid = child.id();
        if let (Some(pid), Some(job)) = (pid, &job_object) {
            if let Err(e) = job.assign_process(pid) {
                tracing::warn!(id = %uuid, error = %e, "failed to assign to sandbox");
            }
        }
        rpc.registry
            .update_status(uuid, AgentStatus::Running, pid)
            .await
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        {
            let db = rpc.db.lock().await;
            let _ = db.update_agent_status(uuid, AgentStatus::Running, pid);
        }
        rpc.supervisor
            .start_monitoring(uuid, child, record.manifest.clone(), job_object)
            .await;
        tracing::info!(id = %uuid, pid = ?pid, "agent started");
    }
    rpc.registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))
}

pub async fn stop_agent(rpc: &MerabRpc, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
    let uuid = parse_id(&id)?;
    let record = rpc
        .registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
    if record.status != AgentStatus::Running {
        return Err(to_rpc_error(MerabError::NotRunning(uuid)));
    }
    if record.manifest.protocol == ProtocolKind::Mcp {
        rpc.mcp_manager.remove_client(uuid).await;
    }
    rpc.supervisor.stop_agent(uuid).await;
    rpc.registry
        .update_status(uuid, AgentStatus::Stopped, None)
        .await
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
    {
        let db = rpc.db.lock().await;
        let _ = db.update_agent_status(uuid, AgentStatus::Stopped, None);
    }
    tracing::info!(id = %uuid, "agent stopped");
    rpc.registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))
}

pub async fn unregister_agent(rpc: &MerabRpc, id: String) -> Result<bool, ErrorObjectOwned> {
    let uuid = parse_id(&id)?;
    let record = rpc
        .registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
    if record.status == AgentStatus::Running {
        stop_agent(rpc, id).await?;
    }
    rpc.mcp_manager.remove_client(uuid).await;
    rpc.registry
        .remove(uuid)
        .await
        .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
    {
        let db = rpc.db.lock().await;
        db.delete_agent_messages(uuid)
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
        db.delete_agent(uuid)
            .map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))?;
    }
    Ok(true)
}

pub async fn list_tools(
    rpc: &MerabRpc,
    agent_id: String,
) -> Result<Vec<McpToolInfo>, ErrorObjectOwned> {
    let uuid = parse_id(&agent_id)?;
    let record = rpc
        .registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
    if record.manifest.protocol != ProtocolKind::Mcp {
        return Err(to_rpc_error(MerabError::InvalidManifest("not an MCP agent".into())));
    }
    if record.status != AgentStatus::Running {
        return Err(to_rpc_error(MerabError::NotRunning(uuid)));
    }
    let client = rpc.mcp_manager.get_client(uuid).await.ok_or_else(|| {
        to_rpc_error(MerabError::Internal(
            "MCP client not found despite agent running".into(),
        ))
    })?;
    client
        .list_tools()
        .await
        .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
}

pub async fn call_tool(
    rpc: &MerabRpc,
    agent_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let uuid = parse_id(&agent_id)?;
    let record = rpc
        .registry
        .get(uuid)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(uuid)))?;
    if record.status != AgentStatus::Running {
        return Err(to_rpc_error(MerabError::NotRunning(uuid)));
    }
    let client = rpc.mcp_manager.get_client(uuid).await.ok_or_else(|| {
        to_rpc_error(MerabError::Internal(
            "MCP client not found despite agent running".into(),
        ))
    })?;
    let args = match arguments {
        serde_json::Value::Object(map) => Some(map),
        serde_json::Value::Null => None,
        _ => {
            return Err(to_rpc_error(MerabError::InvalidManifest("args must be object".into())));
        }
    };
    let result = client
        .call_tool(tool_name, args)
        .await
        .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))?;
    serde_json::to_value(&result).map_err(|e| to_rpc_error(MerabError::Internal(e.to_string())))
}

pub async fn a2a_discover(rpc: &MerabRpc, url: String) -> Result<AgentCard, ErrorObjectOwned> {
    let _ = rpc; // no rpc state needed
    A2aClient::fetch_card(&url)
        .await
        .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
}

pub async fn a2a_send(
    rpc: &MerabRpc,
    url: String,
    skill: String,
    input: serde_json::Value,
) -> Result<TaskResponse, ErrorObjectOwned> {
    let _ = rpc;
    A2aClient::send_task(&url, &skill, input)
        .await
        .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
}

pub async fn a2a_get_task(
    rpc: &MerabRpc,
    url: String,
    task_id: String,
) -> Result<TaskDetails, ErrorObjectOwned> {
    let _ = rpc;
    A2aClient::get_task_status(&url, &task_id)
        .await
        .map_err(|e| to_rpc_error(MerabError::Transport(e.to_string())))
}
