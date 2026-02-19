use merab_core::{MerabError, ProjectInfo};
use jsonrpsee::types::ErrorObjectOwned;

use super::server::{to_rpc_error, MerabRpc};

pub async fn project_list(rpc: &MerabRpc, limit: Option<u32>) -> Result<Vec<ProjectInfo>, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.list_projects(limit.unwrap_or(20) as usize)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
}

pub async fn project_touch(rpc: &MerabRpc, path: String) -> Result<(), ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.touch_project(&path)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
}
