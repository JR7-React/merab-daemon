use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type AgentId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolKind {
    A2a,
    Mcp,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Registered,
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_protocol")]
    pub protocol: ProtocolKind,
    #[serde(default)]
    pub working_dir: Option<String>,
}

fn default_protocol() -> ProtocolKind {
    ProtocolKind::Native
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: AgentId,
    pub manifest: AgentManifest,
    pub status: AgentStatus,
    pub pid: Option<u32>,
    pub registered_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
}

impl AgentRecord {
    pub fn new(manifest: AgentManifest) -> Self {
        Self {
            id: Uuid::new_v4(),
            manifest,
            status: AgentStatus::Registered,
            pid: None,
            registered_at: Utc::now(),
            started_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: AgentId,
    pub name: String,
    pub status: AgentStatus,
    pub protocol: ProtocolKind,
    pub pid: Option<u32>,
}

impl From<&AgentRecord> for AgentSummary {
    fn from(r: &AgentRecord) -> Self {
        Self {
            id: r.id,
            name: r.manifest.name.clone(),
            status: r.status,
            protocol: r.manifest.protocol,
            pid: r.pid,
        }
    }
}
