use crate::agent::AgentId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Persona {
    #[default]
    Engineer,
    Coder,
    Reviewer,
    QA,
}

impl Persona {
    pub fn as_str(&self) -> &'static str {
        match self {
            Persona::Engineer => "engineer",
            Persona::Coder => "coder",
            Persona::Reviewer => "reviewer",
            Persona::QA => "qa",
        }
    }
}

impl std::fmt::Display for Persona {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for Persona {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "coder" => Persona::Coder,
            "reviewer" => Persona::Reviewer,
            "qa" | "tester" => Persona::QA,
            _ => Persona::Engineer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub persona: Persona,
    pub subtasks: Vec<Task>,
    pub assigned_agent: Option<AgentId>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}
