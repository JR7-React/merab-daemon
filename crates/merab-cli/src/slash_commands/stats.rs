use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient) -> SlashCommandResult {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    match client.get_project_stats(&cwd).await {
        Ok(stats) => SlashCommandResult::Output(vec![
            "Project Statistics:".to_string(),
            format!("  Sessions: {}", stats.total_sessions),
            format!("  Total tokens input: {}", stats.total_tokens_input),
            format!("  Total tokens output: {}", stats.total_tokens_output),
            format!("  Estimated cost: ${:.4}", stats.total_cost_usd),
        ]),
        Err(e) => SlashCommandResult::Error(format!("Failed to get stats: {e}")),
    }
}
