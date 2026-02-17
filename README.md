# Merab — AI Agent Runtime Engine

Runtime local en Rust para orquestar agentes de IA a nivel de sistema operativo.

## Objetivo

Merab es una infraestructura que permite registrar, lanzar, supervisar y comunicar agentes de IA como procesos del sistema operativo. El daemon central (`merabd`) actua como orquestador: gestiona el ciclo de vida de cada agente, media la comunicacion entre ellos y expone una API JSON-RPC local.

El objetivo final es que cualquier desarrollador pueda:

- **Registrar agentes** escritos en cualquier lenguaje como procesos independientes
- **Supervisarlos** con restart automatico, deteccion de fallos y reconciliacion al reinicio
- **Comunicarlos** entre si mediante message passing a traves del daemon
- **Conectarlos** al ecosistema de IA mediante protocolos estandar (MCP de Anthropic, A2A de Google)
- **Compartir contexto** entre agentes via memoria persistente y cache de tokens LLM
- **Aislarlos** con sandboxing real (Windows Job Objects / cgroups)

## Arquitectura

```
merab (CLI) --> merabd (daemon JSON-RPC :9090) --> agentes (procesos del SO)
                    |
                    +-- Registry (agentes en memoria)
                    +-- ProcessSupervisor (monitoreo de procesos)
                    +-- SQLite (persistencia)
                    +-- MCP/A2A transports (protocolos IA)
```

### Crates

| Crate | Descripcion |
|-------|-------------|
| `merab-core` | Tipos compartidos: Agent, Message, Permission, Error |
| `merab-daemon` | Servidor JSON-RPC, supervisor de procesos, binary `merabd` |
| `merab-cli` | CLI interactivo, binary `merab` |
| `merab-store` | Persistencia SQLite |
| `merab-transport` | Protocolos A2A y MCP |
| `merab-sandbox` | Permisos y sandboxing de procesos |

## Estado

Ver [docs/sprints/README.md](docs/sprints/README.md) para el historial completo de sprints.

- **Sprint 1-8** (completado): MVP, lifecycle, message passing, MCP, AI Coordinator, agentes
- **Sprint 9-10** (completado): Multi-agent pipeline, Planner LLM, Personas especializadas

## Uso rapido

```bash
# Compilar
cargo build --workspace

# Inicializar (inicia daemon + registra agentes)
cargo run --bin merab -- init

# Chat interactivo
cargo run --bin merab -- chat

# Pregunta directa
cargo run --bin merab -- ask "Lee el README y dime que hace el proyecto"
```

## Configuracion

Archivo `merab.toml`:

```toml
[daemon]
host = "127.0.0.1"
rpc_port = 9090

[ai]
model = "arcee-ai/trinity-mini:free"

[ai.personas.coder]
model = "arcee-ai/trinity-mini:free"

[ai.personas.reviewer]
model = "stepfun/step-3.5-flash:free"
```

## Requisitos

- Rust (edicion 2024)
- Windows: MinGW o VS Build Tools
- Unix: libc estandar

## Licencia

MIT
