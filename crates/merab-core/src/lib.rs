pub mod agent;
pub mod artifact;
pub mod error;
pub mod events;
pub mod message;
pub mod permission;
pub mod pricing;
pub mod project;
pub mod project_instructions;
pub mod session;
pub mod stats;
pub mod task;
pub mod test_result;

pub use agent::*;
pub use artifact::{ArtifactLog, CommandResult};
pub use error::MerabError;
pub use events::{EventKind, ProgressEvent};
pub use message::*;
pub use permission::*;
pub use pricing::{estimate_cost, TokenUsage};
pub use project::ProjectInfo;
pub use project_instructions::ProjectInstructions;
pub use session::{Session, SessionStatus};
pub use stats::*;
pub use task::*;
pub use test_result::{TestRunResult, TestRunner};

pub mod multi_agent_pipeline;

pub use multi_agent_pipeline::{Persona, Task as PipelineTask, TaskStatus as PipelineTaskStatus};

#[cfg(test)]
mod tests;
