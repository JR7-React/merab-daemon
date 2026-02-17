use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use jsonrpsee::server::Server;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use forge_core::AgentStatus;
use forge_daemon::a2a_server::{start_a2a_server, A2AContext};
use forge_daemon::handlers::{ForgeApiServer, ForgeRpc};
use forge_daemon::mcp_manager::McpManager;
use forge_daemon::process::is_process_alive;
use forge_daemon::registry::AgentRegistry;
use forge_daemon::supervisor::ProcessSupervisor;
use forge_store::Database;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let data_dir = dirs_data_dir().join("forge");
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("forge.db");

    tracing::info!(path = %db_path.display(), "opening database");
    let db = Database::open(&db_path)?;

    let registry = Arc::new(AgentRegistry::new());
    registry.load_from_db(&db)?;

    // Reconcile: mark stale "Running" agents as Failed
    reconcile_stale_agents(&db, &registry);

    let db = Arc::new(Mutex::new(db));
    let supervisor = Arc::new(ProcessSupervisor::new(registry.clone(), db.clone()));
    let mcp_manager = Arc::new(McpManager::new());

    // Start A2A Server
    let a2a_ctx = Arc::new(A2AContext {
        db: db.clone(),
        mcp_manager: mcp_manager.clone(),
    });
    let a2a_addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tokio::spawn(async move {
        if let Err(e) = start_a2a_server(a2a_addr, a2a_ctx).await {
            tracing::error!("A2A server failed: {:?}", e);
        }
    });

    let rpc = ForgeRpc {
        registry: registry.clone(),
        db,
        supervisor: supervisor.clone(),
        mcp_manager: mcp_manager.clone(),
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], 9090));
    let server = Server::builder().build(addr).await?;
    let handle = server.start(rpc.into_rpc());

    tracing::info!(%addr, "forge daemon listening");

    // Wait for ctrl+c or server stop
    tokio::select! {
        _ = handle.stopped() => {
            tracing::info!("server stopped");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("ctrl+c received, shutting down");
        }
    }

    // Graceful shutdown: stop all MCP clients and supervised agents
    mcp_manager.shutdown_all().await;
    supervisor.shutdown_all().await;
    tracing::info!("all agents shut down, exiting");

    Ok(())
}

/// Check agents marked as "Running" in DB — if their process is no longer alive,
/// mark them as Failed. This handles daemon restarts where agents died while daemon was down.
fn reconcile_stale_agents(db: &Database, registry: &AgentRegistry) {
    let running = match db.list_running_agents() {
        Ok(agents) => agents,
        Err(e) => {
            tracing::error!(error = %e, "failed to list running agents for reconciliation");
            return;
        }
    };

    for record in running {
        let alive = record.pid.is_some_and(|pid| is_process_alive(pid));
        if !alive {
            tracing::warn!(
                id = %record.id,
                name = %record.manifest.name,
                pid = ?record.pid,
                "stale agent detected, marking as failed"
            );
            let _ = db.update_agent_exit(record.id, AgentStatus::Failed, None);
            if let Ok(mut agents) = registry.agents_mut() {
                if let Some(r) = agents.get_mut(&record.id) {
                    r.status = AgentStatus::Failed;
                    r.pid = None;
                    r.stopped_at = Some(chrono::Utc::now());
                }
            }
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    if let Some(dir) = dirs_next() {
        dir
    } else {
        PathBuf::from(".")
    }
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA").ok().map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local/share"))
            })
    }
}
