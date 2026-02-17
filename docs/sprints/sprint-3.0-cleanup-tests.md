# Sprint 3.0 — Cleanup & Tests

**Estado**: Completado
**Resultado**: 0 warnings, 48 tests passing

## Objetivo
Consolidar la calidad del codigo antes de agregar mas features. Eliminar warnings, corregir bugs de seguridad y agregar la primera suite de tests del proyecto.

## Parte 1: Cleanup

### 1a. Warnings de compilacion (7 fixes)

| Archivo | Issue | Fix |
|---------|-------|-----|
| `forge-store/src/memory.rs` | unused `DateTime` import | Quitar del import |
| `forge-config/src/lib.rs` | unused `PathBuf` import | Quitar import |
| `forge-transport/src/a2a.rs` (x3) | unused `mut` en res | `let res =` sin mut |
| `forge-cli/src/client.rs` | unused `TaskDetails` import | Quitar del import |
| `forge-daemon/src/a2a_server.rs` | unused field `jsonrpc` | `#[allow(dead_code)]` |

### 1b. SQL Injection en `list_memory_keys`

**Antes** (vulnerable):
```rust
format!("SELECT key FROM shared_memory WHERE key LIKE '{}%' ...", p)
```

**Despues** (parameterizado):
```rust
"SELECT key FROM shared_memory WHERE key LIKE ?1 AND ..."
let like_pattern = format!("{}%", p);
stmt.query_map(params![like_pattern, now], ...)?;
```

### 1c. Proxy reutiliza `reqwest::Client`

- Se agrego `http_client: reqwest::Client` a `ProxyContext`
- Se construye una sola vez en `main.rs` con timeout de 60s
- `forward_request` usa `&ctx.http_client` en vez de crear uno nuevo

### 1d. A2A Agent Card URL dinamica

- Se agrego `a2a_port: u16` a `A2AContext`
- `handle_agent_card` usa `format!("http://localhost:{}", ctx.a2a_port)` en vez de hardcodear `:8080`

### 1e. A2A Content-Type headers

- `handle_agent_card` y `handle_rpc` ahora retornan `Content-Type: application/json`

## Parte 2: Tests (48 total)

### forge-core — 14 unit tests (`crates/forge-core/src/tests.rs`)

- AgentManifest TOML roundtrip (serialize/deserialize)
- AgentManifest con protocol MCP
- Default protocol es Native
- ProtocolKind JSON serialization (mcp, a2a, native)
- ProtocolKind JSON deserialization
- AgentRecord::new genera UUID valido y status Registered
- AgentRecord IDs son unicos
- ForgeError::code() retorna codigos correctos (13 variantes)
- ForgeError display
- Message::new genera ID, timestamps, status Pending
- Message broadcast (to_agent = None)
- TaskStatus to_string (5 variantes)
- TaskStatus from &str (5 + unknown -> Failed)
- AgentStatus serialization (4 variantes)

### forge-store — 22 unit tests (`crates/forge-store/src/tests.rs`)

Todos usan `Database::open_in_memory()`:

**Agents (7):** insert+get, list, get_by_name, update_status, update_exit, delete, duplicate name fails

**Messages (4):** insert+get, acknowledge, delete_agent_messages, broadcast (visible a otros, no al sender)

**Tasks (2):** create+get, update_status con output

**Cache (3):** store+get, miss returns None, stats (hit counting)

**Memory (6):** put+get, delete, list con prefix, TTL expiration, SQL injection safety, overwrite (upsert)

### forge-daemon — 12 integration tests (`crates/forge-daemon/tests/rpc_integration.rs`)

Server JSON-RPC real en puerto efimero (port 0), usando jsonrpsee HttpClient:

- `ping` -> "pong"
- `registerAgent` -> retorna record con UUID
- `listAgents` -> contiene agente registrado
- `getAgent` -> retorna el mismo agente
- `registerAgent` duplicado -> error
- `startAgent` agente inexistente -> error
- `stopAgent` agente no running -> error
- `unregisterAgent` -> limpia todo
- `memory.put` + `memory.get` + `memory.delete` + get returns None
- `memory.put` con TTL=1s -> sleep(2s) -> get returns None
- `memory.list` con y sin prefix
- `getSystemStatus` -> status valido

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/forge-store/src/memory.rs` | Fix SQL injection + quitar unused import |
| `crates/forge-config/src/lib.rs` | Quitar unused import |
| `crates/forge-transport/src/a2a.rs` | Quitar 3x unused mut |
| `crates/forge-cli/src/client.rs` | Quitar unused import |
| `crates/forge-daemon/src/a2a_server.rs` | Fix unused field + Content-Type + dynamic URL |
| `crates/forge-daemon/src/proxy.rs` | Reusar reqwest client |
| `crates/forge-daemon/src/main.rs` | Pasar config a A2AContext + construir reqwest client |

## Archivos nuevos

| Archivo | Contenido |
|---------|-----------|
| `crates/forge-core/src/tests.rs` | 14 unit tests |
| `crates/forge-store/src/tests.rs` | 22 unit tests |
| `crates/forge-daemon/tests/rpc_integration.rs` | 12 integration tests |

## Verificacion

```bash
cargo build --workspace   # 0 warnings
cargo test --workspace    # 48 tests, 0 failed
```
