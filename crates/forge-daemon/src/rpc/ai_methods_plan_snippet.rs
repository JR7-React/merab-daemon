
/// Handle `forge.ai.plan` — decompose a task into a plan.
pub async fn handle_ai_plan(
    _config: &ForgeConfig,
    _mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    // In the future, this will use the LLM to decompose.
    // For now, use the hardcoded heuristic in Planner.
    let planner = Planner::new();
    let plan = planner.decompose(&task);
    
    serde_json::to_value(&plan)
        .map_err(|e| to_rpc_error(ForgeError::Internal(e.to_string())))
}
