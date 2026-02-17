use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub hash: String,
    pub request: String,
    pub response: String,
    pub model: String,
    pub provider: Option<String>,
}

impl Database {
    pub fn get_cache(&self, hash: &str) -> Result<Option<CacheEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT hash, request, response, model, provider FROM llm_cache WHERE hash = ?1",
        )?;

        let mut rows = stmt.query(params![hash])?;

        if let Some(row) = rows.next()? {
            // Update hit count
            let _ = self.conn.execute(
                "UPDATE llm_cache SET hits = hits + 1 WHERE hash = ?1",
                params![hash],
            );

            Ok(Some(CacheEntry {
                hash: row.get(0)?,
                request: row.get(1)?,
                response: row.get(2)?,
                model: row.get(3)?,
                provider: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn store_cache(&self, entry: &CacheEntry) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO llm_cache (hash, request, response, model, provider)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(hash) DO UPDATE SET hits = hits + 1",
            params![
                entry.hash,
                entry.request,
                entry.response,
                entry.model,
                entry.provider,
            ],
        )?;
        Ok(())
    }

    pub fn clear_cache(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute("DELETE FROM llm_cache", [])?;
        Ok(())
    }

    pub fn get_proxy_stats(&self) -> Result<(u64, u64), rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT SUM(hits), COUNT(*) FROM llm_cache")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let hits: u64 = row.get(0).unwrap_or(0);
            let total: u64 = row.get(1).unwrap_or(0);
            Ok((hits, total))
        } else {
            Ok((0, 0))
        }
    }
}
