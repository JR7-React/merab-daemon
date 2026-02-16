# Forge — Agent Runtime Engine

Runtime local en Rust para orquestar agentes de IA a nivel de sistema operativo.

## Objetivo

Forge es una infraestructura que permite registrar, lanzar, supervisar y comunicar agentes de IA como procesos del sistema operativo. El daemon central (`forged`) actua como orquestador: gestiona el ciclo de vida de cada agente, media la comunicacion entre ellos y expone una API JSON-RPC local.

El objetivo final es que cualquier desarrollador pueda:

- **Registrar agentes** escritos en cualquier lenguaje como procesos independientes
- **Supervisarlos** con restart automatico, deteccion de fallos y reconciliacion al reinicio
- **Comunicarlos** entre si mediante message passing a traves del daemon
- **Conectarlos** al ecosistema de IA mediante protocolos estandar (MCP de Anthropic, A2A de Google)
- **Compartir contexto** entre agentes via memoria persistente y cache de tokens LLM
- **Aislarlos** con sandboxing real (Windows Job Objects / cgroups)

## Arquitectura

```
forge (CLI) --> forged (daemon JSON-RPC :9090) --> agentes (procesos del SO)
                    |
                    +-- Registry (agentes en memoria)
                    +-- ProcessSupervisor (monitoreo de procesos)
                    +-- SQLite (persistencia)
                    +-- MCP/A2A transports (protocolos IA)
```

### Crates

| Crate | Descripcion |
|-------|-------------|
| `forge-core` | Tipos compartidos: Agent, Message, Permission, Error |
| `forge-daemon` | Servidor JSON-RPC, supervisor de procesos, binary `forged` |
| `forge-cli` | CLI interactivo, binary `forge` |
| `forge-store` | Persistencia SQLite |
| `forge-transport` | Protocolos A2A y MCP |
| `forge-sandbox` | Permisos y sandboxing de procesos |

## Estado

- **Sprint 1** (completado): Scaffold, tipos base, CRUD agentes, daemon, CLI
- **Sprint 2.1** (completado): Agent lifecycle real — ProcessSupervisor, deteccion de fallos, restart automatico, reconciliacion al startup, graceful shutdown
- **Sprint 2.2** (pendiente): Message passing entre agentes
- **Sprint 2.3** (pendiente): Implementacion MCP
- **Sprint 2.4** (pendiente): Implementacion A2A
- **Sprint 2.5** (pendiente): Context Proxy (cache tokens LLM)
- **Sprint 2.6** (pendiente): Memoria compartida
- **Sprint 2.7** (pendiente): Sandboxing real

## Uso rapido

```bash
# Compilar
cargo build --workspace

# Iniciar daemon
cargo run --bin forged

# En otra terminal: registrar y arrancar un agente
cargo run --bin forge -- register mi-agente.toml
cargo run --bin forge -- start <agent-id>
cargo run --bin forge -- list
cargo run --bin forge -- stop <agent-id>
```

## Requisitos

- Rust (edicion 2024)
- Windows: MinGW o VS Build Tools
- Unix: libc estandar
