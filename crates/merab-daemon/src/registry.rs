use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockWriteGuard};

use merab_core::{AgentId, AgentRecord, AgentStatus, AgentSummary, MerabError};
use merab_store::Database;

pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<AgentId, AgentRecord>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn load_from_db(&self, db: &Database) -> Result<(), MerabError> {
        let summaries = db
            .list_agents()
            .map_err(|e| MerabError::Store(e.to_string()))?;
        // We need full records, so fetch each one
        let mut map = HashMap::new();
        for summary in &summaries {
            if let Some(record) = db
                .get_agent(summary.id)
                .map_err(|e| MerabError::Store(e.to_string()))?
            {
                map.insert(record.id, record);
            }
        }
        tracing::info!("Loaded {} agents from DB: {:?}", map.len(), map.keys());
        // Block on write since this is called at startup before the async runtime is fully used

        // We'll use try_write since we know nobody else holds the lock at init
        if let Ok(mut agents) = self.agents.try_write() {
            *agents = map;
        }
        Ok(())
    }

    pub async fn register(&self, record: AgentRecord) -> Result<AgentId, MerabError> {
        let mut agents = self.agents.write().await;
        let name = record.manifest.name.clone();
        if agents.values().any(|a| a.manifest.name == name) {
            return Err(MerabError::AlreadyExists(name));
        }
        let id = record.id;
        agents.insert(id, record);
        Ok(id)
    }

    pub async fn get(&self, id: AgentId) -> Result<AgentRecord, MerabError> {
        let agents = self.agents.read().await;
        agents
            .get(&id)
            .cloned()
            .ok_or(MerabError::AgentNotFound(id))
    }

    pub async fn list(&self) -> Vec<AgentSummary> {
        let agents = self.agents.read().await;
        agents.values().map(AgentSummary::from).collect()
    }

    pub async fn update_status(
        &self,
        id: AgentId,
        status: AgentStatus,
        pid: Option<u32>,
    ) -> Result<(), MerabError> {
        let mut agents = self.agents.write().await;
        let record = agents.get_mut(&id).ok_or(MerabError::AgentNotFound(id))?;
        if status == AgentStatus::Running {
            record.started_at = Some(chrono::Utc::now());
        }
        record.status = status;
        record.pid = pid;
        Ok(())
    }

    pub async fn set_exit_info(&self, id: AgentId, exit_code: Option<i32>) {
        let mut agents = self.agents.write().await;
        if let Some(record) = agents.get_mut(&id) {
            record.exit_code = exit_code;
            record.stopped_at = Some(chrono::Utc::now());
        }
    }

    /// Synchronous mutable access to agents map. Only use at startup before async tasks run.
    pub fn agents_mut(
        &self,
    ) -> Result<RwLockWriteGuard<'_, HashMap<AgentId, AgentRecord>>, MerabError> {
        self.agents
            .try_write()
            .map_err(|_| MerabError::Other(anyhow::anyhow!("registry lock held")))
    }

    pub async fn remove(&self, id: AgentId) -> Result<AgentRecord, MerabError> {
        let mut agents = self.agents.write().await;
        agents.remove(&id).ok_or(MerabError::AgentNotFound(id))
    }
}
