mod bootstrap;
mod client;
mod config_cmd;
mod git_utils;
mod chat_ui;
mod project_context;
mod event_tail;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

use merab_core::AgentManifest;

use client::MerabClient;

#[derive(Parser)]
#[command(name = "merab", version, about = "Merab - AI Agent Runtime Engine")]
struct Cli {
    /// Daemon URL
    #[arg(long, default_value = "http://127.0.0.1:9090", global = true)]
    url: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Merab: start daemon and register built-in agents
    Init,
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
    /// Chat with Merab AI
    Chat,
    #[command(about = "Ask AI to answer a question or execute a task")]
    Ask {
        #[arg(index = 1)]
        question: String,
        /// Run tests automatically and fix failures
        #[arg(long, short)]
        test: bool,
        /// Run in background (returns job ID immediately)
        #[arg(long)]
        bg: bool,
    },

    /// Manage background jobs
    #[command(subcommand)]
    Jobs(JobsCommands),

    #[command(about = "Ask AI to create a plan for a task")]
    Plan {
        #[arg(index = 1)]
        task: String,
    },

    #[command(about = "Execute a plan with persona-based subtasks")]
    ExecutePlan {
        #[arg(index = 1)]
        plan_json: String,
    },

    #[command(about = "Show detected project context")]
    Context,

    #[command(about = "List recent sessions for the current project")]
    Sessions {
        /// Maximum number of sessions to show (default: 10)
        #[arg(long, default_value = "10")]
        limit: u32,
    },

    #[command(about = "Continue from a previous session")]
    Continue {
        /// Session ID to resume (default: last session)
        id: Option<String>,
    },

    #[command(about = "Show token usage and cost stats for the project")]
    Stats,

    /// Manage Merab configuration
    #[command(subcommand)]
    Config(config_cmd::ConfigCommands),
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

#[derive(Subcommand)]
enum JobsCommands {
    /// List all background jobs
    List {
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// Show log output of a job
    Log {
        /// Job ID
        id: String,
    },
    /// Wait until a job completes
    Wait {
        /// Job ID
        id: String,
    },
    /// Cancel a running job
    Cancel {
        /// Job ID
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = MerabClient::new(&cli.url)?;

    // No subcommand = enter chat (auto-bootstrap daemon + agents)
    let command = match cli.command {
        Some(cmd) => cmd,
        None => Commands::Chat,
    };

    match command {
        Commands::Init => {
            println!("Merab Init");
            println!("===========");
            
            let client = MerabClient::new(&cli.url)?;
            
            // 1. Check if daemon is running
            match client.ping().await {
                Ok(_) => println!("[OK] Daemon already running"),
                Err(_) => {
                    print!("[..] Starting daemon... ");
                    bootstrap::init_daemon()?;
                    bootstrap::wait_for_daemon(&client).await?;
                    println!("OK");
                }
            }
            
            // 2. Ensure built-in agents
            let agents = bootstrap::init_agents(&client).await?;
            
            println!("\nAgents ready:");
            for agent in &agents {
                println!("  - {} ({:?})", agent.name, agent.status);
            }
            
            println!("\nMerab is ready. Try:");
            println!("  merab chat      - Interactive chat");
            println!("  merab ask \"...\" - Ask a question");
            println!("  merab list      - List agents");
        }
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
            let app = merab_tui::TuiApp::new();
            let client = Arc::new(client);

            merab_tui::run_tui(app, move || {
                let client = client.clone();
                async move { client.get_system_status().await }
            })
            .await?;
        }
        Commands::Chat => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            chat_ui::start_chat_session(&ready_client).await?;
        }

        Commands::Ask { question, test, bg } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;

            if bg {
                match ready_client.job_submit(&question).await {
                    Ok(job_id) => println!("Job submitted: {}\nUse `merab jobs log {}` to follow progress.", job_id, job_id),
                    Err(e) => eprintln!("Error: {}", e),
                }
                return Ok(());
            }

            let event_file = std::env::temp_dir().join(format!("merab-events-{}.jsonl", std::process::id()));
            let event_file_str = event_file.to_string_lossy().to_string();

            let running = Arc::new(AtomicBool::new(true));
            let tail_handle = event_tail::start_event_tail(&event_file, running.clone());

            let result = if test {
                ready_client.ai_orchestrate_with_tests(&question, &event_file_str).await
            } else {
                ready_client.ai_orchestrate_stream(&question, &event_file_str).await
            };

            running.store(false, Ordering::Relaxed);
            let _ = tail_handle.join();

            let _ = std::fs::remove_file(&event_file);

            match result {
                Ok(resp) => {
                    if let Some(usage) = &resp.usage {
                        println!("\nTokens: {} input / {} output",
                            format_number(usage.input),
                            format_number(usage.output));
                        if let Some(cost) = merab_core::estimate_cost(&usage.model, usage.input, usage.output) {
                            println!("Costo estimado: ${:.4}", cost);
                        }
                    }
                    if let Some(artifacts) = &resp.artifacts {
                        print_artifact_summary(artifacts);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::Jobs(cmd) => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            match cmd {
                JobsCommands::List { limit } => {
                    match ready_client.job_list(Some(limit)).await {
                        Ok(jobs) if jobs.is_empty() => println!("No hay jobs activos."),
                        Ok(jobs) => {
                            println!("{:<38} {:<12} {:<20} {}", "JOB ID", "ESTADO", "CREADO", "TAREA");
                            println!("{}", "─".repeat(100));
                            for j in jobs {
                                let task_preview = if j.task.len() > 30 {
                                    format!("{}...", &j.task[..30])
                                } else {
                                    j.task.clone()
                                };
                                println!("{:<38} {:<12} {:<20} {}", j.id, j.status, j.created_at, task_preview);
                            }
                        }
                        Err(e) => eprintln!("Error: {}", e),
                    }
                }
                JobsCommands::Log { id } => {
                    match ready_client.job_log(&id).await {
                        Ok(log) => print!("{}", log),
                        Err(e) => eprintln!("Error: {}", e),
                    }
                }
                JobsCommands::Wait { id } => {
                    println!("Esperando job {}...", id);
                    loop {
                        match ready_client.job_status(&id).await {
                            Ok(Some(job)) => {
                                if job.status == "completed" || job.status == "failed" || job.status == "cancelled" {
                                    println!("Job {}: {}", id, job.status);
                                    if let Some(secs) = job.duration_secs {
                                        println!("Duración: {}s", secs);
                                    }
                                    break;
                                }
                                print!(".");
                                use std::io::Write;
                                std::io::stdout().flush()?;
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            }
                            Ok(None) => {
                                eprintln!("Job {} no encontrado.", id);
                                break;
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                break;
                            }
                        }
                    }
                }
                JobsCommands::Cancel { id } => {
                    match ready_client.job_cancel(&id).await {
                        Ok(true) => println!("Job {} cancelado.", id),
                        Ok(false) => println!("No se pudo cancelar job {} (¿ya terminó?).", id),
                        Err(e) => eprintln!("Error: {}", e),
                    }
                }
            }
        }

        Commands::Plan { task } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            match ready_client.ai_plan(&task).await {
                Ok(plan) => {
                    println!("{}", serde_json::to_string_pretty(&plan).unwrap());
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::ExecutePlan { plan_json } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            match ready_client.ai_execute_plan(&plan_json).await {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::Context => {
            let cwd = std::env::current_dir()?;
            let ctx = project_context::ProjectContext::detect(&cwd);
            ctx.display();
        }

        Commands::Sessions { limit } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            let project_path = std::env::current_dir()?
                .to_string_lossy()
                .to_string();
            match ready_client.session_list(&project_path, limit).await {
                Ok(sessions) if sessions.is_empty() => {
                    println!("No hay sesiones previas para este proyecto.");
                }
                Ok(sessions) => {
                    println!("{:<8} {:<17} {:<12} {:<12} {:<8} {}", "ID", "FECHA", "ESTADO", "TOKENS", "COSTO", "TAREA");
                    println!("{}", "─".repeat(90));
                    for s in sessions {
                        let short_id = &s.id[..8.min(s.id.len())];
                        let date = s.display_date();
                        let status = s.status.to_string();
                        let tokens = format_number(s.total_tokens());
                        let cost = s.display_cost();
                        let task_preview = if s.task.len() > 35 {
                            format!("{}...", &s.task[..35])
                        } else {
                            s.task.clone()
                        };
                        println!("{:<8} {:<17} {:<12} {:<12} {:<8} {}", short_id, date, status, tokens, cost, task_preview);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::Continue { id } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            let project_path = std::env::current_dir()?
                .to_string_lossy()
                .to_string();

            // Obtener la sesión (por ID o la última)
            let session_opt = if let Some(ref session_id) = id {
                // Buscar por ID en la lista
                match ready_client.session_list(&project_path, 50).await {
                    Ok(sessions) => sessions.into_iter().find(|s| s.id.starts_with(session_id.as_str())),
                    Err(e) => {
                        eprintln!("Error al buscar sesión: {}", e);
                        None
                    }
                }
            } else {
                match ready_client.session_get_last(&project_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        None
                    }
                }
            };

            let session = match session_opt {
                Some(s) => s,
                None => {
                    println!("No hay sesión previa para continuar en este proyecto.");
                    println!("Usa `merab ask` para iniciar una nueva sesión.");
                    return Ok(());
                }
            };

            println!("Retomando sesión {}", &session.id[..8.min(session.id.len())]);
            println!("Tarea original: {}", session.task);
            println!("Fecha: {}", session.display_date());
            println!("Estado anterior: {}", session.status);
            println!();

            // Construir prompt de continuación
            let mut continuation = format!(
                "Continuando sesión anterior.\n\nLo que se hizo:\n{}\n\n",
                session.summary
            );

            if !session.artifacts.files_created.is_empty()
                || !session.artifacts.files_modified.is_empty()
            {
                continuation.push_str("Archivos tocados:\n");
                for f in &session.artifacts.files_created {
                    continuation.push_str(&format!("  + {}\n", f));
                }
                for f in &session.artifacts.files_modified {
                    continuation.push_str(&format!("  ~ {}\n", f));
                }
                continuation.push('\n');
            }

            continuation.push_str(&format!(
                "Tarea pendiente: {}\n\nContinúa desde donde quedamos.",
                session.task
            ));

            println!("Enviando contexto al agente...\n");
            
            let event_file = std::env::temp_dir().join(format!("merab-events-{}.jsonl", std::process::id()));
            let event_file_str = event_file.to_string_lossy().to_string();
            
            let running = Arc::new(AtomicBool::new(true));
            let tail_handle = event_tail::start_event_tail(&event_file, running.clone());
            
            let result = ready_client.ai_orchestrate_stream(&continuation, &event_file_str).await;
            
            running.store(false, Ordering::Relaxed);
            let _ = tail_handle.join();
            
            let _ = std::fs::remove_file(&event_file);
            
            match result {
                Ok(resp) => {
                    if let Some(ref usage) = resp.usage {
                        println!("\nTokens: {} input / {} output", 
                            format_number(usage.input), 
                            format_number(usage.output));
                        if let Some(cost) = merab_core::estimate_cost(&usage.model, usage.input, usage.output) {
                            println!("Costo estimado: ${:.4}", cost);
                        }
                    }
                    if let Some(artifacts) = &resp.artifacts {
                        print_artifact_summary(artifacts);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::Config(cmd) => {
            match cmd {
                config_cmd::ConfigCommands::List => config_cmd::config_list()?,
                config_cmd::ConfigCommands::Get { key } => config_cmd::config_get(&key)?,
                config_cmd::ConfigCommands::Set { key, value } => config_cmd::config_set(&key, &value)?,
                config_cmd::ConfigCommands::Edit => config_cmd::config_edit()?,
                config_cmd::ConfigCommands::Path => config_cmd::config_path()?,
            }
        }

        Commands::Stats => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            let project_path = std::env::current_dir()?
                .to_string_lossy()
                .to_string();

            match ready_client.get_project_stats(&project_path).await {
                Ok(stats) => {
                    let cwd = std::env::current_dir()?;
                    let project_name = cwd.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    println!("Proyecto: {}", project_name);
                    println!("{}", "─".repeat(40));
                    println!("Total sesiones: {}", stats.total_sessions);
                    println!("Total tokens: {}", format_number(stats.total_tokens()));
                    println!("  - Input:  {}", format_number(stats.total_tokens_input));
                    println!("  - Output: {}", format_number(stats.total_tokens_output));
                    println!("Costo estimado: ${:.4}", stats.total_cost_usd);
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }

    Ok(())
}

fn print_artifact_summary(artifacts: &merab_core::artifact::ArtifactLog) {
    if artifacts.files_created.is_empty()
        && artifacts.files_modified.is_empty()
        && artifacts.commands.is_empty()
    {
        return;
    }

    println!("\n{}", "─".repeat(45));
    println!(" Resumen de cambios");
    println!("{}", "─".repeat(45));

    if !artifacts.files_created.is_empty() {
        println!(" Creados:");
        for f in &artifacts.files_created {
            println!("   + {}", f);
        }
        println!();
    }

    if !artifacts.files_modified.is_empty() {
        println!(" Modificados:");
        for f in &artifacts.files_modified {
            println!("   ~ {}", f);
        }
        println!();
    }

    if !artifacts.commands.is_empty() {
        println!(" Comandos ejecutados:");
        for cmd in &artifacts.commands {
            let status = if cmd.success { "OK" } else { "FAIL" };
            println!("   {}  ->  [{}]", cmd.command, status);
        }
        println!();
    }

    println!("{}", "─".repeat(45));
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }
    result
}
