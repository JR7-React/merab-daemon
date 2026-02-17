# Forge — Sprint Documentation

Documentación detallada de cada sprint para continuidad del desarrollo.

## Sprints completados

| Sprint | Descripción | Commit | Doc |
|---|---|---|---|
| 1.0 | MVP Skeleton — workspace, tipos, CRUD, daemon, CLI | `b752f01` | [sprint-1](sprint-1-mvp-skeleton.md) |
| 2.1 | Agent Lifecycle — supervisor, restart policies | `a12543a` | [sprint-2.1](sprint-2.1-agent-lifecycle.md) |
| 2.2 | Message Passing — mensajería entre agentes | `c223911` | [sprint-2.2](sprint-2.2-message-passing.md) |
| 2.3 | MCP Implementation — rmcp SDK, tools discovery/invocation | `cf17cc9` | [sprint-2.3](sprint-2.3-mcp-implementation.md) |
| 2.4 | A2A Protocol — HTTP server, discovery, task execution | pendiente | [sprint-2.4](sprint-2.4-a2a-protocol.md) |
| 2.5 | Context Proxy — cache de tokens para LLMs | pendiente | [sprint-2.5](sprint-2.5-context-proxy.md) |
| 2.6 | Shared Memory — key-value store persistente | pendiente | [sprint-2.6](sprint-2.6-shared-memory.md) |
| 2.7 | Sandboxing Real — Windows Job Objects | pendiente | [sprint-2.7](sprint-2.7-sandboxing.md) |
| 2.8 | Polish & Stability — RAM limits, CLI polling, cleanup | pendiente | [sprint-2.8](sprint-2.8-polish.md) |
| 2.9 | Hardening — Config, Indexing, Error Handling | pendiente | [sprint-2.9](sprint-2.9-hardening.md) |
| 2.10 | TUI Dashboard — Real-time observability | pendiente | [sprint-2.10](sprint-2.10-tui.md) |
| 3.0 | Cleanup & Tests — 0 warnings, SQL injection fix, 48 tests | pendiente | [sprint-3.0](sprint-3.0-cleanup-tests.md) |
| 4.0 | AI Coordinator — forge-ai, llm integration, chat/ask commands | pendiente | [sprint-4.0](sprint-4.0-ai-coordinator.md) |
| 5.0 | First MCP Agent — forge-echo reference implementation | pendiente | [sprint-5.0](sprint-5.0-first-agent.md) |
| 6.0 | FileSystem Agent — forge-fs reading/writing files | pendiente | [sprint-6.0](sprint-6.0-fs-agent.md) |

## Build rápido

```bash
export PATH="/c/tools/mingw64/bin:$HOME/.cargo/bin:$PATH"
cargo build
```

## Estructura del proyecto

```
forge/
├── Cargo.toml                 # Workspace
├── docs/sprints/              # Esta documentación
├── crates/
│   ├── forge-core/            # Tipos: Agent, Message, Permission, Error
│   ├── forge-daemon/          # Servidor JSON-RPC (forged) — 127.0.0.1:9090
│   ├── forge-cli/             # CLI (forge) — clap
│   ├── forge-store/           # SQLite (rusqlite bundled)
│   ├── forge-transport/       # MCP client (rmcp), A2A (placeholder)
│   ├── forge-sandbox/         # Sandboxing (placeholder)
│   └── forge-ai/              # AI Client & Types (OpenRouter/Local Proxy)
```

## Todos los RPC methods actuales

| Method | Sprint | Descripción |
|---|---|---|
| `forge.ping` | 1.0 | Health check |
| `forge.registerAgent` | 1.0 | Registrar agente desde manifest |
| `forge.listAgents` | 1.0 | Listar agentes |
| `forge.getAgent` | 1.0 | Detalle de un agente |
| `forge.startAgent` | 1.0 | Iniciar proceso (MCP o Native) |
| `forge.stopAgent` | 1.0 | Detener proceso |
| `forge.unregisterAgent` | 1.0 | Eliminar agente |
| `forge.sendMessage` | 2.2 | Mensaje directo entre agentes |
| `forge.broadcastMessage` | 2.2 | Broadcast a todos |
| `forge.getMessages` | 2.2 | Mensajes pendientes |
| `forge.ackMessage` | 2.2 | Confirmar recepción |
| `forge.listTools` | 2.3 | Tools de un agente MCP |
| `forge.callTool` | 2.3 | Invocar tool MCP |
| `forge.ai.chat` | 4.0 | Chat con LLM (contexto history) |
| `forge.ai.orchestrate` | 4.0 | Orquestación one-shot de tareas |

## Todos los CLI commands actuales

```
forge ping
forge register <manifest.toml>
forge list
forge status <id>
forge start <id>
forge stop <id>
forge unregister <id>
forge send <from> <to> <content>
forge broadcast <from> <content>
forge messages <agent_id>
forge ack <message_id>
forge tools <agent_id>
forge call <agent_id> <tool_name> [arguments_json]
forge chat
forge ask <question>
```
