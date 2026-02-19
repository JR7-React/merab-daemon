use chrono::Utc;
use rusqlite::params;

use crate::db::Database;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConvMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConvSummary {
    pub id: String,
    pub project_path: String,
    pub title: Option<String>,
    pub message_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl Database {
    pub fn create_conversation(&self, id: &str, project_path: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO conversations (id, project_path, created_at, updated_at, title, message_count)
             VALUES (?1, ?2, ?3, ?3, NULL, 0)",
            params![id, project_path, now],
        )?;
        Ok(())
    }

    pub fn add_message(
        &self,
        conv_id: &str,
        role: &str,
        content: &str,
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO conversation_messages (conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![conv_id, role, content, now],
        )?;

        self.conn.execute(
            "UPDATE conversations SET updated_at = ?1, message_count = message_count + 1 WHERE id = ?2",
            params![now, conv_id],
        )?;

        Ok(())
    }

    pub fn get_conversation_messages(
        &self,
        conv_id: &str,
    ) -> Result<Vec<ConvMessage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content, created_at FROM conversation_messages
             WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map(params![conv_id], |row| {
            Ok(ConvMessage {
                role: row.get(0)?,
                content: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;

        rows.collect()
    }

    pub fn list_conversations(
        &self,
        project_path: &str,
        limit: u32,
    ) -> Result<Vec<ConvSummary>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_path, title, message_count, created_at, updated_at
             FROM conversations WHERE project_path = ?1
             ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![project_path, limit], |row| {
            Ok(ConvSummary {
                id: row.get(0)?,
                project_path: row.get(1)?,
                title: row.get(2)?,
                message_count: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        rows.collect()
    }

    pub fn get_last_conversation(
        &self,
        project_path: &str,
    ) -> Result<Option<ConvSummary>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_path, title, message_count, created_at, updated_at
             FROM conversations WHERE project_path = ?1
             ORDER BY updated_at DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![project_path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ConvSummary {
                id: row.get(0)?,
                project_path: row.get(1)?,
                title: row.get(2)?,
                message_count: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_conversation_title(
        &self,
        conv_id: &str,
        title: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE conversations SET title = ?1 WHERE id = ?2",
            params![title, conv_id],
        )?;
        Ok(())
    }
}
