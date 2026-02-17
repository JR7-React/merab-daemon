mod client;

use std::io::{Write, stdin, stdout};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use forge_ai::ChatMessage;
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
    /// List tools exposed by an MCP agent
    Tools {
        /// Agent ID (UUID)
        agent_id: String,
    },
    /// Call a tool on an MCP agent
    Call {
        /// Agent ID (UUID)
        agent_id: String,
        /// Tool name
        tool_name: String,
        /// Arguments as JSON string (e.g. '{"key":"value"}')
        #[arg(default_value = "{}")]
        arguments: String,
    },
    /// Discover a remote A2A agent
    A2aDiscover {
        /// URL of the remote agent
        url: String,
    },
    /// Send a task to a remote A2A agent
    A2aSend {
        /// URL of the remote agent
        url: String,
        /// Skill name to invoke
        skill: String,
        /// Input arguments as JSON
        #[arg(default_value = "{}")]
        input: String,
    },
    /// Shared Memory Operations
    #[command(subcommand)]
    Memory(MemoryCommands),
    /// Monitor the system in real-time (TUI)
    Monitor,
    /// Chat with Forge AI
    Chat,
    /// Ask a single question to Forge AI (orchestration)
    Ask {
        /// The question or task
        question: String,
    },
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// Store a value in shared memory
    Put {
        key: String,
        value: String,
        #[arg(long)]
        ttl: Option<u64>,
    },
    /// Retrieve a value from shared memory
    Get { key: String },
    /// Delete a value from shared memory
    Delete { key: String },
    /// List keys in shared memory
    List {
        #[arg(long)]
        prefix: Option<String>,
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
            println!(
                "Registered agent: {} (id: {})",
                record.manifest.name, record.id
            );
        }
        Commands::List => {
            let agents = client.list_agents().await?;
            if agents.is_empty() {
                println!("No agents registered.");
            } else {
                println!(
                    "{:<38} {:<20} {:<12} {:<8} {:>6}",
                    "ID", "NAME", "PROTOCOL", "STATUS", "PID"
                );
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
            println!(
                "Started agent {} (pid: {:?})",
                record.manifest.name, record.pid
            );
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
                println!(
                    "{:<38} {:<38} {:<10} {}",
                    "MESSAGE ID", "FROM", "TYPE", "CONTENT"
                );
                println!("{}", "-".repeat(100));
                for m in messages {
                    let msg_type = if m.to_agent.is_some() {
                        "direct"
                    } else {
                        "broadcast"
                    };
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
        Commands::Tools { agent_id } => {
            let tools = client.list_tools(&agent_id).await?;
            if tools.is_empty() {
                println!("No tools available.");
            } else {
                println!("{:<30} {}", "TOOL", "DESCRIPTION");
                println!("{}", "-".repeat(70));
                for t in tools {
                    let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    println!("{:<30} {}", name, desc);
                }
            }
        }
        Commands::Call {
            agent_id,
            tool_name,
            arguments,
        } => {
            let args: serde_json::Value = serde_json::from_str(&arguments)?;
            let result = client.call_tool(&agent_id, &tool_name, args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::A2aDiscover { url } => {
            let card = client.a2a_discover(&url).await?;
            println!("{}", serde_json::to_string_pretty(&card)?);
        }
        Commands::A2aSend { url, skill, input } => {
            let args: serde_json::Value = serde_json::from_str(&input)?;
            println!("Sending task to {}...", url);
            let response = client.a2a_send(&url, &skill, args).await?;
            println!(
                "Task submitted. ID: {} (Status: {})",
                response.task_id, response.status
            );

            let mut status = response.status;
            while status != "completed" && status != "failed" && status != "cancelled" {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                print!(".");
                use std::io::Write;
                std::io::stdout().flush()?;

                let details = client.a2a_get_task(&url, &response.task_id).await?;
                status = details.status;

                if status == "completed" {
                    println!("\nTask Completed!");
                    println!("Output: {}", serde_json::to_string_pretty(&details.output)?);
                } else if status == "failed" {
                    println!("\nTask Failed!");
                    println!("Error: {:?}", details.error);
                }
            }
        }
        Commands::Memory(cmd) => match cmd {
            MemoryCommands::Put { key, value, ttl } => {
                let json_value: serde_json::Value =
                    serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
                client.memory_put(&key, json_value, ttl).await?;
                println!("Stored key: {}", key);
            }
            MemoryCommands::Get { key } => match client.memory_get(&key).await? {
                Some(val) => println!("{}", serde_json::to_string_pretty(&val)?),
                None => println!("Key not found or expired."),
            },
            MemoryCommands::Delete { key } => {
                let deleted = client.memory_delete(&key).await?;
                if deleted {
                    println!("Deleted key: {}", key);
                } else {
                    println!("Key not found.");
                }
            }
            MemoryCommands::List { prefix } => {
                let keys = client.memory_list(prefix.as_deref()).await?;
                if keys.is_empty() {
                    println!("No keys found.");
                } else {
                    for k in keys {
                        println!("{}", k);
                    }
                }
            }
        },
        Commands::Monitor => {
            let app = forge_tui::TuiApp::new();
            let client = Arc::new(client);

            forge_tui::run_tui(app, move || {
                let client = client.clone();
                async move { client.get_system_status().await }
            })
            .await?;
        }
        Commands::Chat => {
            println!("Forge AI Chat (type 'quit' or 'exit' to stop)");
            println!("{}", "-".repeat(50));

            let mut context: Vec<ChatMessage> = Vec::new();

            loop {
                print!("> ");
                stdout().flush()?;

                let mut input = String::new();
                stdin().read_line(&mut input)?;
                let input = input.trim();

                if input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") {
                    break;
                }

                if input.is_empty() {
                    continue;
                }

                match client.ai_chat(input, context.clone()).await {
                    Ok(resp) => {
                        println!("\nAI: {}\n", resp.content);
                        context.push(ChatMessage::user(input));
                        context.push(ChatMessage::assistant(resp.content));

                        if let Some(tool) = resp.tool_call {
                            println!("[Tool Call: {}]", tool.name);
                        }
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }
        }
        Commands::Ask { question } => match client.ai_orchestrate(&question).await {
            Ok(resp) => {
                println!("{}", resp.content);
                if let Some(tool) = resp.tool_call {
                    println!("\n[Orchestrated Tool Call: {}]", tool.name);
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        },
    }

    Ok(())
}
