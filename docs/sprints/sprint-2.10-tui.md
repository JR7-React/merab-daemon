# Sprint 2.10 — Observability & TUI Dashboard

**Estado**: Completado
**Commit**: (Pendiente de commit)

## Objetivo
Crear una interfaz visual profesional en la terminal (TUI) que permita monitorear el estado del nodo, el consumo de recursos de los agentes y el rendimiento del proxy en tiempo real.

## Componentes Implementados

### 1. Métricas de Sandbox (`crates/forge-sandbox`)
- Se implementó `get_memory_usage()` en Windows usando `QueryInformationJobObject`.
- Permite obtener el consumo real de RAM de cada agente (Native) de forma individual.

### 2. Backend de Estadísticas (`crates/forge-daemon`)
- Nuevo método RPC: `forge.getSystemStatus`.
- Recopila en un solo "latido" (heartbeat):
  - Estado y RAM de todos los agentes.
  - Estadísticas del LLM Proxy (Total requests, Cache Hits).
  - Información del nodo (Uptime, puertos configurados, versión).

### 3. Forge TUI (`crates/forge-tui`)
- Nuevo crate basado en `ratatui` y `crossterm`.
- **Interfaz Dividida**:
  - **Header**: Info del nodo y uptime.
  - **Panel de Agentes**: Lista visual con indicadores de estado (Running/Stopped) y consumo de memoria.
  - **Panel de Estadísticas**: Resumen del rendimiento del Proxy y puertos activos.
  - **Footer**: Branding y comandos rápidos.

### 4. Integración CLI
- Nuevo comando: `forge monitor`.
- Inicia el bucle de renderizado de la TUI y realiza polling automático al daemon cada 500ms para actualizar los datos.

## Cómo ejecutar
1. Iniciar daemon: `cargo run -p forge-daemon`
2. Iniciar monitor: `cargo run -p forge-cli -- monitor`
3. Salir: Presionar `Q`.

**Nota Final**: Durante la fase de pruebas, se configuró con éxito el entorno de desarrollo para usar el toolchain `stable-x86_64-pc-windows-msvc` de Rust, resolviendo los problemas de compilación relacionados con `dlltool.exe` y estableciendo un entorno de desarrollo profesional y nativo para Windows.

