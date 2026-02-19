use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient) -> SlashCommandResult {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    match client.index_build(cwd).await {
        Ok(count) => SlashCommandResult::Output(vec![
            format!("Indexed {} symbols", count),
        ]),
        Err(e) => SlashCommandResult::Error(format!("Failed to build index: {e}")),
    }
}
