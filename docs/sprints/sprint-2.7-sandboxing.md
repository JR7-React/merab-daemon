# Sprint 2.7 — Real Sandboxing (Windows Job Objects)

**Estado**: Completado
**Commit**: (Pendiente de commit)

## Objetivo
Implementar aislamiento de procesos y límites de recursos para los agentes utilizando las capacidades nativas del SO (Windows Job Objects), asegurando que los procesos huérfanos se terminen automáticamente si el daemon muere.

## Componentes Implementados

### 1. Job Object Wrapper (`crates/forge-sandbox`)
Un wrapper seguro sobre las APIs de Windows (`windows-sys`).
- **`JobObject::new()`**:
  - Crea un nuevo Job Object nativo.
  - Aplica `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Esto garantiza que si el handle del Job Object se cierra (porque el daemon crashea o se cierra), todos los procesos asignados a él son terminados inmediatamente por el sistema operativo.
- **`assign_process(pid)`**: Asocia un proceso existente al Job Object.

### 2. Integración en el Runtime
- **`start_agent` (Daemon Handler)**:
  - Antes de lanzar un agente "nativo", crea una instancia de `JobObject`.
  - Spawnea el proceso hijo.
  - Inmediatamente asigna el PID al Job Object.
- **`ProcessSupervisor`**:
  - Se actualizó para almacenar el `JobObject` junto con el `Child` handle. Esto mantiene vivo el handle del Job mientras el agente está supervisado.
  - Al detener el agente o al reiniciar, el Job Object se gestiona adecuadamente.

## Beneficios
- **Limpieza Garantizada**: No más procesos de agentes "zombies" si el daemon falla.
- **Base para Límites**: Ahora es trivial agregar límites de memoria (RAM) o CPU simplemente extendiendo la configuración del Job Object en futuras iteraciones.

## Notas Técnicas
- En Linux, la implementación actual es un placeholder (`unix.rs`). La arquitectura permite implementar `cgroups` en el futuro sin cambiar el código del daemon.
- Dependencia clave: `windows-sys` con features `Win32_System_JobObjects`.
