use chrono::{DateTime, Utc};
use rusqlite::params;

use merab_core::artifact::ArtifactLog;
use merab_core::session::{Session, SessionStatus};
use merab_core::ProjectStats;

use crate::db::Database;

impl Database {
    pub fn save_session(&self, session: &Session) -> Result<(), rusqlite::Error> {
        let artifacts_json = serde_json::to_string(&session.artifacts).unwrap_or_default();
        let status_str = match session.status {
            SessionStatus::Completed => "completed",
            SessionStatus::Partial => "partial",
            SessionStatus::Failed => "failed",
        };
        let created_at = session.created_at.to_rfc3339();
        let completed_at = session.completed_at.map(|d| d.to_rfc3339());

        self.conn.execute(
            "INSERT INTO sessions
               (id, project_path, task, summary, artifacts, status, created_at, completed_at,
                tokens_input, tokens_output, cost_usd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               summary       = excluded.summary,
               artifacts     = excluded.artifacts,
               status        = excluded.status,
               completed_at  = excluded.completed_at,
               tokens_input  = excluded.tokens_input,
               tokens_output = excluded.tokens_output,
               cost_usd      = excluded.cost_usd",
            params![
                session.id,
                session.project_path,
                session.task,
                session.summary,
                artifacts_json,
                status_str,
                created_at,
                completed_at,
                session.tokens_input,
                session.tokens_output,
                session.cost_usd,
            ],
        )?;

        // Mantener máximo 50 sesiones por proyecto
        self.conn.execute(
            "DELETE FROM sessions
             WHERE project_path = ?1
               AND id NOT IN (
                   SELECT id FROM sessions
                   WHERE project_path = ?1
                   ORDER BY created_at DESC
                   LIMIT 50
               )",
            params![session.project_path],
        )?;

        Ok(())
    }

    pub fn get_last_session(&self, project_path: &str) -> Result<Option<Session>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_path, task, summary, artifacts, status, created_at, completed_at,
                    tokens_input, tokens_output, cost_usd
             FROM sessions
             WHERE project_path = ?1
             ORDER BY created_at DESC
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![project_path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(parse_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_sessions(
        &self,
        project_path: &str,
        limit: u32,
    ) -> Result<Vec<Session>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_path, task, summary, artifacts, status, created_at, completed_at,
                    tokens_input, tokens_output, cost_usd
             FROM sessions
             WHERE project_path = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![project_path, limit], parse_row)?;
        rows.collect()
    }

    pub fn get_project_stats(&self, project_path: &str) -> Result<ProjectStats, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*), COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0),
                    COALESCE(SUM(cost_usd), 0.0)
             FROM sessions
             WHERE project_path = ?1",
        )?;

        let row = stmt.query_row(params![project_path], |row| {
            Ok(ProjectStats {
                total_sessions: row.get(0)?,
                total_tokens_input: row.get(1)?,
                total_tokens_output: row.get(2)?,
                total_cost_usd: row.get(3)?,
            })
        })?;

        Ok(row)
    }
}

fn parse_row(row: &rusqlite::Row) -> Result<Session, rusqlite::Error> {
    let artifacts_json: String = row.get(4)?;
    let artifacts: ArtifactLog = serde_json::from_str(&artifacts_json).unwrap_or_default();

    let status_str: String = row.get(5)?;
    let status = match status_str.as_str() {
        "completed" => SessionStatus::Completed,
        "partial" => SessionStatus::Partial,
        _ => SessionStatus::Failed,
    };

    let created_at = parse_dt(row.get::<_, String>(6)?);
    let completed_at = row.get::<_, Option<String>>(7)?.map(parse_dt);

    Ok(Session {
        id: row.get(0)?,
        project_path: row.get(1)?,
        task: row.get(2)?,
        summary: row.get(3)?,
        artifacts,
        status,
        created_at,
        completed_at,
        tokens_input: row.get::<_, Option<u64>>(8)?.unwrap_or(0),
        tokens_output: row.get::<_, Option<u64>>(9)?.unwrap_or(0),
        cost_usd: row.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
    })
}

fn parse_dt(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
