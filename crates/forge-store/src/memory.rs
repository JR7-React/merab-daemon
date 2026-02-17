use chrono::Utc;
use rusqlite::params;
use serde_json::Value;

use crate::db::Database;

impl Database {
    pub fn put_memory(
        &self,
        key: &str,
        value: &Value,
        agent_id: Option<&str>,
        ttl_seconds: Option<u64>,
    ) -> Result<(), rusqlite::Error> {
        let value_str = serde_json::to_string(value).unwrap_or_default();
        let expires_at = ttl_seconds.map(|ttl| (Utc::now() + std::time::Duration::from_secs(ttl)).to_rfc3339());
        
        self.conn.execute(
            "INSERT INTO shared_memory (key, value, agent_id, expires_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                agent_id = excluded.agent_id,
                updated_at = datetime('now'),
                expires_at = excluded.expires_at",
            params![key, value_str, agent_id, expires_at],
        )?;
        Ok(())
    }

    pub fn get_memory(&self, key: &str) -> Result<Option<Value>, rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        
        let mut stmt = self.conn.prepare(
            "SELECT value FROM shared_memory 
             WHERE key = ?1 AND (expires_at IS NULL OR expires_at > ?2)",
        )?;
        
        let mut rows = stmt.query(params![key, now])?;
        
        if let Some(row) = rows.next()? {
            let value_str: String = row.get(0)?;
            let value: Value = serde_json::from_str(&value_str).unwrap_or(Value::Null);
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn delete_memory(&self, key: &str) -> Result<bool, rusqlite::Error> {
        let count = self.conn.execute(
            "DELETE FROM shared_memory WHERE key = ?1",
            params![key],
        )?;
        Ok(count > 0)
    }

    pub fn list_memory_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, rusqlite::Error> {
        let now = Utc::now().to_rfc3339();

        if let Some(p) = prefix {
            let like_pattern = format!("{}%", p);
            let mut stmt = self.conn.prepare(
                "SELECT key FROM shared_memory WHERE key LIKE ?1 AND (expires_at IS NULL OR expires_at > ?2)",
            )?;
            let rows = stmt.query_map(params![like_pattern, now], |row| row.get(0))?;
            let mut keys = Vec::new();
            for key in rows {
                keys.push(key?);
            }
            Ok(keys)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT key FROM shared_memory WHERE (expires_at IS NULL OR expires_at > ?1)",
            )?;
            let rows = stmt.query_map(params![now], |row| row.get(0))?;
            let mut keys = Vec::new();
            for key in rows {
                keys.push(key?);
            }
            Ok(keys)
        }
    }

    pub fn cleanup_expired_memory(&self) -> Result<usize, rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "DELETE FROM shared_memory WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            params![now],
        )?;
        Ok(count)
    }
}
