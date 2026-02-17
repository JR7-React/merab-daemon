# Sprint 1 — MVP Skeleton

**Estado**: Completado
**Commit**: `b752f01`

## Objetivo
Crear el scaffold completo del workspace Cargo con tipos base, CRUD de agentes, daemon JSON-RPC y CLI funcional.

## Arquitectura creada

```
forge/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── forge-core/               # Tipos compartidos
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── agent.rs          # AgentId, ProtocolKind, AgentStatus, AgentManifest, AgentRecord, AgentSummary
│   │       ├── message.rs        # (placeholder)
│   │       ├── permission.rs     # (placeholder)
│   │       └── error.rs          # ForgeError
│   ├── forge-daemon/             # Servidor JSON-RPC (binary: forged)
│   │   └── src/
│   │       ├── main.rs           # Entry point, binds 127.0.0.1:9090
│   │       ├── lib.rs            # Module exports
│   │       ├── handlers.rs       # ForgeApi trait + ForgeRpc impl
│   │       ├── registry.rs       # AgentRegistry (in-memory HashMap)
│   │       └── process.rs        # is_process_alive()
│   ├── forge-cli/                # CLI (binary: forge)
│   │   └── src/
│   │       ├── main.rs           # Clap commands
│   │       └── client.rs         # ForgeClient (HTTP JSON-RPC)
│   ├── forge-store/              # SQLite persistence
│   │   └── src/
│   │       └── lib.rs            # Database struct, agents table
│   ├── forge-transport/          # Protocolos (placeholder)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── mcp.rs            # McpToolDefinition, McpResource (placeholders)
│   │       └── a2a.rs            # (placeholder)
│   └── forge-sandbox/            # Sandboxing (placeholder)
│       └── src/
│           └── lib.rs
```

## Tipos clave (forge-core)

```rust
pub type AgentId = Uuid;

pub enum ProtocolKind { A2a, Mcp, Native }
pub enum AgentStatus { Registered, Running, Stopped, Failed }

pub struct AgentManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub command: String,          // Ejecutable del agente
    pub args: Vec<String>,
    pub protocol: ProtocolKind,   // default: Native
    pub working_dir: Option<String>,
    pub restart_on_failure: bool,
}

pub struct AgentRecord {
    pub id: AgentId,
    pub manifest: AgentManifest,
    pub status: AgentStatus,
    pub pid: Option<u32>,
    pub registered_at, started_at, stopped_at: DateTime<Utc>,
    pub exit_code: Option<i32>,
}
```

## RPC Methods (daemon)

| Method | Params | Returns |
|---|---|---|
| `forge.ping` | — | `"pong"` |
| `forge.registerAgent` | `manifest: AgentManifest` | `AgentRecord` |
| `forge.listAgents` | — | `Vec<AgentSummary>` |
| `forge.getAgent` | `id: String` | `AgentRecord` |
| `forge.startAgent` | `id: String` | `AgentRecord` |
| `forge.stopAgent` | `id: String` | `AgentRecord` |
| `forge.unregisterAgent` | `id: String` | `bool` |

## CLI Commands

```bash
forge ping
forge register <manifest.toml>
forge list
forge status <id>
forge start <id>
forge stop <id>
forge unregister <id>
```

## Manifest TOML (ejemplo)

```toml
name = "mi-agente"
version = "0.1.0"
description = "Agente de ejemplo"
command = "python"
args = ["script.py"]
protocol = "native"
```

## Dependencias workspace

```toml
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
rusqlite = { version = "0.31", features = ["bundled"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "2"
anyhow = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
jsonrpsee = { version = "0.24", features = ["server", "client", "macros", "http-client"] }
```

## Build

```bash
export PATH="/c/tools/mingw64/bin:$HOME/.cargo/bin:$PATH"
cargo build
```

## SQLite (forge-store)

- Tabla `agents`: id, manifest_json, status, pid, registered_at, started_at, stopped_at, exit_code
- `Database::open(path)`, `insert_agent()`, `list_agents()`, `update_agent_status()`, `delete_agent()`
