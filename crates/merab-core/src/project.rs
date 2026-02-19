use serde::{Deserialize, Serialize};

/// Information about a registered project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub session_count: u64,
    pub last_active: String,
    pub has_instructions: bool,
}
