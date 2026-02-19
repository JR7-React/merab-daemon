use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub struct Database {
    pub conn: Connection,
    pub path: PathBuf,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn,
            path: path.to_path_buf(),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn,
            path: PathBuf::from(":memory:"),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agents (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL UNIQUE,
                manifest    TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'registered',
                pid         INTEGER,
                registered_at TEXT NOT NULL,
                started_at  TEXT,
                stopped_at  TEXT,
                exit_code   INTEGER
            );

            CREATE TABLE IF NOT EXISTS messages (
                id              TEXT PRIMARY KEY,
                from_agent_id   TEXT NOT NULL,
                to_agent_id     TEXT,
                content         TEXT NOT NULL,
                status          TEXT NOT NULL DEFAULT 'pending',
                created_at      TEXT NOT NULL,
                delivered_at    TEXT,
                FOREIGN KEY (from_agent_id) REFERENCES agents(id)
            );

            CREATE TABLE IF NOT EXISTS memory (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id    TEXT NOT NULL,
                key         TEXT NOT NULL,
                value       TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (agent_id) REFERENCES agents(id),
                UNIQUE(agent_id, key)
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id          TEXT PRIMARY KEY,
                source      TEXT,
                target      TEXT NOT NULL,
                input       TEXT NOT NULL,
                status      TEXT NOT NULL,
                output      TEXT,
                error       TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS llm_cache (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                hash        TEXT NOT NULL UNIQUE,
                request     TEXT NOT NULL,
                response    TEXT NOT NULL,
                model       TEXT NOT NULL,
                provider    TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                hits        INTEGER DEFAULT 1
            );
            
            CREATE INDEX IF NOT EXISTS idx_cache_hash ON llm_cache(hash);

            CREATE TABLE IF NOT EXISTS shared_memory (
                key         TEXT PRIMARY KEY,
                value       TEXT NOT NULL,
                agent_id    TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at  TEXT
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id           TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                task         TEXT NOT NULL,
                summary      TEXT NOT NULL,
                artifacts    TEXT NOT NULL,
                status       TEXT NOT NULL,
                created_at   TEXT NOT NULL,
                completed_at TEXT,
                tokens_input  INTEGER DEFAULT 0,
                tokens_output INTEGER DEFAULT 0,
                cost_usd     REAL DEFAULT 0.0
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_project
                ON sessions(project_path, created_at DESC);

            CREATE TABLE IF NOT EXISTS jobs (
                id          TEXT PRIMARY KEY,
                task        TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'pending',
                log_file    TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                started_at  TEXT,
                finished_at TEXT,
                result      TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
            CREATE INDEX IF NOT EXISTS idx_jobs_created ON jobs(created_at);

            CREATE TABLE IF NOT EXISTS conversations (
                id          TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL,
                title       TEXT,
                message_count INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS conversation_messages (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL,
                role            TEXT NOT NULL,
                content         TEXT NOT NULL,
                created_at      TEXT NOT NULL,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );

            CREATE INDEX IF NOT EXISTS idx_conv_project
                ON conversations(project_path, updated_at DESC);
            ",
        )?;

        // Migrate existing databases: add new columns if missing
        self.add_column_if_missing("agents", "stopped_at", "TEXT")?;
        self.add_column_if_missing("agents", "exit_code", "INTEGER")?;
        self.add_column_if_missing("sessions", "tokens_input", "INTEGER DEFAULT 0")?;
        self.add_column_if_missing("sessions", "tokens_output", "INTEGER DEFAULT 0")?;
        self.add_column_if_missing("sessions", "cost_usd", "REAL DEFAULT 0.0")?;

        Ok(())
    }

    fn add_column_if_missing(
        &self,
        table: &str,
        column: &str,
        col_type: &str,
    ) -> Result<(), rusqlite::Error> {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}");
        match self.conn.execute_batch(&sql) {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("duplicate column name") => Ok(()),
            Err(e) => Err(e),
        }
    }
}
