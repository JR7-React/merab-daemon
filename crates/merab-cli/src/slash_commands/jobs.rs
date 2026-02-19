use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient) -> SlashCommandResult {
    match client.job_list(Some(10)).await {
        Ok(jobs) => {
            let mut lines = vec!["Background Jobs:".to_string()];
            
            if jobs.is_empty() {
                lines.push("  (no jobs)".to_string());
            } else {
                for j in &jobs {
                    lines.push(format!(
                        "  {} — {} [{}]",
                        &j.id[..j.id.len().min(8)],
                        &j.task[..j.task.len().min(30)],
                        j.status
                    ));
                }
            }
            
            SlashCommandResult::Output(lines)
        }
        Err(e) => SlashCommandResult::Error(format!("Failed to list jobs: {e}")),
    }
}
