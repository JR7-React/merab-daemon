use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient, args: &str) -> SlashCommandResult {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let session_id = if args.is_empty() {
        // Get last session
        match client.session_get_last(&cwd).await {
            Ok(Some(s)) => s.id.to_string(),
            Ok(None) => return SlashCommandResult::Error("No previous sessions found".to_string()),
            Err(e) => return SlashCommandResult::Error(format!("Failed to get last session: {e}")),
        }
    } else {
        args.to_string()
    };

    SlashCommandResult::Output(vec![
        format!("Continuing session: {}", session_id.split('-').next().unwrap_or("?")),
        "(Note: Full continue functionality requires chat restart)".to_string(),
    ])
}
