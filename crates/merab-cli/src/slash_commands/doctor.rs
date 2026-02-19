use crate::client::MerabClient;
use crate::doctor;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient) -> SlashCommandResult {
    let report = doctor::run_doctor(Some(client), false).await;
    
    let mut lines = vec!["Health Check Results:".to_string()];
    
    for check in &report.checks {
        let status = if check.passed { "✓" } else { "✗" };
        lines.push(format!("  {} {}", status, check.name));
        if !check.message.is_empty() {
            lines.push(format!("    {}", check.message));
        }
    }
    
    lines.push(String::new());
    if report.has_errors() {
        lines.push("Some checks failed. Run 'merab doctor' for details.".to_string());
    } else {
        lines.push("All checks passed!".to_string());
    }
    
    SlashCommandResult::Output(lines)
}
