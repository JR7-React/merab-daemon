# Sprint 2.3 — MCP Implementation (Model Context Protocol)

**Estado**: Completado
**Commit**: pendiente

## Objetivo
Forge actúa como MCP Host/Client: lanza procesos MCP server, se comunica via stdin/stdout JSON-RPC, descubre tools y permite invocarlos usando el SDK `rmcp`.

## Dependencia principal

```toml
# Workspace Cargo.toml
rmcp = { version = "0.15", features = ["client", "transport-child-process"] }
```

## Archivos modificados/creados

### 1. `crates/forge-transport/src/mcp.rs` — McpClient (reescritura completa)

```rust
pub struct McpClient {
    service: RunningService<rmcp::RoleClient, ()>,
    agent_id: AgentId,
}
```

**Métodos:**
- `connect(agent_id, command) -> Result<Self>` — Crea `TokioChildProcess` desde un `tokio::process::Command`, llama `.serve()` para hacer el handshake MCP, retorna el client conectado.
- `list_tools() -> Result<Vec<McpToolInfo>>` — Lista tools del server MCP.
- `call_tool(name: String, arguments) -> Result<CallToolResult>` — Invoca un tool.
- `shutdown(self)` — `service.cancel()` para cerrar limpio.

**McpToolInfo** (tipo simplificado para JSON-RPC responses):
```rust
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

### 2. `crates/forge-daemon/src/mcp_manager.rs` — NUEVO

```rust
pub struct McpManager {
    clients: Mutex<HashMap<AgentId, Arc<McpClient>>>,
}
```

**Métodos:**
- `add_client(id, client)` — Registra un McpClient.
- `get_client(id) -> Option<Arc<McpClient>>` — Obtiene referencia.
- `remove_client(id)` — Shutdown graceful + eliminar.
- `shutdown_all()` — Cierra todas las conexiones (daemon exit).

### 3. `crates/forge-daemon/src/handlers.rs` — Cambios principales

**ForgeRpc** — nuevo campo:
```rust
pub mcp_manager: Arc<McpManager>,
```

**start_agent()** — bifurcación por protocolo:
- Si `protocol == Mcp`:
  - Configura stdin/stdout como `Stdio::piped()`
  - Llama `McpClient::connect(uuid, cmd)` (spawn + handshake)
  - Registra client en `mcp_manager`
  - NO pasa child al supervisor (rmcp gestiona el proceso)
- Si `protocol != Mcp`:
  - Comportamiento original (Stdio::null, supervisor)

**stop_agent()** — antes de matar:
- Si es MCP: `mcp_manager.remove_client(id)` para shutdown graceful

**unregister_agent()** — limpia MCP client si existe

**Nuevos RPCs:**

| Method | Params | Returns |
|---|---|---|
| `forge.listTools` | `agent_id: String` | `Vec<McpToolInfo>` |
| `forge.callTool` | `agent_id, tool_name, arguments: Value` | `serde_json::Value` |

Ambos validan que el agente sea MCP y esté Running.

### 4. `crates/forge-daemon/src/main.rs` — Wiring

```rust
let mcp_manager = Arc::new(McpManager::new());
// ... pasado a ForgeRpc
// En shutdown:
mcp_manager.shutdown_all().await;
supervisor.shutdown_all().await;
```

### 5. `crates/forge-cli/src/client.rs` — 2 nuevos métodos

```rust
pub async fn list_tools(&self, agent_id: &str) -> Result<Vec<serde_json::Value>>
pub async fn call_tool(&self, agent_id: &str, tool_name: &str, arguments: Value) -> Result<Value>
```

### 6. `crates/forge-cli/src/main.rs` — 2 nuevos comandos

```bash
forge tools <agent_id>              # Lista tools del agente MCP
forge call <agent_id> <tool> [json] # Llama un tool (args default: "{}")
```

## Cómo usar (ejemplo con server Python)

### 1. Crear MCP server (Python)

```python
# echo_server.py
import sys, json

def handle_request(req):
    if req["method"] == "initialize":
        return {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "echo", "version": "0.1.0"}}
    elif req["method"] == "tools/list":
        return {"tools": [{"name": "echo", "description": "Echoes text", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}
    elif req["method"] == "tools/call":
        text = req.get("params", {}).get("arguments", {}).get("text", "")
        return {"content": [{"type": "text", "text": text}]}
    return None

# ... stdin/stdout JSON-RPC loop
```

(O mejor usar `mcp` Python SDK: `pip install mcp`)

### 2. Manifest TOML

```toml
name = "echo-mcp"
version = "0.1.0"
description = "MCP echo server"
command = "python"
args = ["echo_server.py"]
protocol = "mcp"
```

### 3. Flujo

```bash
forge register echo-mcp.toml        # Registrar
forge start <id>                     # Spawn + MCP handshake
forge tools <id>                     # Ver tools disponibles
forge call <id> echo '{"text":"hi"}' # Invocar tool
forge stop <id>                      # Shutdown graceful
```

## API rmcp usada

```rust
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};

// Conectar
let transport = TokioChildProcess::new(command)?;
let service = ().serve(transport).await?;

// Listar tools
let tools = service.list_tools(Default::default()).await?;

// Llamar tool
let result = service.call_tool(CallToolRequestParams {
    meta: None,
    name: "tool_name".into(),
    arguments: Some(json_map),
    task: None,
}).await?;

// Cerrar
service.cancel().await?;
```

## Notas técnicas
- rmcp gestiona el proceso child internamente via `TokioChildProcess`
- El PID no está disponible directamente desde rmcp, se registra como `None`
- `McpClient.call_tool()` toma `name: String` (no `&str`) porque rmcp requiere `'static` lifetime
- El `()` en `.serve()` es un `ClientHandler` vacío (sin sampling/elicitation)
- `CallToolResult` se serializa a `serde_json::Value` para la respuesta RPC

## Siguiente sprint
Sprint 2.4: Implementación A2A (Agent-to-Agent protocol de Google) — Agent Cards, HTTP discovery.
