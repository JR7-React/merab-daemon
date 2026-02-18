# Sprint 15 — Memoria de Sesión

## Objetivo

Las sesiones de trabajo persisten en SQLite. Merab recuerda lo que hizo en sesiones anteriores — qué archivos tocó, qué decisiones tomó, qué quedó pendiente. El usuario puede retomar trabajo interrumpido con `merab continue`.

## Problema actual

Cada `merab ask` empieza desde cero. No hay continuidad entre sesiones. Si ayer el Coder implementó la mitad de una feature, hoy Merab no sabe nada de eso.

## Comportamiento objetivo

```bash
# Sesión 1:
$ merab ask "implementa el módulo de autenticación"
# ... trabaja, escribe archivos, corre tests (algunos fallan) ...
# ✓ Sesión guardada (ID: a3f9)

# Sesión 2 (al día siguiente):
$ merab continue
# Retomando sesión a3f9: "implementa el módulo de autenticación"
# Archivos tocados: src/auth.rs, src/middleware.rs
# Estado: tests fallando en test_invalid_token
# Continuando desde donde quedamos...

# Ver historial:
$ merab sessions
# ID     FECHA          TAREA
# a3f9   2026-02-17     implementa el módulo de autenticación (incompleta)
# b7c2   2026-02-16     refactoriza el módulo de usuarios (completada)
```

## Implementación

### 1. Tabla `sessions` en SQLite

Archivo: `crates/merab-store/src/db.rs`

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id           TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    task         TEXT NOT NULL,
    summary      TEXT NOT NULL,
    artifacts    TEXT NOT NULL,    -- ArtifactLog serializado como JSON
    status       TEXT NOT NULL,    -- "completed" | "partial" | "failed"
    created_at   TEXT NOT NULL,
    completed_at TEXT
);
```

Métodos nuevos en `Database`:
```rust
pub fn save_session(&self, session: &Session) -> Result<()>;
pub fn get_last_session(&self, project_path: &str) -> Result<Option<Session>>;
pub fn list_sessions(&self, project_path: &str, limit: u32) -> Result<Vec<Session>>;
```

### 2. Tipo `Session` en `merab-core`

Archivo: `crates/merab-core/src/session.rs`

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id:           String,
    pub project_path: String,
    pub task:         String,
    pub summary:      String,       // resumen generado por el LLM al final
    pub artifacts:    ArtifactLog,
    pub status:       SessionStatus,
    pub created_at:   DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum SessionStatus { Completed, Partial, Failed }
```

### 3. Guardar sesión al final de `handle_ai_orchestrate`

Después de ejecutar el plan, el orquestador:
1. Pide al LLM un resumen de 2-3 líneas de lo que hizo
2. Guarda `Session` en SQLite con los artefactos y el resumen

### 4. Nuevos RPC methods

```
merab.session.save(session: Session) → bool
merab.session.getLast(project_path: String) → Option<Session>
merab.session.list(project_path: String) → Vec<Session>
```

### 5. Nuevos comandos CLI

```
merab sessions          → lista las últimas 10 sesiones del proyecto actual
merab continue          → retoma la última sesión (carga contexto + artefactos)
merab continue <id>     → retoma una sesión específica
```

`merab continue` inyecta el contexto de la sesión anterior en el prompt:
```
"Continuando sesión anterior. Lo que se hizo:
  [resumen de la sesión]
Archivos modificados: [lista]
Tarea pendiente: [tarea original]
Continúa desde donde quedamos."
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `crates/merab-core/src/session.rs` | Nuevo — tipos `Session`, `SessionStatus` |
| `crates/merab-core/src/lib.rs` | Exportar `session` |
| `crates/merab-store/src/db.rs` | Tabla `sessions` + métodos CRUD |
| `crates/merab-daemon/src/rpc/server.rs` | RPC `merab.session.*` |
| `crates/merab-daemon/src/rpc/ai_methods.rs` | Guardar sesión al finalizar orchestrate |
| `crates/merab-cli/src/main.rs` | Comandos `Sessions`, `Continue` |
| `crates/merab-cli/src/client.rs` | Métodos `session_save`, `session_list`, `session_get_last` |

## Verificación

```bash
merab ask "añade logging a todos los handlers"
# ... ejecuta ...
merab sessions
# Debe mostrar la sesión recién creada con status "completed"

merab continue
# Debe cargar el contexto de la sesión anterior y ofrecer continuar
```

## Notas

- Las sesiones se asocian al `project_path` actual (`std::env::current_dir()`).
- Guardar un máximo de 50 sesiones por proyecto (limpiar las más antiguas automáticamente).
- `merab continue` sin sesión previa en el proyecto muestra un mensaje claro, no un error.
