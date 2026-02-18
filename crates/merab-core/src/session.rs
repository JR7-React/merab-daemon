use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactLog;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_path: String,
    pub task: String,
    pub summary: String,
    pub artifacts: ArtifactLog,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Fecha formateada para mostrar al usuario.
    pub fn display_date(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Completed,
    Partial,
    Failed,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Completed => write!(f, "completada"),
            SessionStatus::Partial => write!(f, "parcial"),
            SessionStatus::Failed => write!(f, "fallida"),
        }
    }
}
