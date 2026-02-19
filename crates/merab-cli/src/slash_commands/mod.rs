use std::sync::LazyLock;

use crate::client::MerabClient;

pub mod clear;
pub mod config;
pub mod context;
pub mod cont;
pub mod doctor;
pub mod help;
pub mod index;
pub mod jobs;
pub mod quit;
pub mod reset;
pub mod search;
pub mod sessions;
pub mod stats;

#[derive(Debug, Clone)]
pub struct SlashCommandDef {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub usage: String,
    pub needs_args: bool,
}

#[derive(Debug, Clone)]
pub enum SlashCommandResult {
    Output(Vec<String>),
    Exit,
    Silent,
    Error(String),
}

static COMMANDS: LazyLock<Vec<SlashCommandDef>> = LazyLock::new(|| {
    vec![
        SlashCommandDef {
            name: "help".to_string(),
            aliases: vec!["h".to_string()],
            description: "Show available slash commands".to_string(),
            usage: "/help".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "quit".to_string(),
            aliases: vec!["q".to_string(), "exit".to_string()],
            description: "Exit the chat".to_string(),
            usage: "/quit".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "clear".to_string(),
            aliases: vec![],
            description: "Clear the chat history".to_string(),
            usage: "/clear".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "reset".to_string(),
            aliases: vec![],
            description: "Reset context and chat history".to_string(),
            usage: "/reset".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "sessions".to_string(),
            aliases: vec![],
            description: "List recent sessions".to_string(),
            usage: "/sessions".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "continue".to_string(),
            aliases: vec!["cont".to_string()],
            description: "Continue from a previous session".to_string(),
            usage: "/continue [id]".to_string(),
            needs_args: true,
        },
        SlashCommandDef {
            name: "doctor".to_string(),
            aliases: vec![],
            description: "Run health check".to_string(),
            usage: "/doctor".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "index".to_string(),
            aliases: vec![],
            description: "Build codebase index".to_string(),
            usage: "/index".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "search".to_string(),
            aliases: vec!["s".to_string()],
            description: "Search symbols in codebase".to_string(),
            usage: "/search <query>".to_string(),
            needs_args: true,
        },
        SlashCommandDef {
            name: "stats".to_string(),
            aliases: vec![],
            description: "Show project statistics".to_string(),
            usage: "/stats".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "context".to_string(),
            aliases: vec!["ctx".to_string()],
            description: "Show detected project context".to_string(),
            usage: "/context".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "jobs".to_string(),
            aliases: vec![],
            description: "List background jobs".to_string(),
            usage: "/jobs".to_string(),
            needs_args: false,
        },
        SlashCommandDef {
            name: "config".to_string(),
            aliases: vec!["cfg".to_string()],
            description: "Show current configuration".to_string(),
            usage: "/config".to_string(),
            needs_args: false,
        },
    ]
});

pub fn parse_slash_input(input: &str) -> Option<(&str, &str)> {
    if !input.starts_with('/') {
        return None;
    }
    
    let rest = &input[1..];
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    
    if parts.is_empty() || parts[0].is_empty() {
        return None;
    }
    
    let cmd = parts[0];
    let args = parts.get(1).copied().unwrap_or("");
    
    Some((cmd, args.trim()))
}

pub fn resolve_command(name: &str) -> Option<&'static str> {
    let name_lower = name.to_lowercase();
    
    for cmd in COMMANDS.iter() {
        if cmd.name.eq_ignore_ascii_case(&name_lower) {
            return Some(cmd.name.as_str());
        }
        for alias in &cmd.aliases {
            if alias.eq_ignore_ascii_case(&name_lower) {
                return Some(cmd.name.as_str());
            }
        }
    }
    
    None
}

pub fn get_completions(input: &str) -> Vec<SlashCommandDef> {
    if !input.starts_with('/') {
        return Vec::new();
    }
    
    let rest = &input[1..];
    if rest.is_empty() {
        return COMMANDS.clone();
    }
    
    COMMANDS
        .iter()
        .filter(|cmd| {
            cmd.name.starts_with(rest) ||
            cmd.aliases.iter().any(|a| a.starts_with(rest))
        })
        .cloned()
        .collect()
}

pub async fn execute_slash_command(
    name: &str,
    args: &str,
    client: &MerabClient,
) -> SlashCommandResult {
    match name {
        "help" => help::handle().await,
        "quit" => quit::handle().await,
        "clear" => clear::handle().await,
        "reset" => reset::handle().await,
        "sessions" => sessions::handle(client).await,
        "continue" => cont::handle(client, args).await,
        "doctor" => doctor::handle(client).await,
        "index" => index::handle(client).await,
        "search" => search::handle(client, args).await,
        "stats" => stats::handle(client).await,
        "context" => context::handle().await,
        "jobs" => jobs::handle(client).await,
        "config" => config::handle().await,
        _ => SlashCommandResult::Error(format!("Unknown command: /{}", name)),
    }
}
