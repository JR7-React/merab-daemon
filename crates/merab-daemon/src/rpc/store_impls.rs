use jsonrpsee::types::ErrorObjectOwned;
use merab_core::{MerabError, Message};

use super::server::{parse_id, to_rpc_error, MerabRpc};

// ── Messages ──────────────────────────────────────────────────────────────────

pub async fn send_message(
    rpc: &MerabRpc,
    from: String,
    to: String,
    content: String,
) -> Result<Message, ErrorObjectOwned> {
    let from_id = parse_id(&from)?;
    let to_id = parse_id(&to)?;
    rpc.registry
        .get(from_id)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(from_id)))?;
    rpc.registry
        .get(to_id)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(to_id)))?;
    let msg = Message::new(from_id, Some(to_id), content);
    {
        let db = rpc.db.lock().await;
        db.insert_message(&msg)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
    }
    Ok(msg)
}

pub async fn broadcast_message(
    rpc: &MerabRpc,
    from: String,
    content: String,
) -> Result<Message, ErrorObjectOwned> {
    let from_id = parse_id(&from)?;
    rpc.registry
        .get(from_id)
        .await
        .map_err(|_| to_rpc_error(MerabError::AgentNotFound(from_id)))?;
    let msg = Message::new(from_id, None, content);
    {
        let db = rpc.db.lock().await;
        db.insert_message(&msg)
            .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
    }
    Ok(msg)
}

pub async fn get_messages(rpc: &MerabRpc, agent_id: String) -> Result<Vec<Message>, ErrorObjectOwned> {
    let uuid = parse_id(&agent_id)?;
    let db = rpc.db.lock().await;
    db.get_messages_for(uuid)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
}

pub async fn ack_message(rpc: &MerabRpc, message_id: String) -> Result<bool, ErrorObjectOwned> {
    let uuid = parse_id(&message_id)?;
    let db = rpc.db.lock().await;
    let updated = db
        .acknowledge_message(uuid)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
    if !updated {
        return Err(to_rpc_error(MerabError::MessageNotFound(uuid)));
    }
    Ok(true)
}

// ── Memory ────────────────────────────────────────────────────────────────────

pub async fn memory_put(
    rpc: &MerabRpc,
    key: String,
    value: serde_json::Value,
    ttl_seconds: Option<u64>,
) -> Result<bool, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.put_memory(&key, &value, None, ttl_seconds)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
    let _ = db.cleanup_expired_memory();
    Ok(true)
}

pub async fn memory_get(
    rpc: &MerabRpc,
    key: String,
) -> Result<Option<serde_json::Value>, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.get_memory(&key)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
}

pub async fn memory_delete(rpc: &MerabRpc, key: String) -> Result<bool, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.delete_memory(&key)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
}

pub async fn memory_list(
    rpc: &MerabRpc,
    prefix: Option<String>,
) -> Result<Vec<String>, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.list_memory_keys(prefix.as_deref())
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))
}
