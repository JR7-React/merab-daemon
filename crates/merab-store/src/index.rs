use crate::db::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedSymbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub signature: String,
}

impl Database {
    pub fn index_clear_project(&self, project_path: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM code_index WHERE project_path = ?1",
            params![project_path],
        )?;
        Ok(())
    }

    pub fn index_insert_symbol(
        &self,
        project_path: &str,
        symbol: &IndexedSymbol,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO code_index (project_path, name, kind, file, line, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                project_path,
                symbol.name,
                symbol.kind,
                symbol.file,
                symbol.line as i64,
                symbol.signature
            ],
        )?;
        Ok(())
    }

    pub fn index_search(
        &self,
        project_path: &str,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<IndexedSymbol>, rusqlite::Error> {
        let pattern = format!("%{}%", query.to_lowercase());

        let results = match kind {
            Some(k) => {
                let mut stmt = self.conn.prepare(
                    "SELECT name, kind, file, line, signature FROM code_index
                     WHERE project_path = ?1 AND LOWER(name) LIKE ?2 AND kind = ?3
                     ORDER BY name LIMIT ?4",
                )?;
                let rows = stmt.query_map(
                    params![project_path, pattern, k, limit as i64],
                    row_to_symbol,
                )?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = self.conn.prepare(
                    "SELECT name, kind, file, line, signature FROM code_index
                     WHERE project_path = ?1 AND LOWER(name) LIKE ?2
                     ORDER BY name LIMIT ?3",
                )?;
                let rows =
                    stmt.query_map(params![project_path, pattern, limit as i64], row_to_symbol)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(results)
    }
}

fn row_to_symbol(row: &rusqlite::Row) -> rusqlite::Result<IndexedSymbol> {
    let line_i64: i64 = row.get(3)?;
    Ok(IndexedSymbol {
        name: row.get(0)?,
        kind: row.get(1)?,
        file: row.get(2)?,
        line: line_i64 as usize,
        signature: row.get(4)?,
    })
}
