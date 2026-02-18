# Sprint 14 — Ejecución Paralela

## Objetivo

Ejecutar subtasks independientes en paralelo. Hoy el pipeline es estrictamente secuencial. Con un DAG de dependencias, las subtasks sin dependencias entre sí corren al mismo tiempo, reduciendo el tiempo total significativamente.

## Problema actual

```
Plan: [Architect, Research, Coder, Reviewer, QA]

HOY (secuencial):
Architect(15s) → Research(10s) → Coder(20s) → Reviewer(10s) → QA(15s)
Total: 70 segundos

OBJETIVO (paralelo donde sea posible):
Architect(15s) ─┐
                 ├→ Coder(20s) → Reviewer(10s) ─┐
Research(10s)  ─┘                                └→ QA(15s)
Total: ~45 segundos
```

## Implementación

### 1. Agregar `depends_on` a la struct `Task`

Archivo: `crates/merab-core/src/multi_agent_pipeline.rs`

```rust
pub struct Task {
    pub id:             String,
    pub description:    String,
    pub status:         TaskStatus,
    pub persona:        Persona,
    pub subtasks:       Vec<Task>,
    pub assigned_agent: Option<AgentId>,
    pub depends_on:     Vec<String>,    // ← nuevo: IDs de tasks que deben terminar antes
}
```

### 2. Actualizar el prompt del Planner

Archivo: `crates/merab-daemon/src/planner.rs`

Agregar al `PLANNER_SYSTEM_PROMPT` instrucciones para declarar dependencias:

```
## Dependencias
Declara en `depends_on` los IDs de las subtasks que deben completarse antes.
Si dos subtasks son independientes, deja `depends_on` vacío en ambas.
Ejemplo: Coder depende de Architect, QA depende de Coder.
```

### 3. Lógica de DAG en `handle_ai_execute_plan`

Archivo: `crates/merab-daemon/src/rpc/ai_methods.rs`

Reemplazar el `for subtask in &plan.subtasks` secuencial con un scheduler de DAG:

```rust
async fn execute_plan_dag(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    plan: &Task,
) -> Result<Vec<SubtaskResult>, ErrorObjectOwned> {
    let subtasks = &plan.subtasks;
    let mut completed: HashMap<String, SubtaskResult> = HashMap::new();
    let mut pending: Vec<&Task> = subtasks.iter().collect();

    while !pending.is_empty() {
        // Encontrar todas las subtasks listas (sin deps pendientes)
        let ready: Vec<&Task> = pending
            .iter()
            .filter(|t| t.depends_on.iter().all(|dep| completed.contains_key(dep)))
            .copied()
            .collect();

        if ready.is_empty() {
            // Dependencia circular o error en el plan
            return Err(to_rpc_error(MerabError::TaskFailed("circular dependency".into())));
        }

        // Ejecutar todas las listas en paralelo
        let handles: Vec<_> = ready.iter().map(|subtask| {
            let config = config.clone();
            let mcp = mcp_manager.clone();
            let subtask = (*subtask).clone();
            let ctx = build_context_from_completed(&completed);
            tokio::spawn(async move {
                execute_subtask(&config, &mcp, &subtask, &ctx).await
            })
        }).collect();

        // Esperar a que todas terminen
        for (subtask, handle) in ready.iter().zip(handles) {
            let result = handle.await??;
            completed.insert(subtask.id.clone(), result);
            pending.retain(|t| t.id != subtask.id);
        }
    }

    Ok(completed.into_values().collect())
}
```

### 4. Sin cambios en la CLI

El paralelismo es completamente interno al daemon. La CLI no cambia.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `crates/merab-core/src/multi_agent_pipeline.rs` | Agregar `depends_on: Vec<String>` a `Task` |
| `crates/merab-daemon/src/planner.rs` | Actualizar prompt para declarar dependencias |
| `crates/merab-daemon/src/rpc/ai_methods.rs` | Reemplazar loop secuencial con scheduler DAG |

## Verificación

```bash
merab ask "implementa función de login con tests"
# En los logs del daemon:
# [INFO] Executing 2 subtasks in parallel: Architect, Research
# [INFO] Architect completed in 12s
# [INFO] Research completed in 8s
# [INFO] Executing Coder (depends on: Architect, Research)
# ...
```

## Notas

- Si el LLM no declara dependencias, todas las subtasks son independientes y corren en paralelo.
- El número máximo de subtasks paralelas debe ser configurable en `merab.toml` (`ai.max_parallel_tasks = 4`).
- Los logs deben mostrar claramente cuáles subtasks corren en paralelo.
- Un fallo en una subtask paralela cancela las demás que dependan de ella.
