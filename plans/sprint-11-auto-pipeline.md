# Sprint 11 — Auto-Pipeline

## Objetivo

Hacer que `merab ask "X"` use automáticamente el flujo completo **Plan → ExecutePlan** en lugar del loop single-LLM actual. El usuario no tiene que saber que existe un planner ni un pipeline — simplemente funciona.

## Problema actual

```
merab ask "añade autenticación"
  → ai_orchestrate()
    → 1 LLM + tool loop (max 10 pasos)
    → responde con texto
```

El flujo actual es un solo LLM que decide qué tools usar. No hay planificación real, no hay agentes especializados, no hay división del trabajo.

## Flujo objetivo

```
merab ask "añade autenticación"
  → PlannerAgent.decompose()          ← genera el plan con subtasks y personas
    → handle_ai_execute_plan()        ← ejecuta cada subtask con su persona
      → [Architect] analiza el código
      → [Coder]     implementa la feature
      → [Reviewer]  revisa el diff
      → [QA]        corre los tests
    → resultado: artefactos reales
```

## Implementación

### 1. Reescribir `handle_ai_orchestrate` en `ai_methods.rs`

El método `handle_ai_orchestrate` pasa a ser el nuevo flujo Plan → Execute:

```rust
pub async fn handle_ai_orchestrate(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    task: String,
) -> Result<AiResponse, ErrorObjectOwned> {
    // 1. Planificar
    let mut planner = PlannerAgent::new(config);
    let plan = planner.decompose(&task).await?;

    // 2. Ejecutar el plan
    let plan_json = serde_json::to_string(&plan)...;
    handle_ai_execute_plan(config, mcp_manager, plan_json).await
}
```

El loop single-LLM anterior se renombra a `handle_ai_chat_loop` y sigue siendo usado por `merab.ai.chat`.

### 2. Mantener `merab.ai.chat` con el loop actual

El chat interactivo sigue usando el loop simple (un LLM con tools), ya que es más rápido para preguntas conversacionales cortas. Solo `merab ask` (que llama a `ai.orchestrate`) usa el pipeline completo.

### 3. Sin cambios en la CLI

`Commands::Ask` no cambia — el cambio es completamente interno en el daemon.

## Archivos a modificar

| Archivo | Cambio |
|---------|--------|
| `crates/merab-daemon/src/rpc/ai_methods.rs` | Reescribir `handle_ai_orchestrate` |

## Verificación

```bash
merab ask "crea una función que calcule fibonacci en src/math.rs"
# Esperado:
# [Planner] Descomponiendo tarea...
# [Architect] Analizando estructura del proyecto...
# [Coder] Implementando fibonacci...
# [Reviewer] Revisando implementación...
# [QA] Verificando...
# ✓ Completado en 4 pasos
```

## Notas

- El planner puede fallar si el LLM no genera JSON válido. El fallback debe ser el loop simple anterior, no un crash.
- Agregar tracing en cada paso del pipeline para que el usuario vea progreso.
