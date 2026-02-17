pub mod agent;
pub mod error;
pub mod message;
pub mod permission;
pub mod stats;
pub mod task;

pub use agent::*;
pub use error::ForgeError;
pub use message::*;
pub use permission::*;
pub use stats::*;
pub use task::*;

pub mod multi_agent_pipeline;

pub use multi_agent_pipeline::{Persona, Task as PipelineTask, TaskStatus as PipelineTaskStatus};

#[cfg(test)]
mod tests;
