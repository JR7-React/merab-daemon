use jsonrpsee::types::ErrorObjectOwned;

use super::server::{to_rpc_error, MerabRpc};
use crate::jobs::JobSummary;

pub async fn job_submit(rpc: &MerabRpc, task: String) -> Result<String, ErrorObjectOwned> {
    let job_id = rpc.job_manager.submit(task).await.map_err(to_rpc_error)?;

    let job_manager = rpc.job_manager.clone();
    let db = rpc.db.clone();
    let config = rpc.config.clone();
    let mcp_manager = rpc.mcp_manager.clone();
    let job_id_clone = job_id.clone();

    tokio::spawn(async move {
        if let Err(e) = job_manager.start(&job_id_clone).await {
            tracing::error!(job_id = %job_id_clone, error = %e, "Failed to start job");
            return;
        }
        job_manager.append_log(&job_id_clone, &format!("[{}] Starting task...", job_id_clone));

        match super::ai_methods::handle_ai_orchestrate(
            &config,
            &mcp_manager,
            &db,
            format!("Task: job {}", job_id_clone),
            None,
            false,
        )
        .await
        {
            Ok(response) => {
                let result = serde_json::to_string(&response).unwrap_or_default();
                if let Err(e) = job_manager.complete(&job_id_clone, &result).await {
                    tracing::error!(job_id = %job_id_clone, error = %e, "Failed to complete job");
                }
                job_manager.append_log(&job_id_clone, &format!("[{}] Completed", job_id_clone));
            }
            Err(e) => {
                if let Err(ee) = job_manager.fail(&job_id_clone, &e.to_string()).await {
                    tracing::error!(job_id = %job_id_clone, error = %ee, "Failed to fail job");
                }
                job_manager
                    .append_log(&job_id_clone, &format!("[{}] Failed: {}", job_id_clone, e));
            }
        }
    });

    Ok(job_id)
}

pub async fn job_status(
    rpc: &MerabRpc,
    job_id: String,
) -> Result<Option<JobSummary>, ErrorObjectOwned> {
    rpc.job_manager
        .get_status(&job_id)
        .await
        .map_err(to_rpc_error)
}

pub async fn job_log(rpc: &MerabRpc, job_id: String) -> Result<String, ErrorObjectOwned> {
    rpc.job_manager.get_log(&job_id).await.map_err(to_rpc_error)
}

pub async fn job_list(rpc: &MerabRpc, limit: Option<u32>) -> Result<Vec<JobSummary>, ErrorObjectOwned> {
    rpc.job_manager
        .list(limit.unwrap_or(20))
        .await
        .map_err(to_rpc_error)
}

pub async fn job_cancel(rpc: &MerabRpc, job_id: String) -> Result<bool, ErrorObjectOwned> {
    rpc.job_manager
        .cancel(&job_id)
        .await
        .map_err(to_rpc_error)
}
