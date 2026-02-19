use merab_core::ProjectInfo;

use crate::db::Database;

impl Database {
    /// Register or update a project's last activity (upsert).
    pub fn touch_project(&self, path: &str) -> Result<(), rusqlite::Error> {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO projects (path, name, last_active)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET last_active = ?3",
            rusqlite::params![path, name, now],
        )?;
        Ok(())
    }

    /// List known projects ordered by last activity.
    pub fn list_projects(&self, limit: usize) -> Result<Vec<ProjectInfo>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT p.path, p.name, p.last_active,
                    COUNT(s.id) as session_count
             FROM projects p
             LEFT JOIN sessions s ON s.project_path = p.path
             GROUP BY p.path
             ORDER BY p.last_active DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(ProjectInfo {
                path: row.get(0)?,
                name: row.get(1)?,
                last_active: row.get(2)?,
                session_count: row.get::<_, i64>(3)? as u64,
                has_instructions: false, // determined later in CLI
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }
}
