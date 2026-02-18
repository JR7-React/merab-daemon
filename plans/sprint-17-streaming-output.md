# Sprint 17 — Streaming Output

## Objetivo

`merab ask` muestra la respuesta en tiempo real mientras el LLM genera, en vez de esperar a que todo termine. Para tareas largas (5+ subtasks) el usuario ve progreso inmediato.

## Problema actual

`merab ask "tarea larga"` bloquea el terminal 30-60 segundos sin output. El usuario no sabe si cuelga o trabaja.

## Comportamiento objetivo

```bash
$ merab ask "implementa el módulo de pagos"

[Planner] Descomponiendo tarea...
[Plan] 4 subtasks: setup → model → service → tests

[engineer] Analizando requisitos...
  → Revisando estructura del proyecto...
  → Identificando dependencias...

[coder] Implementando PaymentModel...
  → Creando src/models/payment.rs
  → struct Payment { id, amount, currency, status }...

[coder] Implementando PaymentService...     ← aparece mientras coder trabaja
  → Función charge() con validación...

✓ Completado en 42s
```

## Implementación

### Enfoque: Server-Sent Events (SSE) para el canal RPC → CLI

El JSON-RPC actual es request/response. Para streaming se necesita un canal separado.

**Opción A (simple):** Puerto SSE dedicado en el daemon (`/events`). El CLI se subscribe antes de lanzar la tarea.

**Opción B (más simple aún):** El daemon escribe eventos en un archivo temporal. El CLI hace tail mientras espera el resultado RPC.

**Recomendación: Opción B** — cero dependencias nuevas, compatible con la arquitectura actual.

### Paso a paso

#### `crates/merab-daemon/src/rpc/ai_methods.rs`
Agregar un `EventSink` opcional que escribe eventos a un archivo:
```rust
pub struct ProgressEvent {
    pub persona: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}
```
En `execute_subtask()`, emitir eventos al iniciar/terminar cada subtask.

#### `crates/merab-daemon/src/rpc/server.rs`
Nuevo método RPC:
```
merab.ai.orchestrate.stream(task, event_file) → AiResponse
```
El daemon escribe eventos JSON (one per line) al `event_file` mientras trabaja.

#### `crates/merab-cli/src/main.rs`
En `Commands::Ask`, antes de llamar a `ai_orchestrate`:
1. Crear archivo temporal para eventos
2. Spawn tokio task que hace tail del archivo e imprime
3. Llamar al RPC con el path del archivo de eventos
4. Al finalizar, cancelar el tail task

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-core/src/events.rs` | Nuevo — `ProgressEvent` struct |
| `merab-daemon/src/events.rs` | `EventSink` — escribe a archivo temp |
| `merab-daemon/src/dag.rs` | Emitir eventos en execute_subtask |
| `merab-daemon/src/rpc/server.rs` | Nuevo param `event_file` en orchestrate |
| `merab-cli/src/main.rs` | Tail del archivo de eventos en paralelo |

## Verificación

```bash
merab ask "crea 4 archivos distintos"
# Debe mostrar progreso de cada subtask en tiempo real
# No debe duplicar output al terminar
```
