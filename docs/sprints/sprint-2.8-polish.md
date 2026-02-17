# Sprint 2.8 — Polish & Stability

**Estado**: Completado
**Commit**: (Pendiente de commit)

## Objetivo
Refinar las implementaciones de los sprints anteriores para mejorar la robustez, seguridad y experiencia de usuario antes de cerrar la versión 2.x.

## Mejoras Implementadas

### 1. Sandbox: Límites de Recursos (RAM)
- **Problema**: El sandboxing (Sprint 2.7) manejaba el ciclo de vida pero no impedía el consumo excesivo de memoria.
- **Solución**:
  - Se implementó `set_memory_limit` en `forge-sandbox` usando `JOB_OBJECT_LIMIT_PROCESS_MEMORY`.
  - El daemon ahora aplica un límite por defecto de **512 MB** a cada agente nativo al iniciarlo.

### 2. A2A CLI: Polling de Resultados
- **Problema**: El comando `forge a2a-send` terminaba inmediatamente tras enviar la tarea, obligando al usuario a consultar el estado manualmente.
- **Solución**:
  - Se implementó un bucle de espera activa (polling) en la CLI.
  - El comando ahora muestra un indicador de progreso (`...`) y espera hasta que la tarea remota esté `completed` o `failed`.
  - Muestra el JSON de salida completo al finalizar.

### 3. Shared Memory: Limpieza (TTL)
- **Problema**: Los datos expirados permanecían en SQLite indefinidamente, ocultos pero ocupando espacio.
- **Solución**:
  - Se añadió `cleanup_expired_memory()` en `forge-store`.
  - Se implementó una estrategia de "Lazy Cleanup": cada vez que se escribe un nuevo valor (`memory_put`), se purgan los expirados.

## Archivos Afectados
- `crates/forge-sandbox/src/windows.rs`
- `crates/forge-daemon/src/handlers.rs`
- `crates/forge-cli/src/main.rs`
- `crates/forge-cli/src/client.rs`
- `crates/forge-store/src/memory.rs`
