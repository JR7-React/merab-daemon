use std::sync::{Arc, Mutex};

use merab_core::artifact::{ArtifactLog, CommandResult};

/// Rastrea los artefactos producidos por tool calls durante una ejecución del pipeline.
pub struct ArtifactTracker {
    log: Arc<Mutex<ArtifactLog>>,
}

impl ArtifactTracker {
    pub fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(ArtifactLog::default())),
        }
    }

    /// Registra el resultado de un tool call. Solo trackea writes de archivos y comandos de shell.
    pub fn record_tool_result(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        result: &serde_json::Value,
    ) {
        let is_write = tool_name.contains("write") || tool_name.contains("create");
        let is_shell = tool_name.contains("shell")
            || tool_name.contains("execute")
            || tool_name.contains("run");

        if is_write {
            let path = args
                .get("path")
                .or_else(|| args.get("file_path"))
                .or_else(|| args.get("filename"))
                .and_then(|v| v.as_str());

            if let Some(path) = path {
                let path = path.to_string();
                let mut log = self.log.lock().unwrap();
                if log.files_created.contains(&path) || log.files_modified.contains(&path) {
                    // Segunda escritura al mismo path → modificado
                    if !log.files_modified.contains(&path) {
                        log.files_modified.push(path);
                    }
                } else {
                    log.files_created.push(path);
                }
            }
        } else if is_shell {
            let cmd = args
                .get("command")
                .or_else(|| args.get("cmd"))
                .and_then(|v| v.as_str());

            if let Some(cmd) = cmd {
                let stdout = result
                    .get("output")
                    .or_else(|| result.get("stdout"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let exit_code = result
                    .get("exit_code")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;

                let mut log = self.log.lock().unwrap();
                log.commands.push(CommandResult {
                    command: cmd.to_string(),
                    exit_code,
                    stdout,
                    success: exit_code == 0,
                });
            }
        }

        let _ = result; // suprimir warning si no se usa
    }

    /// Retorna el log final y consume el tracker.
    pub fn finish(&self) -> ArtifactLog {
        self.log.lock().unwrap().clone()
    }
}
