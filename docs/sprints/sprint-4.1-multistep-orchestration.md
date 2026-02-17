# Sprint 4.1 — Multi-Step Orchestration Loop

**Estado**: Completado
**Prerequisito**: Sprint 4.0 (AI Coordinator)

## Objetivo

Evolucionar `forge.ai.orchestrate` de un sistema de paso único (1 tool call → consolidación) a un **loop multi-step** donde el LLM puede encadenar múltiples tool calls hasta completar la tarea. Esta es la pieza clave para que Forge pueda construirse a sí mismo.

## Problema Anterior

Sprint 4.0 implementó orquestación de un solo paso:
```
User task → LLM → 1 tool call → execute → LLM consolida → respuesta final
```

Esto limitaba severamente las capacidades: el LLM no podía leer un archivo, luego escribir cambios, luego ejecutar tests — todo en una misma tarea.

## Solución: Loop de Orquestación

```
User task → LLM → tool call? → execute → feed result → LLM → tool call? → ...
                   ↑ NO: return final answer            ↑ loop hasta max_steps
```

### Flujo detallado

1. El usuario envía una tarea via `forge.ai.orchestrate`.
2. Se construye un system prompt dinámico con las tools MCP disponibles.
3. **Loop** (hasta `max_orchestration_steps`):
   a. Se envía el historial de conversación completo al LLM.
   b. Si el LLM responde **sin** tool call → tarea completa, retorna.
   c. Si el LLM responde **con** tool call → se ejecuta via McpManager.
   d. El resultado de la tool se agrega al historial como mensaje de usuario.
   e. Se repite desde (a).
4. Si se alcanza el límite de pasos, se le pide al LLM un resumen final.

### Ejemplo de flujo real

```
Tarea: "Lee el README.md y agrega una sección de instalación"

Step 0: LLM → tool_call: fs.read(path="README.md")
Step 1: Tool result → "# Forge\nRuntime engine..."
        LLM → tool_call: fs.write(path="README.md", content="# Forge\n...\n## Installation\n...")
Step 2: Tool result → "Successfully wrote to README.md"
        LLM → "Done. Added an Installation section to README.md."
        (no tool call → loop ends, 3 steps total)
```

## Cambios Implementados

### 1. Config: `max_orchestration_steps`

**Archivo**: `crates/forge-config/src/lib.rs`
- Nuevo campo `max_orchestration_steps: u32` en `AiConfig`
- Default: 10 pasos (configurable via `forge.toml` o env `FORGE__AI__MAX_ORCHESTRATION_STEPS`)

**Archivo**: `forge.toml`
- Nuevo campo `max_orchestration_steps = 10` en sección `[ai]`

### 2. Error: `MaxStepsExceeded`

**Archivo**: `crates/forge-ai/src/error.rs`
- Nuevo variant `MaxStepsExceeded(u32)` — indica que se alcanzó el límite

### 3. System Prompt Multi-Step

**Archivo**: `crates/forge-daemon/src/prompts.rs`
- Instrucciones explícitas para el LLM sobre el ciclo multi-step
- Reglas claras: 1 tool call por respuesta, texto plano cuando termina
- Énfasis en no llamar tools si el objetivo ya se logró

### 4. Loop de Orquestación

**Archivo**: `crates/forge-daemon/src/rpc/ai_methods.rs`
- `handle_ai_orchestrate()` reescrito con loop `for step in 0..max_steps`
- Historial de conversación crece con cada paso (mensajes assistant + tool results)
- Nueva función auxiliar `execute_tool_call()` extraída para claridad
- Si se alcanza el límite, se pide un resumen final al LLM (graceful degradation)
- Logging estructurado: step number, tool name, completion status

## Seguridad y Límites

| Control | Valor | Configurable |
|---------|-------|-------------|
| Max pasos por orquestación | 10 | `ai.max_orchestration_steps` |
| Max tokens por respuesta LLM | 1024 | `ai.max_tokens` |
| Timeout por tool call | Inherente al MCP client | Via transport |

El límite de pasos evita loops infinitos si el LLM no converge. Cuando se alcanza, el sistema no falla — pide un resumen de lo logrado.

## Archivos Modificados

| Archivo | Cambio |
|---------|--------|
| `crates/forge-config/src/lib.rs` | Campo `max_orchestration_steps` en `AiConfig` |
| `crates/forge-ai/src/error.rs` | Variant `MaxStepsExceeded` |
| `crates/forge-daemon/src/prompts.rs` | System prompt multi-step |
| `crates/forge-daemon/src/rpc/ai_methods.rs` | Loop multi-step + `execute_tool_call()` |
| `forge.toml` | Campo `max_orchestration_steps` |

## Verificación

```bash
cargo build --workspace   # 0 errors
cargo test --workspace    # 56 tests pass
cargo clippy --workspace  # 0 new warnings
```

## Camino al Self-Build

Con el loop multi-step, Forge puede ahora (en teoría):

1. `fs.read` → leer código fuente
2. `fs.write` → escribir/modificar archivos
3. `shell.execute` → `cargo build`, `cargo test`, `cargo fmt`
4. `git.status`, `git.add`, `git.commit` → gestionar cambios
5. Repetir hasta que la tarea esté completa

El próximo paso es probarlo end-to-end con una tarea real.
