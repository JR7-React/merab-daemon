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
| `forge.ai.orchestrate` | 4.1 | Orquestación multi-step autónoma |
| `forge.ai.plan` | 10.0 | Crear plan de subtasks |
| `forge.ai.executePlan` | 9.0 | Ejecutar plan con personas |

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
