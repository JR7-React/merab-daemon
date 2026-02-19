use crate::{AgentId, AgentStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub agents: Vec<AgentStats>,
    pub proxy: ProxyStats,
    pub node_info: NodeInfo,
    pub ai: AiInfo,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiInfo {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub total_sessions: u64,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub total_cost_usd: f64,
}

impl ProjectStats {
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens_input + self.total_tokens_output
    }
}
