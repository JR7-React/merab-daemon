use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use forge_core::{AgentId, AgentManifest, AgentStatus};
use forge_sandbox::JobObject;
use forge_store::Database;
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::registry::AgentRegistry;

struct ProcessHandle {
    monitor_handle: JoinHandle<()>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    _job_object: Option<JobObject>,
}

pub struct ProcessSupervisor {
    handles: Arc<Mutex<HashMap<AgentId, ProcessHandle>>>,
    registry: Arc<AgentRegistry>,
    db: Arc<Mutex<Database>>,
}

impl ProcessSupervisor {
    pub fn new(registry: Arc<AgentRegistry>, db: Arc<Mutex<Database>>) -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            registry,
            db,
        }
    }

    /// Start monitoring a child process. When the process exits, updates registry + DB.
    /// If restart_on_failure is true in the manifest, relaunches the agent.
    pub async fn start_monitoring(
        &self,
        agent_id: AgentId,
        child: Child,
        manifest: AgentManifest,
        job_object: Option<JobObject>,
    ) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let registry = self.registry.clone();
        let db = self.db.clone();
        let handles = self.handles.clone();

        let monitor_handle = tokio::spawn(monitor_task(
            agent_id, child, shutdown_rx, registry, db, handles, manifest,
        ));

        let mut map = self.handles.lock().await;
        map.insert(
            agent_id,
            ProcessHandle {
                monitor_handle,
                shutdown_tx,
                _job_object: job_object,
            },
        );
    }
    
    // ... stop_agent and shutdown_all remain similar but adapt to new ProcessHandle ...
    
    /// Stop a specific agent. Sends shutdown signal, waits for monitor to finish.
    pub async fn stop_agent(&self, agent_id: AgentId) -> bool {
        let handle = {
            let mut map = self.handles.lock().await;
            map.remove(&agent_id)
        };

        if let Some(ph) = handle {
            let _ = ph.shutdown_tx.send(());
            // JobObject will be dropped here, ensuring kill-on-close if configured
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), ph.monitor_handle)
                .await;
            true
        } else {
            false
        }
    }

    /// Shutdown all supervised agents. Called on daemon exit.
    pub async fn shutdown_all(&self) {
        let handles: Vec<(AgentId, ProcessHandle)> = {
            let mut map = self.handles.lock().await;
            map.drain().collect()
        };

        for (id, ph) in handles {
            tracing::info!(id = %id, "shutting down agent");
            let _ = ph.shutdown_tx.send(());
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), ph.monitor_handle)
                .await;
        }
    }
}

async fn monitor_task(
    agent_id: AgentId,
    mut child: Child,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    registry: Arc<AgentRegistry>,
    db: Arc<Mutex<Database>>,
    handles: Arc<Mutex<HashMap<AgentId, ProcessHandle>>>,
    manifest: AgentManifest,
) {
    // Wait for exit or shutdown signal
    let exit_status = tokio::select! {
        res = child.wait() => res,
        _ = shutdown_rx => {
            tracing::info!(id = %agent_id, "shutdown signal received, killing agent");
            let _ = child.kill().await;
            child.wait().await
        }
    };

    // Remove from handles map immediately to drop JobObject and clean up
    {
        let mut map = handles.lock().await;
        map.remove(&agent_id);
    }

    let (status, exit_code) = match exit_status {
        Ok(status) => {
            let code = status.code();
            if status.success() {
                (AgentStatus::Stopped, code)
            } else {
                (AgentStatus::Failed, code)
            }
        }
        Err(e) => {
            tracing::error!(id = %agent_id, error = %e, "error waiting for agent process");
            (AgentStatus::Failed, None)
        }
    };

    tracing::info!(id = %agent_id, ?status, ?exit_code, "agent process exited");

    if let Err(e) = registry.update_status(agent_id, status, None).await {
        tracing::error!(id = %agent_id, error = %e, "failed to update registry on exit");
    }
    registry.set_exit_info(agent_id, exit_code).await;

    {
        let db_lock = db.lock().await;
        let _ = db_lock.update_agent_exit(agent_id, status, exit_code);
    }

    // Restart if configured (spawn as separate task to break async recursion)
    if status == AgentStatus::Failed && manifest.restart_on_failure {
        tracing::info!(id = %agent_id, "restarting agent due to restart_on_failure");
        tokio::spawn(restart_agent(agent_id, manifest, registry, db, handles));
    }
}

fn restart_agent(
    agent_id: AgentId,
    manifest: AgentManifest,
    registry: Arc<AgentRegistry>,
    db: Arc<Mutex<Database>>,
    handles: Arc<Mutex<HashMap<AgentId, ProcessHandle>>>,
) -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let mut cmd = tokio::process::Command::new(&manifest.command);
        cmd.args(&manifest.args);
        if let Some(dir) = &manifest.working_dir {
            cmd.current_dir(dir);
        }
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        // Create new Job Object for the restarted process
        let job_object = match JobObject::new() {
            Ok(job) => Some(job),
            Err(e) => {
                tracing::warn!(id = %agent_id, error = %e, "failed to create job object for restart");
                None
            }
        };

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                
                // Assign to Job Object
                if let (Some(pid), Some(job)) = (pid, &job_object) {
                     if let Err(e) = job.assign_process(pid) {
                         tracing::warn!(id = %agent_id, error = %e, "failed to assign restarted process to job object");
                     }
                }

                if let Err(e) = registry
                    .update_status(agent_id, AgentStatus::Running, pid)
                    .await
                {
                    tracing::error!(id = %agent_id, error = %e, "failed to update registry on restart");
                    return;
                }
                {
                    let db_lock = db.lock().await;
                    let _ = db_lock.update_agent_status(agent_id, AgentStatus::Running, pid);
                }
                tracing::info!(id = %agent_id, pid = ?pid, "agent restarted");

                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

                let reg = registry.clone();
                let db2 = db.clone();
                let handles2 = handles.clone();
                let manifest2 = manifest.clone();

                let monitor_handle = tokio::spawn(monitor_task(
                    agent_id, child, shutdown_rx, reg, db2, handles2, manifest2,
                ));

                let mut map = handles.lock().await;
                map.insert(
                    agent_id,
                    ProcessHandle {
                        monitor_handle,
                        shutdown_tx,
                        _job_object: job_object,
                    },
                );
            }
            Err(e) => {
                tracing::error!(id = %agent_id, error = %e, "failed to restart agent");
            }
        }
    })
}
