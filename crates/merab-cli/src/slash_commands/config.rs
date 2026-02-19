use super::SlashCommandResult;

pub async fn handle() -> SlashCommandResult {
    let config = match merab_config::MerabConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => return SlashCommandResult::Error(format!("Failed to load config: {e}")),
    };

    SlashCommandResult::Output(vec![
        "Configuration:".to_string(),
        format!("  Model: {}", config.ai.model),
        format!("  Max tokens: {}", config.ai.max_tokens),
        format!("  Temperature: {}", config.ai.temperature),
        format!("  RPC port: {}", config.daemon.rpc_port),
        format!("  Proxy port: {}", config.proxy.port),
    ])
}
