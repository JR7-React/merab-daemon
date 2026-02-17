# Sprint 2.6 — Shared Memory (Key-Value Store)

**Estado**: Completado
**Commit**: (Pendiente de commit)

## Objetivo
Implementar un almacén clave-valor persistente y compartido, permitiendo a los agentes guardar y recuperar estado global o específico del agente sin necesidad de pasarse mensajes constantemente.

## Componentes Implementados

### 1. Persistencia (`crates/forge-store/src/memory.rs`)
- **Tabla**: `shared_memory`.
- **Campos**: `key` (TEXT PK), `value` (TEXT JSON), `agent_id` (opcional), `expires_at` (opcional).
- **Lógica**:
  - `put_memory(key, value, ttl)`: Inserta o actualiza. Soporta TTL en segundos.
  - `get_memory(key)`: Recupera valor si no ha expirado.
  - `delete_memory(key)`: Elimina clave.
  - `list_memory_keys(prefix)`: Lista claves activas, opcionalmente filtradas por prefijo.

### 2. API RPC (`forge-daemon`)
Nuevos métodos expuestos en el servidor JSON-RPC:
- `forge.memory.put`: Guarda un valor JSON.
- `forge.memory.get`: Recupera un valor JSON.
- `forge.memory.delete`: Borra una clave.
- `forge.memory.list`: Lista claves.

### 3. CLI (`forge`)
Nuevos subcomandos para gestión manual:
- `forge memory put <key> <value> [--ttl <seconds>]`
- `forge memory get <key>`
- `forge memory delete <key>`
- `forge memory list [--prefix <prefix>]`

## Uso Ejemplo
```bash
# Guardar configuración global
forge memory put config/theme '{"mode":"dark"}'

# Guardar estado efímero (1 hora)
forge memory put status/agent-1 "processing" --ttl 3600

# Leer
forge memory get config/theme
```
