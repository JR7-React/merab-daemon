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

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
