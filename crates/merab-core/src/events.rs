use serde::{Deserialize, Serialize};

/// Tipo de evento de progreso emitido durante la ejecución del pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// El planner está descomponiendo la tarea.
    Planning,
    /// El plan fue generado — incluye número de subtasks.
    PlanReady,
    /// Una subtask comenzó a ejecutarse.
    SubtaskStart,
    /// Una subtask llamó una herramienta (tool call intermedio).
    Step,
    /// Una subtask terminó de ejecutarse.
    SubtaskDone,
    /// El pipeline completo terminó (sentinel para el tail del CLI).
    Done,
    /// Ocurrió un error no fatal durante la ejecución.
    Error,
}

/// Evento de progreso escrito al archivo de eventos en formato NDJSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub kind: EventKind,
    /// Persona que emite el evento (engineer, coder, etc.), si aplica.
    pub persona: Option<String>,
    /// Mensaje legible para el usuario.
    pub message: String,
}

impl ProgressEvent {
    pub fn planning(message: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Planning,
            persona: None,
            message: message.into(),
        }
    }

    pub fn plan_ready(message: impl Into<String>) -> Self {
        Self {
            kind: EventKind::PlanReady,
            persona: None,
            message: message.into(),
        }
    }

    pub fn subtask_start(persona: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: EventKind::SubtaskStart,
            persona: Some(persona.into()),
            message: message.into(),
        }
    }

    pub fn step(persona: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Step,
            persona: Some(persona.into()),
            message: message.into(),
        }
    }

    pub fn subtask_done(persona: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: EventKind::SubtaskDone,
            persona: Some(persona.into()),
            message: message.into(),
        }
    }

    pub fn done() -> Self {
        Self {
            kind: EventKind::Done,
            persona: None,
            message: String::new(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Error,
            persona: None,
            message: message.into(),
        }
    }
}
