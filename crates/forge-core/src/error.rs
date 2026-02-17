use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ForgeError {
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

impl ForgeError {
    pub fn code(&self) -> i32 {
        match self {
            ForgeError::AgentNotFound(_) => -32001,
            ForgeError::AlreadyExists(_) => -32002,
            ForgeError::NotRunning(_) => -32003,
            ForgeError::AlreadyRunning(_) => -32004,
            ForgeError::ToolNotFound(_) => -32005,
            ForgeError::MessageNotFound(_) => -32006,
            ForgeError::MemoryKeyNotFound(_) => -32007,
            ForgeError::PermissionDenied(_) => -32008,
            ForgeError::InvalidManifest(_) => -32602, // Invalid params
            ForgeError::TaskFailed(_) => -32009,
            ForgeError::Store(_) => -32010,
            ForgeError::Transport(_) => -32011,
            ForgeError::AiError(_) => -32012,
            ForgeError::Internal(_) => -32603, // Internal error
            ForgeError::Other(_) => -32000,    // Generic
        }
    }
}
