use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient, args: &str) -> SlashCommandResult {
    if args.is_empty() {
        return SlashCommandResult::Error("Usage: /search <query>".to_string());
    }

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    match client.index_search(cwd, args.to_string(), None, 10).await {
        Ok(symbols) => {
            let mut lines = vec![format!("Search results for '{}':", args)];
            
            if symbols.is_empty() {
                lines.push("  (no results)".to_string());
            } else {
                for s in symbols {
                    lines.push(format!(
                        "  {} {} @ {}:{}",
                        s.kind, s.name, s.file, s.line
                    ));
                }
            }
            
            SlashCommandResult::Output(lines)
        }
        Err(e) => SlashCommandResult::Error(format!("Search failed: {e}")),
    }
}
