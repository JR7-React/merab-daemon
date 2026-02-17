# Sprint 2.6 — Shared Memory (Key-Value Store)

**Estado**: Planificación
**Objetivo**: Implementar un almacén clave-valor persistente y compartido, permitiendo a los agentes guardar y recuperar estado global o específico del agente.

## 1. Concepto

Shared Memory es un almacén simple persistido en SQLite donde los agentes pueden guardar datos JSON arbitrarios bajo una clave.
- **Scope**: Global (cualquier agente puede leer/escribir si conoce la clave) o Scoped (prefijo por agente, aunque para MVP será global).
- **TTL (Time To Live)**: Opcional, para datos efímeros.

## 2. Base de Datos (`forge-store`)

Nueva tabla `memory_store` (distinta de la tabla `memory` existente que parece ser interna del agente).

```sql
CREATE TABLE shared_memory (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,          -- JSON string
    agent_id    TEXT,                   -- Quién lo escribió (opcional)
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT                    -- Opcional TTL
);
```

## 3. API RPC (`forge-daemon`)

Nuevos métodos en `ForgeApi`:

- `forge.memory.put(key: String, value: Value, ttl_seconds: Option<u64>) -> bool`
- `forge.memory.get(key: String) -> Option<Value>`
- `forge.memory.delete(key: String) -> bool`
- `forge.memory.list(prefix: Option<String>) -> Vec<String>`

## 4. CLI (`forge-cli`)

Nuevos subcomandos:
- `forge memory put <key> <value> [--ttl <seconds>]`
- `forge memory get <key>`
- `forge memory delete <key>`
- `forge memory list [--prefix <prefix>]`

## 5. Implementación Paso a Paso

1.  **Persistencia**:
    - Actualizar `forge-store/src/db.rs` con la migración.
    - Actualizar `forge-store/src/memory.rs` (actualmente vacío) con la lógica CRUD.

2.  **Daemon RPC**:
    - Añadir métodos al trait `ForgeApi` en `handlers.rs`.
    - Implementar la lógica conectando con `db.store_memory`, etc.

3.  **Cliente y CLI**:
    - Actualizar `ForgeClient` en `forge-cli/src/client.rs`.
    - Añadir subcomandos en `forge-cli/src/main.rs`.

## 6. Consideraciones
- **Concurrencia**: SQLite maneja el bloqueo de escritura, así que es seguro para múltiples agentes concurrentes.
- **JSON**: El valor se guarda como TEXT pero se trata como JSON `serde_json::Value` en la capa de aplicación.
