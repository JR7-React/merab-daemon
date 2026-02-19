use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient) -> SlashCommandResult {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    match client.session_list(&cwd, 10).await {
        Ok(sessions) => {
            let mut lines = vec!["Recent sessions:".to_string()];
            for s in &sessions {
                lines.push(format!(
                    "  {} — {} [{}]",
                    s.id.to_string().split('-').next().unwrap_or("?"),
                    &s.task[..s.task.len().min(40)],
                    s.status
                ));
            }
            if sessions.is_empty() {
                lines.push("  (none)".to_string());
            }
            SlashCommandResult::Output(lines)
        }
        Err(e) => SlashCommandResult::Error(format!("Failed to list sessions: {e}")),
    }
}
