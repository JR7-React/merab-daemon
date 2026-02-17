# Sprint 2.10 — Observability & TUI Dashboard

**Estado**: Planificación
**Objetivo**: Crear una interfaz de usuario de texto (TUI) profesional y de alto rendimiento que permita monitorear y controlar el runtime de Forge en tiempo real.

## 1. Diseño de la TUI (Ratatui)

La interfaz se dividirá en paneles (estilo `htop` o `k9s`):

- **Header**: Nombre del nodo, uptime y estado de los 3 servidores (RPC, A2A, Proxy).
- **Agents Panel (Left)**: Lista de agentes con barras de progreso para el uso de memoria (ej. `[|||.....] 120MB / 512MB`).
- **Shared Memory Panel (Right)**: Vista rápida de las últimas claves escritas en la memoria compartida.
- **Proxy Stats (Bottom Left)**: Contador de hits/misses y ahorro de tokens.
- **Log Monitor (Bottom Right)**: Feed en vivo de los eventos del sistema.

## 2. Soporte en el Backend (`forge-daemon`)

Para alimentar la TUI, el daemon necesita un "latido" de datos eficiente.

- **RPC Method**: `forge.getSystemStatus`
  - Retornará un objeto JSON masivo con:
    - Lista de agentes + su uso real de RAM (consultando el Job Object).
    - Estadísticas de la tabla `llm_cache`.
    - Lista de tareas A2A activas.
- **Métricas de Sandbox**: Extender `forge-sandbox` para obtener `JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION` (Memoria actual).

## 3. Estructura de Crates

- **`forge-tui`**: Nuevo crate en el workspace.
  - Dependencias: `ratatui`, `crossterm` (para input/output de terminal), `tokio`.
- **`forge-cli`**: El comando `forge monitor` o `forge top` invocará el binario de la TUI.

## 4. Plan de Implementación Paso a Paso

1.  **Sandbox Metrics**: Implementar `get_memory_usage()` en `forge-sandbox`.
2.  **Stats RPC**: Implementar `forge.getSystemStatus` en `forge-daemon`.
3.  **TUI Skeleton**: Crear el crate `forge-tui` y configurar el bucle principal de renderizado.
4.  **TUI Components**: Diseñar los widgets para Agentes, Memoria y Logs.
5.  **CLI Wiring**: Conectar el comando `forge top`.

## 5. Consideraciones de Grado Industrial
- **Eficiencia**: La TUI usará un hilo separado para renderizar a 10-30 FPS sin bloquear la lógica.
- **Robustez**: Manejo de redimensionamiento de ventana y desconexión del daemon.
