mod agent_cmd;
mod ask_cmd;
mod bootstrap;
mod chat_cmd;
mod chat_render;
mod chat_ui;
mod client;
mod config_cmd;
mod doctor;
mod event_tail;
mod git_utils;
mod jobs_cmd;
mod project_context;
mod review;
mod sessions_cmd;
mod watch;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
        manifest: PathBuf,
    },
    /// List all registered agents
    List,
    /// Get details of a specific agent
    Status { id: String },
    /// Start an agent process
    Start { id: String },
    /// Stop a running agent
    Stop { id: String },
    /// Remove an agent from the registry
    Unregister { id: String },
    /// Send a message from one agent to another
    Send { from: String, to: String, content: String },
    /// Broadcast a message from an agent to all others
    Broadcast { from: String, content: String },
    /// Get pending messages for an agent
    Messages { agent_id: String },
    /// Acknowledge a received message
    Ack { message_id: String },
    /// List tools exposed by an MCP agent
    Tools { agent_id: String },
    /// Call a tool on an MCP agent
    Call {
        agent_id: String,
        tool_name: String,
        #[arg(default_value = "{}")]
        arguments: String,
    },
    /// Discover a remote A2A agent
    A2aDiscover { url: String },
    /// Send a task to a remote A2A agent
    A2aSend {
        url: String,
        skill: String,
        #[arg(default_value = "{}")]
        input: String,
    },
    /// Shared Memory Operations
    #[command(subcommand)]
    Memory(MemoryCommands),
    /// Monitor the system in real-time (TUI)
    Monitor,
    /// Chat with Merab AI
    #[command(about = "Interactive AI chat (saves history)")]
    Chat {
        /// Resume the last conversation for this project
        #[arg(long)]
        resume: bool,
        /// Resume a specific conversation by ID prefix
        #[arg(long)]
        session: Option<String>,
        /// Start a new conversation (ignore history)
        #[arg(long)]
        new: bool,
        /// List saved conversations
        #[arg(long)]
        list: bool,
    },
    #[command(about = "Ask AI to answer a question or execute a task")]
    Ask {
        #[arg(index = 1)]
        question: String,
        #[arg(long, short)]
        test: bool,
        #[arg(long)]
        bg: bool,
    },
    /// Manage background jobs
    #[command(subcommand)]
    Jobs(jobs_cmd::JobsCommands),
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
        #[arg(long, default_value = "10")]
        limit: u32,
    },
    #[command(about = "Continue from a previous session")]
    Continue {
        id: Option<String>,
    },
    #[command(about = "Show token usage and cost stats for the project")]
    Stats,
    /// Review code changes with AI
    #[command(about = "Review code changes with AI")]
    Review {
        /// Compare against this branch/commit (default: staged changes)
        #[arg(long)]
        branch: Option<String>,
        /// Review a specific file
        #[arg(long)]
        file: Option<String>,
        /// Show only critical issues
        #[arg(long)]
        critical: bool,
        /// Save review to this file
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Watch files and run AI tasks on changes
    #[command(about = "Watch files and run AI tasks on changes")]
    Watch {
        /// Glob pattern of files to watch (default: "**/*")
        #[arg(long, short, default_value = "**/*")]
        pattern: String,
        /// Task to run when files change
        #[arg(long, short)]
        task: String,
        /// Also run tests and fix failures (like ask --test)
        #[arg(long)]
        test: bool,
        /// Debounce delay in seconds to batch rapid changes (default: 2)
        #[arg(long, default_value = "2")]
        debounce: u64,
        /// Suppress per-event output, only show errors
        #[arg(long)]
        quiet: bool,
    },
    /// Check system health and configuration
    #[command(about = "Check system health and configuration")]
    Doctor {
        /// Show only failed checks
        #[arg(long)]
        only_errors: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage Merab configuration
    #[command(subcommand)]
    Config(config_cmd::ConfigCommands),
}

#[derive(Subcommand)]
enum MemoryCommands {
    Put {
        key: String,
        value: String,
        #[arg(long)]
        ttl: Option<u64>,
    },
    Get { key: String },
    Delete { key: String },
    List {
        #[arg(long)]
        prefix: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = MerabClient::new(&cli.url)?;

    let command = match cli.command {
        Some(cmd) => cmd,
        None => Commands::Chat { resume: false, session: None, new: false, list: false },
    };

    match command {
        Commands::Init => {
            println!("Merab Init");
            println!("===========");
            match client.ping().await {
                Ok(_) => println!("[OK] Daemon already running"),
                Err(_) => {
                    print!("[..] Starting daemon... ");
                    bootstrap::init_daemon()?;
                    bootstrap::wait_for_daemon(&client).await?;
                    println!("OK");
                }
            }
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

        Commands::Register { manifest } => agent_cmd::register(&client, manifest).await?,
        Commands::List => agent_cmd::list(&client).await?,
        Commands::Status { id } => agent_cmd::status(&client, &id).await?,
        Commands::Start { id } => agent_cmd::start(&client, &id).await?,
        Commands::Stop { id } => agent_cmd::stop(&client, &id).await?,
        Commands::Unregister { id } => agent_cmd::unregister(&client, &id).await?,
        Commands::Send { from, to, content } => agent_cmd::send(&client, &from, &to, &content).await?,
        Commands::Broadcast { from, content } => agent_cmd::broadcast(&client, &from, &content).await?,
        Commands::Messages { agent_id } => agent_cmd::messages(&client, &agent_id).await?,
        Commands::Ack { message_id } => agent_cmd::ack(&client, &message_id).await?,
        Commands::Tools { agent_id } => agent_cmd::tools(&client, &agent_id).await?,
        Commands::Call { agent_id, tool_name, arguments } => {
            agent_cmd::call(&client, &agent_id, &tool_name, &arguments).await?
        }
        Commands::A2aDiscover { url } => agent_cmd::a2a_discover(&client, &url).await?,
        Commands::A2aSend { url, skill, input } => {
            agent_cmd::a2a_send(&client, &url, &skill, &input).await?
        }

        Commands::Memory(cmd) => {
            match cmd {
                MemoryCommands::Put { key, value, ttl } => {
                    let json_value: serde_json::Value =
                        serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
                    client.memory_put(&key, json_value, ttl).await?;
                    println!("Stored key: {}", key);
                }
                MemoryCommands::Get { key } => {
                    match client.memory_get(&key).await? {
                        Some(val) => println!("{}", serde_json::to_string_pretty(&val)?),
                        None => println!("Key not found or expired."),
                    }
                }
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
            }
        }

        Commands::Monitor => {
            let app = merab_tui::TuiApp::new();
            let client = Arc::new(client);
            merab_tui::run_tui(app, move || {
                let client = client.clone();
                async move { client.get_system_status().await }
            })
            .await?;
        }

        Commands::Chat { resume, session, new, list } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            chat_cmd::run_chat(&ready_client, resume, session, new, list).await?;
        }

        Commands::Ask { question, test, bg } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            ask_cmd::run_ask(&ready_client, &question, test, bg).await?;
        }

        Commands::Jobs(cmd) => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            jobs_cmd::run(&ready_client, cmd).await?;
        }

        Commands::Plan { task } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            match ready_client.ai_plan(&task).await {
                Ok(plan) => println!("{}", serde_json::to_string_pretty(&plan).unwrap()),
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::ExecutePlan { plan_json } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            match ready_client.ai_execute_plan(&plan_json).await {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
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
            sessions_cmd::run_list(&ready_client, limit).await?;
        }

        Commands::Continue { id } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            ask_cmd::run_continue(&ready_client, id).await?;
        }

        Commands::Stats => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            sessions_cmd::run_stats(&ready_client).await?;
        }

        Commands::Review { branch, file, critical, output } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;

            let diff = review::get_review_diff(&ready_client, branch.as_deref(), file.as_deref()).await?;

            if diff.trim().is_empty() {
                println!("No hay cambios para revisar.");
                println!("Usa 'git add' para stagear cambios, o '--branch <rama>' para comparar.");
                return Ok(());
            }

            let line_count = diff.lines().count();
            println!("[merab] Analizando diff ({} líneas)...", line_count);

            let task = review::build_review_task(&diff, critical);

            let event_file = std::env::temp_dir()
                .join(format!("merab-review-{}.jsonl", std::process::id()));
            let event_file_str = event_file.to_string_lossy().to_string();

            let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let tail_handle = event_tail::start_event_tail(&event_file, running.clone());

            let result = ready_client.ai_orchestrate_stream(&task, &event_file_str).await;

            running.store(false, std::sync::atomic::Ordering::Relaxed);
            let _ = tail_handle.join();
            let _ = std::fs::remove_file(&event_file);

            match result {
                Ok(resp) => {
                    println!("\n{}", resp.content);
                    if let Some(output_path) = output {
                        std::fs::write(&output_path, &resp.content)?;
                        println!("\nReview guardado en: {}", output_path.display());
                    }
                    if let Some(usage) = &resp.usage {
                        println!("\nTokens: {} input / {} output", usage.input, usage.output);
                        if let Some(cost) = merab_core::pricing::estimate_cost(&usage.model, usage.input, usage.output) {
                            println!("Costo estimado: ${:.4}", cost);
                        }
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        Commands::Watch { pattern, task, test, debounce, quiet } => {
            let ready_client = bootstrap::ensure_ready(&cli.url).await?;
            watch::run_watch(&ready_client, watch::WatchConfig {
                pattern,
                task,
                test,
                debounce,
                quiet,
            }).await?;
        }

        Commands::Doctor { only_errors, json } => {
            let client_opt = MerabClient::new(&cli.url).ok();

            let report = doctor::run_doctor(client_opt.as_ref(), only_errors).await;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                doctor::print_report(&report);
            }

            if report.has_errors() {
                std::process::exit(1);
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
    }

    Ok(())
}

pub(crate) fn format_number(n: u64) -> String {
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

pub(crate) fn print_artifact_summary(artifacts: &merab_core::artifact::ArtifactLog) {
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
