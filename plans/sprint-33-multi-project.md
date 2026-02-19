# Sprint 33 — Multi-Project (un daemon, múltiples proyectos)

## Objetivo

Un solo daemon `merabd` gestiona múltiples proyectos simultáneamente. El CLI detecta el proyecto actual por CWD o permite cambiar con `merab switch`. Sessions, memoria, historial de chat, e índice de código están completamente aislados por proyecto.

## Problema actual

Hoy `merabd` es un proceso global sin namespacing real de proyectos. `project.root_path` se guarda en shared memory pero:
- Solo hay una instancia activa de `project.root_path` a la vez
- Si el usuario trabaja en dos proyectos en paralelo (dos terminales), se pisan
- `merab sessions` muestra todas las sessions mezcladas sin separación clara
- `merab chat` no tiene forma de saber en qué proyecto está el usuario desde terminal B si terminal A ya cambió el contexto

```bash
# Terminal A — proyecto-web
cd ~/dev/web-app && merab ask "arregla el bug de login"
# → guarda project.root_path = ~/dev/web-app

# Terminal B — proyecto-backend (al mismo tiempo)
cd ~/dev/backend && merab ask "optimiza la query SQL"
# → sobreescribe project.root_path = ~/dev/backend
# → Terminal A ahora también apunta al backend ← BUG
```

## Comportamiento objetivo

```bash
# El CLI pasa el CWD automáticamente en cada llamada — no hay estado global
cd ~/dev/web-app && merab ask "arregla el bug de login"
# → El daemon recibe project_path=~/dev/web-app con cada RPC
# → Sessions, memoria y chat son del proyecto web-app

cd ~/dev/backend && merab sessions
# → Solo muestra sessions de ~/dev/backend

# Ver todos los proyectos conocidos por el daemon
$ merab projects list
  ~/dev/web-app      12 sessions   última actividad: hace 2h
  ~/dev/backend       8 sessions   última actividad: hace 10m
* ~/dev/merab        45 sessions   última actividad: hace 1m
  (el * indica proyecto actual del CWD)

# Cambiar proyecto explícitamente (para cuando CWD no es suficiente)
$ merab switch ~/dev/otro-proyecto
Proyecto activo: ~/dev/otro-proyecto
```

## Implementación

### Principio central: project_path en cada RPC call

Cada método RPC que dependa del proyecto recibe `project_path: String` como parámetro. El CLI siempre pasa `std::env::current_dir()` al iniciar. No hay estado global de proyecto en el daemon.

### Cambios en el cliente CLI: `crates/merab-cli/src/client.rs`

```rust
impl MerabClient {
    /// Obtiene el project_path actual (CWD normalizado)
    pub fn current_project_path() -> String {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    }
}
```

Todos los métodos que hoy usan `project_path` hardcodeado o lo leen de memoria compartida deben recibir `project_path` como parámetro desde el CLI.

### Nuevo tipo en `merab-core`: `ProjectInfo`

```rust
// merab-core/src/project.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,           // último componente del path
    pub session_count: u64,
    pub last_active: String,    // ISO 8601
    pub has_instructions: bool, // si existe merab.md o AGENTS.md
}
```

### Store: `crates/merab-store/src/projects.rs` (nuevo)

```rust
impl Database {
    /// Registra actividad de un proyecto (upsert)
    pub fn touch_project(&self, path: &str) -> Result<(), StoreError> {
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

    pub fn list_projects(&self, limit: usize) -> Result<Vec<ProjectInfo>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT p.path, p.name, p.last_active,
                    COUNT(s.id) as session_count
             FROM projects p
             LEFT JOIN sessions s ON s.project_path = p.path
             GROUP BY p.path
             ORDER BY p.last_active DESC
             LIMIT ?1"
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(ProjectInfo {
                path: row.get(0)?,
                name: row.get(1)?,
                last_active: row.get(2)?,
                session_count: row.get::<_, i64>(3)? as u64,
                has_instructions: false, // se determina en CLI
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::from)
    }
}
```

### Migración SQLite

```sql
CREATE TABLE IF NOT EXISTS projects (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    last_active TEXT NOT NULL
);
```

### RPC: nuevos métodos

```rust
// En server.rs trait:
#[method(name = "merab.project.list")]
async fn project_list(&self, limit: Option<u32>) -> RpcResult<Vec<ProjectInfo>>;

#[method(name = "merab.project.touch")]
async fn project_touch(&self, path: String) -> RpcResult<()>;
```

Implementación en `crates/merab-daemon/src/rpc/project_impls.rs` (nuevo):

```rust
pub async fn project_list(rpc: &MerabRpc, limit: Option<u32>) -> RpcResult<Vec<ProjectInfo>> {
    let db = rpc.db.lock().await;
    db.list_projects(limit.unwrap_or(20) as usize)
        .map_err(to_rpc_error)
}

pub async fn project_touch(rpc: &MerabRpc, path: String) -> RpcResult<()> {
    let db = rpc.db.lock().await;
    db.touch_project(&path).map_err(to_rpc_error)
}
```

### CLI: `merab projects` y `merab switch`

En `main.rs`:
```rust
/// List and manage projects
Projects,
/// Switch active project context
Switch {
    path: String,
},
```

En `project_cmd.rs` (nuevo):
```rust
pub async fn run_list(client: &MerabClient) -> anyhow::Result<()> {
    let current = MerabClient::current_project_path();
    let projects = client.project_list(Some(20)).await?;

    if projects.is_empty() {
        println!("No hay proyectos registrados. Usa 'merab ask' para comenzar.");
        return Ok(());
    }

    println!("{:<3} {:<40} {:>8}  {}", "", "Proyecto", "Sessions", "Última actividad");
    println!("{}", "─".repeat(70));

    for p in &projects {
        let marker = if p.path == current { "*" } else { " " };
        let name = if p.path.len() > 38 {
            format!("...{}", &p.path[p.path.len()-35..])
        } else {
            p.path.clone()
        };
        println!("{:<3} {:<40} {:>8}  {}",
            marker, name, p.session_count, p.last_active);
    }
    Ok(())
}

pub fn run_switch(path: &str) -> anyhow::Result<()> {
    let resolved = std::path::Path::new(path).canonicalize()
        .map_err(|_| anyhow::anyhow!("Directorio no encontrado: {}", path))?;
    // Escribir en ~/.merab/active-project para que el CLI lo lea en la próxima llamada
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("No se encontró config dir"))?
        .join("merab");
    std::fs::create_dir_all(&config_dir)?;
    std::fs::write(config_dir.join("active-project"), resolved.to_string_lossy().as_bytes())?;
    println!("Proyecto activo: {}", resolved.display());
    Ok(())
}
```

### Actualizar `bootstrap.rs`

Al iniciar, registrar el proyecto actual:

```rust
// Al final de ensure_ready():
let project_path = std::env::current_dir()
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_default();
if !project_path.is_empty() {
    let _ = client.project_touch(project_path).await;
}
```

### Aislar shared memory por proyecto

En los métodos `memory_put` y `memory_get` del daemon, las keys de memoria deben tener el prefijo del `project_path` para evitar colisiones:

```rust
// memory_put: prefixar la key con hash del project_path
let scoped_key = format!("{}::{}", project_hash(&project_path), key);
```

Esto es transparente al usuario — el CLI siempre pasa el project_path y el daemon hace el scoping.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-core/src/project.rs` | Nuevo — `ProjectInfo` tipo |
| `merab-core/src/lib.rs` | Exportar `ProjectInfo` |
| `merab-store/src/projects.rs` | Nuevo — `touch_project`, `list_projects` |
| `merab-store/src/db.rs` | Migración tabla `projects` |
| `merab-store/src/lib.rs` | Exportar módulo `projects` |
| `merab-daemon/src/rpc/project_impls.rs` | Nuevo — `project_list`, `project_touch` |
| `merab-daemon/src/rpc/server.rs` | Declarar métodos RPC nuevos |
| `merab-daemon/src/rpc/mod.rs` | `pub mod project_impls` |
| `merab-cli/src/project_cmd.rs` | Nuevo — `run_list`, `run_switch` |
| `merab-cli/src/main.rs` | Agregar `Projects`, `Switch` variants + `mod project_cmd` |
| `merab-cli/src/client.rs` | Agregar `current_project_path()`, `project_list()`, `project_touch()` |
| `merab-cli/src/bootstrap.rs` | Llamar `project_touch` al iniciar |

## Verificación

```bash
# Trabajar en dos proyectos en paralelo
cd ~/dev/proyecto-a && merab ask "describe este proyecto"
cd ~/dev/proyecto-b && merab ask "describe este proyecto"

# Listar proyectos
merab projects list
# → Debe mostrar ambos proyectos separados con sus session counts

# Sessions aisladas
cd ~/dev/proyecto-a && merab sessions
# → Solo sessions de proyecto-a

cd ~/dev/proyecto-b && merab sessions
# → Solo sessions de proyecto-b

# Switch explícito
merab switch ~/dev/proyecto-a
merab sessions  # desde cualquier directorio
# → Sessions de proyecto-a
```

## Notas para el modelo de IA

- **El cambio más importante** es asegurarse de que `project_path` fluye desde el CLI hasta el daemon en **cada** RPC call relevante, no solo en `ai_orchestrate`. Revisar `memory_put`, `memory_get`, `session_list`, `conv_list`, `index_search`.
- **Backwards compatibility**: los proyectos registrados antes de este sprint no tienen entrada en la tabla `projects`. `touch_project` resuelve esto con upsert — se registran la primera vez que se usan después del upgrade.
- **`merab switch`** no cambia el CWD del shell — solo escribe un archivo de preferencia. El CWD sigue siendo la fuente de verdad primaria; `active-project` es un override explícito.
- **Scoping de shared memory**: usar `blake3` o simplemente `format!("{:016x}", hash)` con la crate `std::collections::hash_map::DefaultHasher` para el prefix. No hace falta criptografía.
- **Dificultad**: Media (~250 líneas). La mayor parte es wiring, no lógica compleja.
