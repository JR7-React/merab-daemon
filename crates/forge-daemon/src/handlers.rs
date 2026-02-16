use std::sync::Arc;

use forge_core::{AgentManifest, AgentRecord, AgentStatus, AgentSummary};
use forge_store::Database;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use tokio::sync::Mutex;
use uuid::Uuid;

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
}

pub struct ForgeRpc {
    pub registry: Arc<AgentRegistry>,
    pub db: Arc<Mutex<Database>>,
    pub supervisor: Arc<ProcessSupervisor>,
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
        // Persist to DB
        {
            let db = self.db.lock().await;
            db.insert_agent(&record).map_err(to_rpc_error)?;
        }
        // Register in memory
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
        let mut cmd = tokio::process::Command::new(&manifest.command);
        cmd.args(&manifest.args);
        if let Some(dir) = &manifest.working_dir {
            cmd.current_dir(dir);
        }
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

        // Persist status
        {
            let db = self.db.lock().await;
            let _ = db.update_agent_status(uuid, AgentStatus::Running, pid);
        }

        // Hand off child to supervisor for monitoring
        self.supervisor
            .start_monitoring(uuid, child, record.manifest.clone())
            .await;

        tracing::info!(id = %uuid, pid = ?pid, "agent started");
        self.registry.get(uuid).await.map_err(to_rpc_error)
    }

    async fn stop_agent(&self, id: String) -> Result<AgentRecord, ErrorObjectOwned> {
        let uuid = parse_id(&id)?;
        let record = self.registry.get(uuid).await.map_err(to_rpc_error)?;

        if record.status != AgentStatus::Running {
            return Err(to_rpc_error(format!("agent not running: {uuid}")));
        }

        // Delegate to supervisor (kills child + waits)
        let stopped = self.supervisor.stop_agent(uuid).await;

        if !stopped {
            // Fallback: agent not in supervisor (shouldn't happen, but be safe)
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

        // Persist status
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

        self.registry.remove(uuid).await.map_err(to_rpc_error)?;

        // Remove from DB
        {
            let db = self.db.lock().await;
            db.delete_agent(uuid).map_err(to_rpc_error)?;
        }

        tracing::info!(id = %uuid, "agent unregistered");
        Ok(true)
    }
}
