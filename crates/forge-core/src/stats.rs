use serde::{Deserialize, Serialize};
use crate::{AgentStatus, AgentId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub agents: Vec<AgentStats>,
    pub proxy: ProxyStats,
    pub node_info: NodeInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub id: AgentId,
    pub name: String,
    pub status: AgentStatus,
    pub memory_usage_bytes: usize,
    pub memory_limit_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStats {
    pub cache_hits: u64,
    pub total_requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub version: String,
    pub uptime_seconds: u64,
    pub rpc_port: u16,
    pub a2a_port: u16,
    pub proxy_port: u16,
}
