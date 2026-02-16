mod client;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use forge_core::AgentManifest;

use client::ForgeClient;

#[derive(Parser)]
#[command(name = "forge", version, about = "Forge Agent Runtime CLI")]
struct Cli {
    /// Daemon URL
    #[arg(long, default_value = "http://127.0.0.1:9090", global = true)]
    url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Health check — returns "pong"
    Ping,
    /// Register a new agent from a manifest TOML file
    Register {
        /// Path to agent manifest (.toml)
        manifest: PathBuf,
    },
    /// List all registered agents
    List,
    /// Get details of a specific agent
    Status {
        /// Agent ID (UUID)
        id: String,
    },
    /// Start an agent process
    Start {
        /// Agent ID (UUID)
        id: String,
    },
    /// Stop a running agent
    Stop {
        /// Agent ID (UUID)
        id: String,
    },
    /// Remove an agent from the registry
    Unregister {
        /// Agent ID (UUID)
        id: String,
    },
    /// Send a message from one agent to another
    Send {
        /// Sender agent ID (UUID)
        from: String,
        /// Recipient agent ID (UUID)
        to: String,
        /// Message content
        content: String,
    },
    /// Broadcast a message from an agent to all others
    Broadcast {
        /// Sender agent ID (UUID)
        from: String,
        /// Message content
        content: String,
    },
    /// Get pending messages for an agent
    Messages {
        /// Agent ID (UUID)
        agent_id: String,
    },
    /// Acknowledge a received message
    Ack {
        /// Message ID (UUID)
        message_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = ForgeClient::new(&cli.url)?;

    match cli.command {
        Commands::Ping => {
            let result = client.ping().await?;
            println!("{result}");
        }
        Commands::Register { manifest } => {
            let content = std::fs::read_to_string(&manifest)?;
            let agent_manifest: AgentManifest = toml::from_str(&content)?;
            let record = client.register_agent(agent_manifest).await?;
            println!("Registered agent: {} (id: {})", record.manifest.name, record.id);
        }
        Commands::List => {
            let agents = client.list_agents().await?;
            if agents.is_empty() {
                println!("No agents registered.");
            } else {
                println!("{:<38} {:<20} {:<12} {:<8} {:>6}", "ID", "NAME", "PROTOCOL", "STATUS", "PID");
                println!("{}", "-".repeat(84));
                for a in agents {
                    println!(
                        "{:<38} {:<20} {:<12} {:<8} {:>6}",
                        a.id,
                        a.name,
                        format!("{:?}", a.protocol).to_lowercase(),
                        format!("{:?}", a.status).to_lowercase(),
                        a.pid.map(|p| p.to_string()).unwrap_or_default(),
                    );
                }
            }
        }
        Commands::Status { id } => {
            let record = client.get_agent(&id).await?;
            println!("{}", serde_json::to_string_pretty(&record)?);
        }
        Commands::Start { id } => {
            let record = client.start_agent(&id).await?;
            println!("Started agent {} (pid: {:?})", record.manifest.name, record.pid);
        }
        Commands::Stop { id } => {
            let record = client.stop_agent(&id).await?;
            println!("Stopped agent {}", record.manifest.name);
        }
        Commands::Unregister { id } => {
            client.unregister_agent(&id).await?;
            println!("Unregistered agent {id}");
        }
        Commands::Send { from, to, content } => {
            let msg = client.send_message(&from, &to, &content).await?;
            println!("Message sent (id: {})", msg.id);
        }
        Commands::Broadcast { from, content } => {
            let msg = client.broadcast_message(&from, &content).await?;
            println!("Broadcast sent (id: {})", msg.id);
        }
        Commands::Messages { agent_id } => {
            let messages = client.get_messages(&agent_id).await?;
            if messages.is_empty() {
                println!("No pending messages.");
            } else {
                println!("{:<38} {:<38} {:<10} {}", "MESSAGE ID", "FROM", "TYPE", "CONTENT");
                println!("{}", "-".repeat(100));
                for m in messages {
                    let msg_type = if m.to_agent.is_some() { "direct" } else { "broadcast" };
                    println!(
                        "{:<38} {:<38} {:<10} {}",
                        m.id, m.from_agent, msg_type, m.content
                    );
                }
            }
        }
        Commands::Ack { message_id } => {
            client.ack_message(&message_id).await?;
            println!("Message {message_id} acknowledged.");
        }
    }

    Ok(())
}
