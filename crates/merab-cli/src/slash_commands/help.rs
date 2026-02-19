use super::SlashCommandResult;
use super::COMMANDS;

pub async fn handle() -> SlashCommandResult {
    let mut lines = vec![
        "Available slash commands:".to_string(),
        String::new(),
    ];
    
    for cmd in COMMANDS.iter() {
        let aliases = if cmd.aliases.is_empty() {
            String::new()
        } else {
            format!(" (aliases: {})", cmd.aliases.join(", "))
        };
        lines.push(format!(
            "  /{}{} - {}",
            cmd.name, aliases, cmd.description
        ));
        lines.push(format!("       Usage: {}", cmd.usage));
    }
    
    lines.push(String::new());
    lines.push("Type /<command> to execute. Use Tab for autocomplete.".to_string());
    
    SlashCommandResult::Output(lines)
}
