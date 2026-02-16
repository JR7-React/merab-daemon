use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use jsonrpsee::server::Server;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use forge_daemon::handlers::{ForgeApiServer, ForgeRpc};
use forge_daemon::registry::AgentRegistry;
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

    let db = Arc::new(Mutex::new(db));

    let rpc = ForgeRpc {
        registry: registry.clone(),
        db,
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], 9090));
    let server = Server::builder().build(addr).await?;
    let handle = server.start(rpc.into_rpc());

    tracing::info!(%addr, "forge daemon listening");

    handle.stopped().await;
    Ok(())
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
