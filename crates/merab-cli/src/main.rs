mod agent_cmd;
mod ask_cmd;
mod bootstrap;
mod chat_cmd;
mod chat_handlers;
mod chat_render;
mod chat_ui;
mod client;
mod cmd_dispatch;
mod config_cmd;
mod doctor;
mod event_channel;
mod event_tail;
mod git_utils;
mod index_cmd;
mod jobs_cmd;
mod project_cmd;
mod project_context;
mod review;
mod self_upgrade;
mod sessions_cmd;
mod slash_commands;
mod watch;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use client::MerabClient;

#[derive(Parser)]
#[command(name = "merab", version, about = "Merab - AI Agent Runtime Engine")]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:9090", global = true)]
    url: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Ping,
    Register { manifest: PathBuf },
    List,
    Status { id: String },
    Start { id: String },
    Stop { id: String },
    Unregister { id: String },
    Send { from: String, to: String, content: String },
    Broadcast { from: String, content: String },
    Messages { agent_id: String },
    Ack { message_id: String },
    Tools { agent_id: String },
    Call { agent_id: String, tool_name: String, arguments: String },
    A2aDiscover { url: String },
    A2aSend { url: String, skill: String, input: String },
    #[command(subcommand)]
    Memory(MemoryCommands),
    Monitor,
    Chat { #[arg(long)] resume: bool, #[arg(long)] session: Option<String>, #[arg(long)] new: bool, #[arg(long)] list: bool },
    Ask { #[arg(index = 1)] question: String, #[arg(long, short)] test: bool, #[arg(long)] bg: bool },
    #[command(subcommand)]
    Jobs(jobs_cmd::JobsCommands),
    Plan { #[arg(index = 1)] task: String },
    ExecutePlan { #[arg(index = 1)] plan_json: String },
    Context,
    Sessions { #[arg(long, default_value = "10")] limit: u32 },
    Continue { id: Option<String> },
    Stats,
    Review { #[arg(long)] branch: Option<String>, #[arg(long)] file: Option<String>, #[arg(long)] critical: bool, #[arg(long)] output: Option<PathBuf> },
    Watch { #[arg(long, short, default_value = "**/*")] pattern: String, #[arg(long, short)] task: String, #[arg(long)] test: bool, #[arg(long, default_value = "2")] debounce: u64, #[arg(long)] quiet: bool },
    Doctor { #[arg(long)] only_errors: bool, #[arg(long)] json: bool },
    #[command(subcommand)]
    Config(config_cmd::ConfigCommands),
    #[command(subcommand)]
    Index(IndexCommands),
    /// Upgrade Merab itself using AI
    SelfUpgrade {
        /// What to add or change
        #[arg(index = 1)]
        task: String,
        /// Auto-commit if build and tests pass
        #[arg(long)]
        yes: bool,
        /// Show proposed changes without applying
        #[arg(long)]
        dry_run: bool,
    },
    /// List and manage projects
    #[command(subcommand)]
    Projects(ProjectCommands),
    /// Switch active project context
    Switch {
        /// Path to the project directory
        path: String,
    },
}

#[derive(Subcommand)]
pub enum MemoryCommands {
    Put { key: String, value: String, #[arg(long)] ttl: Option<u64> },
    Get { key: String },
    Delete { key: String },
    List { #[arg(long)] prefix: Option<String> },
}

#[derive(Subcommand)]
pub enum IndexCommands {
    Build,
    Search { query: String, #[arg(long, short)] kind: Option<String>, #[arg(long, default_value = "20")] limit: usize },
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// List all known projects
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = MerabClient::new(&cli.url)?;

    let command = match cli.command {
        Some(cmd) => cmd,
        None => Commands::Chat { resume: false, session: None, new: false, list: false },
    };

    cmd_dispatch::dispatch(command, client, &cli.url).await
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
    if !artifacts.files_created.is_empty() {
        println!("\nArchivos creados:");
        for f in &artifacts.files_created {
            println!("  + {}", f);
        }
    }
    if !artifacts.files_modified.is_empty() {
        println!("\nArchivos modificados:");
        for f in &artifacts.files_modified {
            println!("  ~ {}", f);
        }
    }
    if !artifacts.commands.is_empty() {
        println!("\nComandos ejecutados:");
        for c in &artifacts.commands {
            println!("  $ {}", c.command);
        }
    }
}

pub fn detect_merab_root() -> anyhow::Result<std::path::PathBuf> {
    // 1. Variable de entorno explícita
    if let Ok(p) = std::env::var("MERAB_SOURCE_PATH") {
        let path = std::path::PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. Buscar hacia arriba desde cwd
    let mut current = std::env::current_dir()?;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                // Check if this is the workspace root with merab-cli
                if content.contains("[workspace]") && content.contains("merab-cli") {
                    return Ok(current);
                }
            }
        }
        if !current.pop() {
            break;
        }
    }

    Err(anyhow::anyhow!(
        "No se encontró el directorio raíz de Merab.\n\
         Configura MERAB_SOURCE_PATH=/ruta/a/merab"
    ))
}
