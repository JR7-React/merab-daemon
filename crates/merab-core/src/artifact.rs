use serde::{Deserialize, Serialize};

/// Registro de artefactos producidos durante una ejecución del pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtifactLog {
    pub files_created: Vec<String>,
    pub files_modified: Vec<String>,
    pub commands: Vec<CommandResult>,
}

/// Resultado de un comando ejecutado por shell-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub success: bool,
}
