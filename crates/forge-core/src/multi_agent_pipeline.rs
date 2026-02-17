use crate::agent::AgentId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub subtasks: Vec<Task>,
    pub assigned_agent: Option<AgentId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Planner
    }

    /// Decomposes a high-level task into subtasks.
    /// In a real implementation, this would use an LLM.
    pub fn decompose(&self, task_description: &str) -> Task {
        let task_id = uuid::Uuid::new_v4().to_string();
        
        // Simple heuristic decomposition for demonstration
        let subtasks = vec![
            Task {
                id: uuid::Uuid::new_v4().to_string(),
                description: format!("Research phase for: {}", task_description),
                status: TaskStatus::Pending,
                subtasks: vec![],
                assigned_agent: None,
            },
            Task {
                id: uuid::Uuid::new_v4().to_string(),
                description: format!("Implementation phase for: {}", task_description),
                status: TaskStatus::Pending,
                subtasks: vec![],
                assigned_agent: None,
            },
            Task {
                id: uuid::Uuid::new_v4().to_string(),
                description: format!("Verification phase for: {}", task_description),
                status: TaskStatus::Pending,
                subtasks: vec![],
                assigned_agent: None,
            },
        ];

        Task {
            id: task_id,
            description: task_description.to_string(),
            status: TaskStatus::Pending,
            subtasks,
            assigned_agent: None,
        }
    }
}
