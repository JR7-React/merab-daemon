# Merab — Sprint Documentation

Documentación detallada de cada sprint para continuidad del desarrollo.

## Sprints completados

| Sprint | Descripción | Commit | Doc |
|---|---|---|---|
| 1.0 | MVP Skeleton — workspace, tipos, CRUD, daemon, CLI | `b752f01` | [sprint-1](sprint-1-mvp-skeleton.md) |
| 2.1 | Agent Lifecycle — supervisor, restart policies | `a12543a` | [sprint-2.1](sprint-2.1-agent-lifecycle.md) |
| 2.2 | Message Passing — mensajería entre agentes | `c223911` | [sprint-2.2](sprint-2.2-message-passing.md) |
| 2.3 | MCP Implementation — rmcp SDK, tools discovery/invocation | `cf17cc9` | [sprint-2.3](sprint-2.3-mcp-implementation.md) |
| 4.0 | AI Coordinator — forge-ai, llm integration, chat/ask commands | pendiente | [sprint-4.0](sprint-4.0-ai-coordinator.md) |
| 4.1 | Multi-Step Orchestration — loop de orquestación autónomo | completado | [sprint-4.1](sprint-4.1-multistep-orchestration.md) |
| 5.0 | First MCP Agent — forge-echo reference implementation | pendiente | [sprint-5.0](sprint-5.0-first-agent.md) |
| 6.0 | FileSystem Agent — forge-fs reading/writing files | pendiente | [sprint-6.0](sprint-6.0-fs-agent.md) |
| 7.0 | Shell Agent — forge-shell execute commands | completado | [sprint-7.0](sprint-7.0-shell-agent.md) |
| 8.0 | Interactive Chat TUI — forge-cli claude-code style | completado | [sprint-8.0](sprint-8.0-chat-tui.md) |
| 9.0 | Multi-Agent Pipeline — Personas, executePlan | completado | [sprint-9.0](sprint-9.0-multi-agent.md) |
| 10.0 | Planner LLM — Intelligent task decomposition | completado | [sprint-10.0](sprint-10.0-planner.md) |

## Estructura del proyecto

```
merab/
├── Cargo.toml                 # Workspace
├── merab.toml                 # Configuración
├── docs/sprints/              # Esta documentación
├── crates/
│   ├── merab-core/            # Tipos: Agent, Message, Permission, Error
│   ├── merab-daemon/          # Servidor JSON-RPC (merabd) — 127.0.0.1:9090
│   ├── merab-cli/             # CLI (merab) — clap
│   ├── merab-store/           # SQLite (rusqlite bundled)
│   ├── merab-transport/       # MCP client (rmcp), A2A
│   ├── merab-sandbox/         # Sandboxing
│   └── merab-ai/              # AI Client & Types (OpenRouter/Local Proxy)
```

## Todos los RPC methods actuales

| Method | Sprint | Descripción |
|---|---|---|
| `merab.ping` | 1.0 | Health check |
| `merab.registerAgent` | 1.0 | Registrar agente desde manifest |
| `merab.listAgents` | 1.0 | Listar agentes |
| `merab.getAgent` | 1.0 | Detalle de un agente |
| `merab.startAgent` | 1.0 | Iniciar proceso (MCP o Native) |
| `merab.stopAgent` | 1.0 | Detener proceso |
| `merab.unregisterAgent` | 1.0 | Eliminar agente |
| `merab.sendMessage` | 2.2 | Mensaje directo entre agentes |
| `merab.broadcastMessage` | 2.2 | Broadcast a todos |
| `merab.getMessages` | 2.2 | Mensajes pendientes |
| `merab.ackMessage` | 2.2 | Confirmar recepción |
| `merab.listTools` | 2.3 | Tools de un agente MCP |
| `merab.callTool` | 2.3 | Invocar tool MCP |
| `merab.ai.chat` | 4.0 | Chat con LLM (contexto history) |
| `merab.ai.orchestrate` | 4.1 | Orquestación multi-step autónoma |
| `merab.ai.plan` | 10.0 | Crear plan de subtasks |
| `merab.ai.executePlan` | 9.0 | Ejecutar plan con personas |

## Todos los CLI commands actuales

```
merab init
merab ping
merab list
merab chat
merab ask <question>
merab plan <task>
merab execute-plan <plan_json>
```
