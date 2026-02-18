use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MerabError {
    #[error("agent not found: {0}")]
    AgentNotFound(Uuid),

    #[error("agent already exists: {0}")]
    AlreadyExists(String),

    #[error("agent not running: {0}")]
    NotRunning(Uuid),

    #[error("agent already running: {0}")]
    AlreadyRunning(Uuid),

    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("memory key not found: {0}")]
    MemoryKeyNotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("task execution failed: {0}")]
    TaskFailed(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("AI error: {0}")]
    AiError(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl MerabError {
    pub fn code(&self) -> i32 {
        match self {
            MerabError::AgentNotFound(_) => -32001,
            MerabError::AlreadyExists(_) => -32002,
            MerabError::NotRunning(_) => -32003,
            MerabError::AlreadyRunning(_) => -32004,
            MerabError::ToolNotFound(_) => -32005,
            MerabError::MessageNotFound(_) => -32006,
            MerabError::MemoryKeyNotFound(_) => -32007,
            MerabError::PermissionDenied(_) => -32008,
            MerabError::InvalidManifest(_) => -32602, // Invalid params
            MerabError::InvalidInput(_) => -32602,    // Invalid params
            MerabError::TaskFailed(_) => -32009,
            MerabError::Store(_) => -32010,
            MerabError::Transport(_) => -32011,
            MerabError::AiError(_) => -32012,
            MerabError::Internal(_) => -32603, // Internal error
            MerabError::Other(_) => -32000,    // Generic
        }
    }
}
