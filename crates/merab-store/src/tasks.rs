use chrono::Utc;
use merab_core::{Task, TaskStatus};
use rusqlite::params;
use uuid::Uuid;

use crate::db::Database;

impl Database {
    pub fn create_task(&self, task: &Task) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO tasks (id, source, target, input, status, output, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                task.id.to_string(),
                task.source,
                task.target,
                task.input,
                task.status.to_string(),
                task.output,
                task.error,
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> Result<Task, rusqlite::Error> {
        self.conn.query_row(
            "SELECT id, source, target, input, status, output, error, created_at, updated_at
             FROM tasks WHERE id = ?1",
            params![id],
            row_to_task,
        )
    }

    pub fn update_task_status(
        &self,
        id: &str,
        status: TaskStatus,
        output: Option<String>,
        error: Option<String>,
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE tasks
             SET status = ?1, output = ?2, error = ?3, updated_at = ?4
             WHERE id = ?5",
            params![status.to_string(), output, error, now, id],
        )?;
        Ok(())
    }
}

fn row_to_task(row: &rusqlite::Row) -> Result<Task, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let source: Option<String> = row.get(1)?;
    let target: String = row.get(2)?;
    let input: String = row.get(3)?;
    let status_str: String = row.get(4)?;
    let output: Option<String> = row.get(5)?;
    let error: Option<String> = row.get(6)?;
    let created_at_str: String = row.get(7)?;
    let updated_at_str: String = row.get(8)?;

    let id = Uuid::parse_str(&id_str).expect("valid uuid");
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .expect("valid datetime")
        .with_timezone(&Utc);
    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
        .expect("valid datetime")
        .with_timezone(&Utc);

    let status = match status_str.as_str() {
        "pending" => TaskStatus::Pending,
        "running" => TaskStatus::Running,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "cancelled" => TaskStatus::Cancelled,
        _ => TaskStatus::Failed,
    };

    Ok(Task {
        id,
        source,
        target,
        input,
        status,
        output,
        error,
        created_at,
        updated_at,
    })
}
