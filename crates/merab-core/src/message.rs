use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AgentId;

pub type MessageId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Delivered,
    Acknowledged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub from_agent: AgentId,
    /// None = broadcast to all agents
    pub to_agent: Option<AgentId>,
    pub content: String,
    pub status: MessageStatus,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

impl Message {
    pub fn new(from: AgentId, to: Option<AgentId>, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_agent: from,
            to_agent: to,
            content,
            status: MessageStatus::Pending,
            created_at: Utc::now(),
            delivered_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub from: AgentId,
    pub to: Option<AgentId>,
    pub content: String,
}
