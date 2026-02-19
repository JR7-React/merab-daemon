use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use merab_core::ProgressEvent;

pub struct EventSink {
    file: Option<Arc<Mutex<File>>>,
}

impl EventSink {
    pub fn new() -> Self {
        Self { file: None }
    }

    pub fn from_file(path: &PathBuf) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(Self {
            file: Some(Arc::new(Mutex::new(file))),
        })
    }

    pub fn emit(&self, event: &ProgressEvent) {
        if let Some(ref file) = self.file {
            if let Ok(mut f) = file.lock() {
                let _ = writeln!(f, "{}", serde_json::to_string(event).unwrap_or_default());
                let _ = f.flush();
            }
        }
    }

    pub fn emit_planning(&self) {
        self.emit(&ProgressEvent::planning("Descomponiendo tarea..."));
    }

    pub fn emit_plan_ready(&self, count: usize) {
        self.emit(&ProgressEvent::plan_ready(format!(
            "Plan generado: {} subtasks",
            count
        )));
    }

    pub fn emit_subtask_start(&self, persona: &str, description: &str) {
        self.emit(&ProgressEvent::subtask_start(persona, description));
    }

    pub fn emit_step(&self, persona: &str, message: &str) {
        self.emit(&ProgressEvent::step(persona, message));
    }

    pub fn emit_subtask_done(&self, persona: &str, description: &str) {
        self.emit(&ProgressEvent::subtask_done(persona, description));
    }

    pub fn emit_done(&self) {
        self.emit(&ProgressEvent::done());
    }

    pub fn emit_error(&self, message: &str) {
        self.emit(&ProgressEvent::error(message));
    }

    pub fn emit_retrying(&self, persona: &str, attempt: u32, delay_secs: u64) {
        self.emit(&ProgressEvent::retrying(persona, attempt, delay_secs));
    }

    pub fn is_enabled(&self) -> bool {
        self.file.is_some()
    }
}

impl Clone for EventSink {
    fn clone(&self) -> Self {
        Self {
            file: self.file.clone(),
        }
    }
}

impl Default for EventSink {
    fn default() -> Self {
        Self::new()
    }
}
