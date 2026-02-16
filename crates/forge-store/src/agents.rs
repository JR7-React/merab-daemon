use chrono::Utc;
use forge_core::{AgentId, AgentManifest, AgentRecord, AgentStatus, AgentSummary};
use rusqlite::params;
use uuid::Uuid;

use crate::db::Database;

impl Database {
    pub fn insert_agent(&self, record: &AgentRecord) -> Result<(), rusqlite::Error> {
        let manifest_json =
            serde_json::to_string(&record.manifest).expect("manifest serialization");
        let status = status_to_str(record.status);
        self.conn.execute(
            "INSERT INTO agents (id, name, manifest, status, pid, registered_at, started_at, stopped_at, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id.to_string(),
                record.manifest.name,
                manifest_json,
                status,
                record.pid,
                record.registered_at.to_rfc3339(),
                record.started_at.map(|t| t.to_rfc3339()),
                record.stopped_at.map(|t| t.to_rfc3339()),
                record.exit_code,
            ],
        )?;
        Ok(())
    }

    pub fn get_agent(&self, id: AgentId) -> Result<Option<AgentRecord>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, manifest, status, pid, registered_at, started_at, stopped_at, exit_code FROM agents WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.to_string()], row_to_record)?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_agent_by_name(&self, name: &str) -> Result<Option<AgentRecord>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, manifest, status, pid, registered_at, started_at, stopped_at, exit_code FROM agents WHERE name = ?1",
        )?;
        let mut rows = stmt.query_map(params![name], row_to_record)?;
        Ok(rows.next().transpose()?)
    }

    pub fn list_agents(&self) -> Result<Vec<AgentSummary>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, manifest, status, pid, registered_at, started_at, stopped_at, exit_code FROM agents ORDER BY registered_at",
        )?;
        let rows = stmt.query_map([], row_to_record)?;
        let mut summaries = Vec::new();
        for row in rows {
            let record = row?;
            summaries.push(AgentSummary::from(&record));
        }
        Ok(summaries)
    }

    pub fn update_agent_status(
        &self,
        id: AgentId,
        status: AgentStatus,
        pid: Option<u32>,
    ) -> Result<bool, rusqlite::Error> {
        let started_at = if status == AgentStatus::Running {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };
        let stopped_at = match status {
            AgentStatus::Stopped | AgentStatus::Failed => Some(Utc::now().to_rfc3339()),
            _ => None,
        };
        let rows = self.conn.execute(
            "UPDATE agents SET status = ?1, pid = ?2, started_at = COALESCE(?3, started_at), stopped_at = COALESCE(?4, stopped_at) WHERE id = ?5",
            params![status_to_str(status), pid, started_at, stopped_at, id.to_string()],
        )?;
        Ok(rows > 0)
    }

    pub fn update_agent_exit(
        &self,
        id: AgentId,
        status: AgentStatus,
        exit_code: Option<i32>,
    ) -> Result<bool, rusqlite::Error> {
        let stopped_at = Utc::now().to_rfc3339();
        let rows = self.conn.execute(
            "UPDATE agents SET status = ?1, pid = NULL, stopped_at = ?2, exit_code = ?3 WHERE id = ?4",
            params![status_to_str(status), stopped_at, exit_code, id.to_string()],
        )?;
        Ok(rows > 0)
    }

    pub fn list_running_agents(&self) -> Result<Vec<AgentRecord>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, manifest, status, pid, registered_at, started_at, stopped_at, exit_code FROM agents WHERE status = 'running'",
        )?;
        let rows = stmt.query_map([], row_to_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn delete_agent(&self, id: AgentId) -> Result<bool, rusqlite::Error> {
        let rows = self
            .conn
            .execute("DELETE FROM agents WHERE id = ?1", params![id.to_string()])?;
        Ok(rows > 0)
    }
}

fn status_to_str(s: AgentStatus) -> &'static str {
    match s {
        AgentStatus::Registered => "registered",
        AgentStatus::Running => "running",
        AgentStatus::Stopped => "stopped",
        AgentStatus::Failed => "failed",
    }
}

fn str_to_status(s: &str) -> AgentStatus {
    match s {
        "running" => AgentStatus::Running,
        "stopped" => AgentStatus::Stopped,
        "failed" => AgentStatus::Failed,
        _ => AgentStatus::Registered,
    }
}

fn row_to_record(row: &rusqlite::Row) -> Result<AgentRecord, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let manifest_json: String = row.get(1)?;
    let status_str: String = row.get(2)?;
    let pid: Option<u32> = row.get(3)?;
    let registered_at_str: String = row.get(4)?;
    let started_at_str: Option<String> = row.get(5)?;
    let stopped_at_str: Option<String> = row.get(6)?;
    let exit_code: Option<i32> = row.get(7)?;

    let id = Uuid::parse_str(&id_str).expect("valid uuid in db");
    let manifest: AgentManifest =
        serde_json::from_str(&manifest_json).expect("valid manifest json in db");
    let status = str_to_status(&status_str);
    let registered_at = chrono::DateTime::parse_from_rfc3339(&registered_at_str)
        .expect("valid datetime")
        .with_timezone(&chrono::Utc);
    let started_at = started_at_str.map(|s| {
        chrono::DateTime::parse_from_rfc3339(&s)
            .expect("valid datetime")
            .with_timezone(&chrono::Utc)
    });
    let stopped_at = stopped_at_str.map(|s| {
        chrono::DateTime::parse_from_rfc3339(&s)
            .expect("valid datetime")
            .with_timezone(&chrono::Utc)
    });

    Ok(AgentRecord {
        id,
        manifest,
        status,
        pid,
        registered_at,
        started_at,
        stopped_at,
        exit_code,
    })
}
