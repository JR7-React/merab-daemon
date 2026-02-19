# Merab — AI Agent Context

Este archivo es el punto de entrada para cualquier IA que trabaje en este proyecto.
**Léelo completo antes de tocar código.**

---

## ¿Qué es Merab?

Runtime local en Rust para orquestar agentes de IA a nivel de sistema operativo.
El usuario corre `merab ask "tarea"` y Merab planifica, ejecuta subtasks en paralelo
con agentes especializados (engineer, coder, reviewer, qa), rastrea artefactos y
guarda la sesión para poder continuar después.

---

## Estructura del workspace (13 crates)

```
crates/
  merab-core/        tipos compartidos (MerabError, Session, ArtifactLog, Task, ...)
  merab-config/      MerabConfig — lee merab.toml + env vars
  merab-store/       SQLite via rusqlite (Database, agents, sessions, memory, tasks)
  merab-ai/          AiClient → proxy HTTP → Anthropic API
  merab-daemon/      binario merabd — servidor JSON-RPC, orquestación, DAG
  merab-cli/         binario merab — CLI, bootstrap, TUI de chat
  merab-transport/   MCP client + A2A client/server
  merab-sandbox/     Windows Job Objects para limitar memoria de agentes
  merab-tui/         monitor TUI (ratatui)
  merab-fs/          agente MCP — filesystem
  merab-shell/       agente MCP — shell commands
  merab-git/         agente MCP — git operations
  merab-echo/        agente MCP — echo (testing)
```

---

## Convenciones CRÍTICAS

```
- Rust edition 2024
- NUNCA usar unwrap() en código de producción — usar ? o map_err
- Logs: tracing::info!/warn!/error! — NUNCA println! en daemon/store
- Errores en librerías: thiserror   (merab-core, merab-store, merab-ai, ...)
- Errores en binarios:  anyhow      (merab-cli, merab-daemon)
- Tests: en el mismo archivo (mod tests) o en tests/ del crate
- Máximo ~500 líneas por archivo nuevo — dividir en módulos si es necesario
- Los archivos existentes que superan ese límite se respetan tal como están
```

---

## Patrones recurrentes — apréndalos antes de escribir código

### Error RPC (merab-daemon)
```rust
// SIEMPRE usar to_rpc_error para convertir MerabError a ErrorObjectOwned
use super::server::to_rpc_error;
return Err(to_rpc_error(MerabError::Store(e.to_string())));
```

### Acceso a base de datos (async)
```rust
// SIEMPRE lockear antes de usar, soltar al terminar el bloque
{
    let db = self.db.lock().await;
    db.some_method()?;
}  // lock liberado aquí
```

### ArtifactTracker — necesita Clone para tokio::spawn
```rust
// ArtifactTracker usa Arc<Mutex<ArtifactLog>> internamente
// #[derive(Clone)] comparte el mismo log entre todos los spawns
let tracker = ArtifactTracker::new();
tokio::spawn(async move { use_tracker(tracker.clone()) });
```

### Nuevo método RPC — 3 pasos
1. Declarar en el trait `MerabApi` en `server.rs` con `#[method(name = "merab.xxx")]`
2. Implementar en `impl MerabApiServer for MerabRpc` en `server.rs`
3. Agregar método cliente en `merab-cli/src/client.rs`

### Nuevo tipo compartido
1. Crear en `merab-core/src/` (con `Serialize, Deserialize, Debug, Clone`)
2. Exportar en `merab-core/src/lib.rs`
3. Usarlo en cualquier crate que dependa de `merab-core`

---

## Tipos clave

```rust
// merab-core
MerabError          // error base del sistema (thiserror)
Session             // sesión de trabajo (id, task, artifacts, status, timestamps)
SessionStatus       // Completed | Partial | Failed
ArtifactLog         // files_created, files_modified, commands
ArtifactTracker     // wrapper Arc<Mutex<ArtifactLog>> para uso async
Task (PipelineTask) // plan del planner (id, description, persona, subtasks, depends_on)
Persona             // Engineer | Coder | Reviewer | Qa

// merab-ai
AiClient            // cliente HTTP al proxy de Anthropic
AiResponse          // content, model, tool_call, artifacts, usage (tokens)
ChatMessage         // role (user/assistant/system) + content

// merab-store
Database            // wrapper de rusqlite::Connection con migrate() automático

// merab-daemon
MerabRpc            // implementación del servidor JSON-RPC
PlannerAgent        // llama al LLM para descomponer tarea en subtasks
ArtifactTracker     // rastreo de archivos/comandos durante el pipeline
```

---

## Flujo principal: merab ask "tarea"

```
CLI (merab ask)
  → bootstrap::ensure_ready()        guarda project.context y project.root_path en memoria
  → client.ai_orchestrate(task)      RPC al daemon

Daemon (handle_ai_orchestrate)
  → PlannerAgent::decompose(task)    LLM genera plan JSON con subtasks y depends_on
  → execute_plan_dag(plan)           ejecuta subtasks en paralelo respetando dependencias
      → por cada ola de subtasks listos:
          tokio::spawn(execute_subtask)
          → build_dynamic_system_prompt()   inyecta project.context + tools disponibles
          → AiClient::chat()                LLM con tools
          → si tool_call → execute_tool_call() → McpManager → agente MCP
  → ArtifactTracker::finish()        consolida archivos/comandos tocados
  → Database::save_session()         persiste sesión en SQLite
  → devuelve AiResponse al CLI

CLI
  → imprime content
  → print_artifact_summary()         muestra archivos tocados
```

---

## Estado actual — Sprints completados

| Sprint | Feature |
|--------|---------|
| 10 | Base: daemon, CLI, agentes MCP, chat TUI |
| 11 | Auto-Pipeline: Plan → DAG execute |
| 12 | Artifact tracking (ArtifactLog, ArtifactTracker) |
| 13 | Project context en shared memory |
| 14 | DAG paralelo (tokio::spawn, depends_on) |
| 15 | Session memory (merab sessions, merab continue) |
| 16 | Per-persona API keys |
| 17 | Streaming output (event_tail, orchestrate_stream) |
| 18 | Token & cost tracking (tokens_input/output, cost_usd) |
| 19 | Test loop (auto-fix con test runner) |
| 20 | Background tasks (merab ask --bg, merab jobs) |
| 21 | Rate limit resilience (retry exponencial) |
| 22 | Config CLI (merab config list/get/set/edit) |
| 23 | Code review (merab review) |
| 24 | HTTP agent (merab-http crate) |
| 25 | Project instructions (merab.md / AGENTS.md) |
| 26 | Watch mode (merab watch) |
| 27 | Doctor (merab doctor) |
| 28 | Chat persistence (conversations en SQLite) |
| 29 | Unit tests (store + core) |
| 30 | Codebase index (merab index build/search) |
| 31 | Diff editing (merab-fs patch) |
| 32 | Self-upgrade (merab self-upgrade) |
| 33 | Multi-project (merab projects list, merab switch) |

---

## Sprints pendientes (ver plans/)

| Sprint | Archivo | Descripción |
|--------|---------|-------------|
| 34 | sprint-34-slash-commands.md | Slash commands en chat |

---

## Cómo iniciar una sesión de trabajo

```bash
# 1. Verificar que compila
cargo build

# 2. Verificar que los tests pasan
cargo test
# Debe mostrar: 56+ tests, 0 failed

# 3. Leer el plan del sprint a implementar
cat plans/sprint-XX-nombre.md

# 4. Leer los archivos mencionados en "Archivos" del plan ANTES de modificarlos

# 5. Después de implementar, siempre:
cargo build    # debe pasar limpio
cargo test     # 0 failures
git add <archivos específicos>
git commit -m "feat(sprint-XX): descripción"
```

---

## Errores frecuentes y cómo evitarlos

| Error | Causa | Fix |
|-------|-------|-----|
| `missing field 'artifacts'` en tests | `AiResponse` tiene `artifacts: Option<ArtifactLog>` | Agregar `artifacts: None` al struct literal |
| `the trait Send is not implemented` | Cerrar el lock antes del await | Usar bloque `{ let db = self.db.lock().await; ... }` |
| `unused import` warning en `ai_methods.rs` | Al mover tipos a `dag.rs` quedan imports sobrantes | Revisar imports después de refactors |
| `cannot borrow as mutable` en DAG | `execute_plan_dag` toma `&Arc<MerabConfig>` pero se pasa `&MerabConfig` | `let arc = Arc::new(config.clone())` |

---

## Comandos CLI disponibles

```bash
# Uso principal
merab                          # inicia chat interactivo
merab ask "tarea"              # ejecuta tarea con pipeline Plan→DAG
merab ask "tarea" --bg         # ejecuta en background
merab plan "tarea"             # genera plan sin ejecutar
merab sessions                 # lista sesiones del proyecto actual
merab continue [id]            # retoma la última sesión (o por ID)

# Diagnóstico y contexto
merab context                  # muestra contexto detectado del proyecto
merab doctor                   # verifica salud del sistema
merab stats                    # estadísticas de uso
merab monitor                  # TUI de monitoreo en tiempo real

# Agentes y herramientas
merab list                     # lista agentes registrados
merab register <manifest>      # registra un agente MCP
merab start/stop/status <id>   # gestión de agentes

# Índice de código
merab index build              # indexa el proyecto actual
merab index search "query"     # busca símbolos por nombre
merab index search -k struct "Config"  # filtra por tipo (fn, struct, trait, etc.)

# Jobs en background
merab jobs list                # lista jobs activos
merab jobs status <id>         # estado de un job
merab jobs cancel <id>         # cancela un job

# Otros
merab memory get/put/list/delete  # gestión de shared memory
merab config ...               # gestión de configuración
merab watch -t "tarea" -p "**/*.rs"  # re-ejecuta tarea al detectar cambios
merab review                   # review de código con IA
merab self-upgrade "mejora"    # se mejora a sí mismo con IA

# Multi-proyecto
merab projects list            # lista proyectos conocidos
merab switch <path>            # cambia proyecto activo (override CWD)
```

---

## Variables de entorno

```bash
ANTHROPIC_API_KEY=sk-ant-...    # requerida para merab ask/chat
MERAB_CONFIG=path/to/config     # opcional, default: ~/.merab/config.toml
```
