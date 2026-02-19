use super::SlashCommandResult;

pub async fn handle() -> SlashCommandResult {
    let ctx = crate::project_context::ProjectContext::detect(std::path::Path::new("."));
    
    let mut lines = vec!["Project Context:".to_string()];
    lines.push(format!("  Name: {}", ctx.name));
    lines.push(format!("  Language: {}", ctx.language));
    lines.push(format!("  Description: {}", ctx.description));
    
    if !ctx.key_files.is_empty() {
        lines.push(format!("  Key files: {}", ctx.key_files.join(", ")));
    }
    
    if !ctx.dependencies.is_empty() {
        let deps = ctx.dependencies.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
        lines.push(format!("  Dependencies (first 5): {}", deps));
    }
    
    SlashCommandResult::Output(lines)
}
