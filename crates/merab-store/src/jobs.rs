use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub task: String,
    pub status: JobStatus,
    pub log_file: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Done => write!(f, "done"),
            JobStatus::Failed => write!(f, "failed"),
        }
    }
}

impl From<&str> for JobStatus {
    fn from(s: &str) -> Self {
        match s {
            "pending" => JobStatus::Pending,
            "running" => JobStatus::Running,
            "done" => JobStatus::Done,
            "failed" => JobStatus::Failed,
            _ => JobStatus::Pending,
        }
    }
}

impl Database {
    pub fn create_job(&self, id: &str, task: &str, log_file: &str) -> Result<(), rusqlite::Error> {
        let created_at = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO jobs (id, task, status, log_file, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, task, "pending", log_file, created_at],
        )?;

        Ok(())
    }

    pub fn start_job(&self, id: &str) -> Result<(), rusqlite::Error> {
        let started_at = Utc::now().to_rfc3339();

        self.conn.execute(
            "UPDATE jobs SET status = 'running', started_at = ?1 WHERE id = ?2",
            params![started_at, id],
        )?;

        Ok(())
    }

    pub fn complete_job(&self, id: &str, result: &str) -> Result<(), rusqlite::Error> {
        let finished_at = Utc::now().to_rfc3339();

        self.conn.execute(
            "UPDATE jobs SET status = 'done', finished_at = ?1, result = ?2 WHERE id = ?3",
            params![finished_at, result, id],
        )?;

        Ok(())
    }

    pub fn fail_job(&self, id: &str, error: &str) -> Result<(), rusqlite::Error> {
        let finished_at = Utc::now().to_rfc3339();

        self.conn.execute(
            "UPDATE jobs SET status = 'failed', finished_at = ?1, result = ?2 WHERE id = ?3",
            params![finished_at, error, id],
        )?;

        Ok(())
    }

    pub fn get_job(&self, id: &str) -> Result<Option<Job>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task, status, log_file, created_at, started_at, finished_at, result
             FROM jobs WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(parse_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_jobs(
        &self,
        limit: u32,
        include_completed: bool,
    ) -> Result<Vec<Job>, rusqlite::Error> {
        let query = if include_completed {
            "SELECT id, task, status, log_file, created_at, started_at, finished_at, result
             FROM jobs ORDER BY created_at DESC LIMIT ?1"
        } else {
            "SELECT id, task, status, log_file, created_at, started_at, finished_at, result
             FROM jobs WHERE status IN ('pending', 'running') OR created_at > datetime('now', '-1 day')
             ORDER BY created_at DESC LIMIT ?1"
        };

        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map(params![limit], parse_row)?;
        rows.collect()
    }

    pub fn cancel_job(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let rows = self.conn.execute(
            "UPDATE jobs SET status = 'failed', finished_at = ?1 WHERE id = ?2 AND status IN ('pending', 'running')",
            params![Utc::now().to_rfc3339(), id],
        )?;

        Ok(rows > 0)
    }

    pub fn cleanup_old_jobs(&self, days: u32) -> Result<u64, rusqlite::Error> {
        let rows = self.conn.execute(
            "DELETE FROM jobs WHERE created_at < datetime('now', ?1)",
            params![format!("-{} days", days)],
        )?;

        Ok(rows as u64)
    }
}

fn parse_row(row: &rusqlite::Row) -> Result<Job, rusqlite::Error> {
    let status_str: String = row.get(2)?;
    let created_at = parse_dt(row.get::<_, String>(4)?);
    let started_at = row.get::<_, Option<String>>(5)?.map(parse_dt);
    let finished_at = row.get::<_, Option<String>>(6)?.map(parse_dt);

    Ok(Job {
        id: row.get(0)?,
        task: row.get(1)?,
        status: JobStatus::from(status_str.as_str()),
        log_file: row.get(3)?,
        created_at,
        started_at,
        finished_at,
        result: row.get(7)?,
    })
}

fn parse_dt(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
