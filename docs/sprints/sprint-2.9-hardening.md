# Sprint 2.9 — Hardening & Architecture

**Estado**: Completado
**Commit**: (Pendiente de commit)

## Objetivo
Elevar la calidad del código de "funcional" a "grado industrial", enfocándose en la configurabilidad, escalabilidad y mantenibilidad antes de cerrar la versión 2.x.

## Mejoras Críticas Implementadas

### 1. Sistema de Configuración Global (`forge-config`)
- **Problema**: Puertos, límites y paths estaban hardcodeados.
- **Solución**:
  - Nuevo crate `forge-config`.
  - Carga en cascada: `Default` -> `config.toml` -> `ENV VARS`.
  - Configuración centralizada para Daemon, Proxy y Sandbox.

### 2. Capability Indexing (Escalabilidad)
- **Problema**: El descubrimiento de herramientas (`tasks/send`) iteraba sobre todos los procesos agentes (O(N)), lo cual es lento y no escala.
- **Solución**:
  - `McpManager` ahora mantiene un **Índice Invertido** en memoria (`Tool Name -> AgentId`).
  - El routing de tareas ahora es **O(1)** (búsqueda en Hash Map).
  - El endpoint `/.well-known/agent.json` se sirve desde memoria sin despertar agentes.

### 3. Structured Error Handling
- **Problema**: Uso excesivo de errores genéricos (`-32000`).
- **Solución**:
  - Definición robusta de `ForgeError` en `forge-core` con variantes semánticas (`ToolNotFound`, `RateLimit`, etc.).
  - Mapeo automático a códigos JSON-RPC estándar (`-32001`, etc.).
  - **Nota Post-Implementación**: Se resolvió el problema de la "Orphan Rule" en `forge-daemon` utilizando una función helper `to_rpc_error()` en lugar de un `impl From<>` directo, asegurando la conformidad con las reglas de Rust.

### 4. Estándares de Ingeniería
- Creación de `docs/RULES.md` definiendo límites de tamaño de archivo, estilo y prácticas de seguridad (no `unwrap()`).

## Refactorización
- División de `handlers.rs` (que estaba creciendo demasiado) y limpieza de deuda técnica.
- Centralización de la lógica RPC.

## Resultado
Una base sólida, rápida y configurable lista para producción.
