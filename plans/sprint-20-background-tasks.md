# Sprint 20 — Background Tasks (merab jobs)

## Objetivo

Ejecutar tareas largas en segundo plano sin bloquear el terminal. El usuario puede lanzar una tarea, cerrar la terminal, y revisar el resultado después.

## Problema actual

`merab ask "tarea larga"` bloquea el terminal durante minutos. Si se cierra la terminal, la tarea muere. No hay forma de ver qué tareas están corriendo.

## Comportamiento objetivo

```bash
# Lanzar en segundo plano
$ merab ask --bg "refactoriza todos los handlers para usar async/await"
# Job lanzado: job-7f3a (merab jobs para ver estado)

# Ver jobs activos
$ merab jobs
# ID       ESTADO     INICIO          TAREA
# job-7f3a running    10:32 (2m ago)  refactoriza todos los handlers...
# job-3b1c done       09:15           agrega logging a auth module

# Ver output de un job
$ merab jobs log job-7f3a
# [Planner] Descomponiendo tarea...
# [Pipeline] 6 subtasks detectadas...
# [coder] Refactorizando user_handler.rs...
# ...

# Esperar a que termine
$ merab jobs wait job-7f3a
# Esperando job-7f3a... ✓ Completado (3m 42s)
# [resultado aquí]
```

## Implementación

### Tabla `jobs` en SQLite (`merab-store/src/db.rs`)
```sql
CREATE TABLE IF NOT EXISTS jobs (
    id          TEXT PRIMARY KEY,
    task        TEXT NOT NULL,
    status      TEXT NOT NULL,   -- pending | running | done | failed
    log_file    TEXT NOT NULL,   -- path al archivo de log
    created_at  TEXT NOT NULL,
    started_at  TEXT,
    finished_at TEXT,
    result      TEXT             -- AiResponse serializado al terminar
);
```

### `crates/merab-daemon/src/jobs.rs` (nuevo)
```rust
pub struct JobManager {
    db: Arc<Mutex<Database>>,
}
impl JobManager {
    pub async fn submit(&self, task: String) -> String  // retorna job_id
    pub async fn get_status(&self, job_id: &str) -> JobStatus
    pub async fn get_log(&self, job_id: &str) -> String
    pub async fn list_active(&self) -> Vec<JobSummary>
}
```

El daemon tiene un `JobManager` global. Al recibir `merab.job.submit`, lanza un `tokio::spawn` que ejecuta `handle_ai_orchestrate` y escribe logs al archivo.

### Nuevos RPC methods (`server.rs`)
```
merab.job.submit(task: String) → String (job_id)
merab.job.status(job_id: String) → JobStatus
merab.job.log(job_id: String) → String
merab.job.list() → Vec<JobSummary>
merab.job.cancel(job_id: String) → bool
```

### `crates/merab-cli/src/main.rs`
```
Commands::Ask {
    question: String,
    #[arg(long)]
    bg: bool,    // --bg lanza en background
}

Commands::Jobs(JobsCommands)  // subcommand
```

```
JobsCommands:
  List              → merab jobs
  Log { id }        → merab jobs log <id>
  Wait { id }       → merab jobs wait <id>
  Cancel { id }     → merab jobs cancel <id>
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-store/src/db.rs` | Tabla `jobs` |
| `merab-store/src/jobs.rs` | Nuevo — CRUD de jobs |
| `merab-daemon/src/jobs.rs` | Nuevo — `JobManager` |
| `merab-daemon/src/rpc/server.rs` | RPC `merab.job.*` |
| `merab-cli/src/client.rs` | Métodos `job_submit`, `job_status`, etc. |
| `merab-cli/src/main.rs` | Flag `--bg` y subcommand `jobs` |

## Verificación

```bash
merab ask --bg "crea 10 archivos de prueba"
merab jobs
# Debe mostrar el job en estado "running"
merab jobs wait <id>
# Debe esperar y mostrar resultado cuando termine
```

## Notas

- Los logs se guardan en `~/.merab/logs/job-<id>.log`
- Limpiar automáticamente jobs con más de 7 días de antigüedad
- `merab jobs` sin args lista solo jobs de las últimas 24h
- El daemon debe sobrevivir al cierre del CLI que lo lanzó (ya es el caso — es un proceso separado)
