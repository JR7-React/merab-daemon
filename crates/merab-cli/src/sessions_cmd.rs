use anyhow::Result;

use crate::client::MerabClient;

pub async fn run_list(client: &MerabClient, limit: u32) -> Result<()> {
    let project_path = std::env::current_dir()?.to_string_lossy().to_string();

    match client.session_list(&project_path, limit).await {
        Ok(sessions) if sessions.is_empty() => {
            println!("No hay sesiones previas para este proyecto.");
        }
        Ok(sessions) => {
            println!(
                "{:<8} {:<17} {:<12} {:<12} {:<8} {}",
                "ID", "FECHA", "ESTADO", "TOKENS", "COSTO", "TAREA"
            );
            println!("{}", "─".repeat(90));
            for s in sessions {
                let short_id = &s.id[..8.min(s.id.len())];
                let date = s.display_date();
                let status = s.status.to_string();
                let tokens = crate::format_number(s.total_tokens());
                let cost = s.display_cost();
                let task_preview = if s.task.len() > 35 {
                    format!("{}...", &s.task[..35])
                } else {
                    s.task.clone()
                };
                println!(
                    "{:<8} {:<17} {:<12} {:<12} {:<8} {}",
                    short_id, date, status, tokens, cost, task_preview
                );
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
    Ok(())
}

pub async fn run_stats(client: &MerabClient) -> Result<()> {
    let project_path = std::env::current_dir()?.to_string_lossy().to_string();

    match client.get_project_stats(&project_path).await {
        Ok(stats) => {
            let cwd = std::env::current_dir()?;
            let project_name = cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            println!("Proyecto: {}", project_name);
            println!("{}", "─".repeat(40));
            println!("Total sesiones: {}", stats.total_sessions);
            println!("Total tokens: {}", crate::format_number(stats.total_tokens()));
            println!("  - Input:  {}", crate::format_number(stats.total_tokens_input));
            println!("  - Output: {}", crate::format_number(stats.total_tokens_output));
            println!("Costo estimado: ${:.4}", stats.total_cost_usd);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
    Ok(())
}
