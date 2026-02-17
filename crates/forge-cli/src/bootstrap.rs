use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use forge_core::{AgentManifest, AgentStatus, ProtocolKind};

use crate::client::ForgeClient;

/// Built-in MCP agents: (name, binary_name, description)
const BUILT_IN_AGENTS: &[(&str, &str, &str)] = &[
    ("forge-fs", "forge-fs.exe", "FileSystem agent (read, write, list, search)"),
    ("forge-shell", "forge-shell.exe", "Shell agent (execute commands)"),
    ("forge-git", "forge-git.exe", "Git agent (status, add, commit, log, diff)"),
];

/// Get the directory containing the current binary (forge.exe).
fn bin_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot find own executable path")?;
    Ok(exe.parent().unwrap_or(std::path::Path::new(".")).to_path_buf())
}

/// Ensure daemon is running, agents are registered and started.
/// Returns a ready-to-use ForgeClient.
pub async fn ensure_ready(url: &str) -> Result<ForgeClient> {
    let client = ForgeClient::new(url)?;

    // 1. Check if daemon is running
    if client.ping().await.is_err() {
        eprintln!("Starting forge daemon...");
        start_daemon()?;
        wait_for_daemon(&client).await?;
        eprintln!("Daemon ready.");
    }

    // 2. Ensure built-in agents
    ensure_agents(&client).await?;

    Ok(client)
}

/// Spawn forged.exe as a detached background process.
fn start_daemon() -> Result<()> {
    let bin = bin_dir()?.join("forged.exe");

    if !bin.exists() {
        anyhow::bail!(
            "Daemon binary not found at {}. Run `cargo build` first.",
            bin.display()
        );
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_CONSOLE (0x10) so forged gets its own console window
        // we could use DETACHED_PROCESS (0x08) to hide it entirely
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        std::process::Command::new(&bin)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .with_context(|| format!("failed to start daemon: {}", bin.display()))?;
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new(&bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start daemon: {}", bin.display()))?;
    }

    Ok(())
}

/// Poll daemon until it responds to ping (max ~6 seconds).
async fn wait_for_daemon(client: &ForgeClient) -> Result<()> {
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if client.ping().await.is_ok() {
            return Ok(());
        }
    }
    anyhow::bail!("daemon did not start within 6 seconds")
}

/// Register and start built-in MCP agents if not already present.
async fn ensure_agents(client: &ForgeClient) -> Result<()> {
    let agents = client.list_agents().await.unwrap_or_default();
    let bin = bin_dir()?;

    for (name, exe_name, description) in BUILT_IN_AGENTS {
        let existing = agents.iter().find(|a| a.name == *name);

        match existing {
            Some(agent) if agent.status == AgentStatus::Running => {
                // Already running — nothing to do
            }
            Some(agent) => {
                // Registered but not running — start it
                eprintln!("Starting {}...", name);
                client
                    .start_agent(&agent.id.to_string())
                    .await
                    .with_context(|| format!("failed to start {}", name))?;
            }
            None => {
                // Not registered — register and start
                let exe_path = bin.join(exe_name);
                if !exe_path.exists() {
                    eprintln!("Warning: {} not found, skipping", exe_path.display());
                    continue;
                }

                let manifest = AgentManifest {
                    name: name.to_string(),
                    version: "0.1.0".to_string(),
                    description: description.to_string(),
                    command: exe_path.to_string_lossy().to_string(),
                    args: vec![],
                    protocol: ProtocolKind::Mcp,
                    working_dir: None,
                    restart_on_failure: false,
                };

                eprintln!("Registering {}...", name);
                match client.register_agent(manifest).await {
                    Ok(record) => {
                        client
                            .start_agent(&record.id.to_string())
                            .await
                            .with_context(|| format!("failed to start {}", name))?;
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to register {}: {}", name, e);
                    }
                }
            }
        }
    }

    // Small delay for MCP handshakes to complete
    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok(())
}
