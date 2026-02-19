# Merab — Project Instructions

> Este archivo es inyectado como contexto de sistema en toda sesión de `merab chat` y `merab ask`.
> Los modelos de IA deben respetarlo como regla estricta, no como sugerencia.

## Regla principal: límite de 500 líneas por archivo

**Ningún archivo `.rs` puede superar 500 líneas.**

Si necesitas agregar código a un archivo que ya tiene ~450 líneas:

1. Extrae la lógica a un módulo nuevo (e.g. `foo_impl.rs`)
2. Deja `foo.rs` como dispatcher delgado que llama a `foo_impl::`
3. El pre-commit hook rechazará automáticamente archivos sobre el límite

```
# Patrón correcto
foo.rs          ← dispatcher (~50-100 líneas)
foo_impl.rs     ← lógica real (~200-400 líneas)
```

## Convenciones del proyecto

- **Edición Rust**: 2024
- **Sin `unwrap()`** en código de producción — usa `?` o maneja el error
- **Logging**: `tracing::{info!, debug!, warn!, error!}` — nunca `println!`
- **Errores en librerías**: `thiserror` con `MerabError`
- **Errores en binarios**: `anyhow::Result`
- **Tests**: en el mismo archivo (`#[cfg(test)]`) o en `tests/`

## Estructura de crates

```
crates/
  merab-core/       ← tipos compartidos (MerabError, AgentRecord, etc.)
  merab-config/     ← MerabConfig + deserialización TOML
  merab-ai/         ← cliente Anthropic, ChatMessage, AiResponse
  merab-store/      ← SQLite via rusqlite
  merab-transport/  ← HTTP MCP + A2A clients
  merab-daemon/     ← servidor RPC (jsonrpsee)
  merab-cli/        ← binario `merab` (clap)
  merab-tui/        ← Monitor TUI (ratatui)
  agents/
    merab-fs/       ← agente MCP filesystem
    merab-shell/    ← agente MCP shell
    merab-git/      ← agente MCP git
    merab-echo/     ← agente MCP echo (test)
```

## Archivos dispatcher actuales

Estos archivos son dispatchers delgados — no agregar lógica aquí:

| Archivo | Lógica real en |
|---|---|
| `merab-cli/src/main.rs` | `agent_cmd`, `ask_cmd`, `jobs_cmd`, `sessions_cmd`, `config_cmd` |
| `merab-daemon/src/rpc/server.rs` | `agent_impls`, `store_impls`, `job_impls`, `ai_methods` |
| `merab-cli/src/chat_ui.rs` | `chat_render` |

## Comandos útiles

```bash
cargo build --workspace          # compilar todo
cargo test --workspace           # todos los tests
cargo clippy --workspace         # linter
merab init                       # iniciar daemon + registrar agentes
merab chat                       # chat interactivo TUI
merab ask "pregunta"             # orquestación single-shot
```
