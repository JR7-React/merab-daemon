use chrono::Utc;
use merab_core::{AgentId, Message, MessageId, MessageStatus};
use rusqlite::params;

use crate::db::Database;

impl Database {
    pub fn insert_message(&self, msg: &Message) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO messages (id, from_agent_id, to_agent_id, content, status, created_at, delivered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                msg.id.to_string(),
                msg.from_agent.to_string(),
                msg.to_agent.map(|id| id.to_string()),
                msg.content,
                status_to_str(msg.status),
                msg.created_at.to_rfc3339(),
                msg.delivered_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// Get pending messages for an agent: direct messages + broadcasts (excluding self-broadcasts).
    pub fn get_messages_for(&self, agent_id: AgentId) -> Result<Vec<Message>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_agent_id, to_agent_id, content, status, created_at, delivered_at
             FROM messages
             WHERE status = 'pending'
               AND (to_agent_id = ?1 OR (to_agent_id IS NULL AND from_agent_id != ?1))
             ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![agent_id.to_string()], row_to_message)?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn acknowledge_message(&self, id: MessageId) -> Result<bool, rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let rows = self.conn.execute(
            "UPDATE messages SET status = 'acknowledged', delivered_at = ?1 WHERE id = ?2",
            params![now, id.to_string()],
        )?;
        Ok(rows > 0)
    }

    pub fn delete_agent_messages(&self, agent_id: AgentId) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM messages WHERE from_agent_id = ?1 OR to_agent_id = ?1",
            params![agent_id.to_string()],
        )?;
        Ok(())
    }
}

fn status_to_str(s: MessageStatus) -> &'static str {
    match s {
        MessageStatus::Pending => "pending",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Acknowledged => "acknowledged",
    }
}

fn str_to_status(s: &str) -> MessageStatus {
    match s {
        "delivered" => MessageStatus::Delivered,
        "acknowledged" => MessageStatus::Acknowledged,
        _ => MessageStatus::Pending,
    }
}

fn row_to_message(row: &rusqlite::Row) -> Result<Message, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let from_str: String = row.get(1)?;
    let to_str: Option<String> = row.get(2)?;
    let content: String = row.get(3)?;
    let status_str: String = row.get(4)?;
    let created_at_str: String = row.get(5)?;
    let delivered_at_str: Option<String> = row.get(6)?;

    let id = uuid::Uuid::parse_str(&id_str).expect("valid uuid in db");
    let from_agent = uuid::Uuid::parse_str(&from_str).expect("valid uuid in db");
    let to_agent = to_str.map(|s| uuid::Uuid::parse_str(&s).expect("valid uuid in db"));
    let status = str_to_status(&status_str);
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .expect("valid datetime")
        .with_timezone(&chrono::Utc);
    let delivered_at = delivered_at_str.map(|s| {
        chrono::DateTime::parse_from_rfc3339(&s)
            .expect("valid datetime")
            .with_timezone(&chrono::Utc)
    });

    Ok(Message {
        id,
        from_agent,
        to_agent,
        content,
        status,
        created_at,
        delivered_at,
    })
}
