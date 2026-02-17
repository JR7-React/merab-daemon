# Sprint 2.7 — Real Sandboxing (Windows Job Objects)

**Estado**: Planificación
**Objetivo**: Implementar aislamiento de procesos y límites de recursos para los agentes utilizando las capacidades nativas del SO (Windows Job Objects).

## 1. Concepto

Actualmente, los agentes se ejecutan como procesos hijos directos sin restricciones adicionales.
El sandboxing permitirá:
- **Límites de Recursos**: Restringir memoria máxima y uso de CPU (opcional).
- **Gestión de Ciclo de Vida**: Asegurar que si el daemon muere, todos los procesos hijos mueran automáticamente (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
- **Seguridad**: Restringir acceso a ciertas APIs o recursos del sistema (más avanzado, MVP se enfoca en límites).

## 2. Implementación (`forge-sandbox`)

### 2.1 Estructura
El crate `forge-sandbox` actualmente es un placeholder. Se actualizará para exponer un trait `SandboxedProcess`.

```rust
pub trait SandboxedProcess {
    fn spawn(&self, command: &mut Command) -> Result<Child>;
    // fn set_memory_limit(&self, limit_bytes: u64) -> Result<()>;
}
```

### 2.2 Windows Implementation (`src/windows.rs`)
Usar `windows-sys` o `winapi` para:
1.  `CreateJobObjectW`
2.  `SetInformationJobObject` con `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`
    - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: Crítico para evitar procesos huérfanos.
    - `JOB_OBJECT_LIMIT_PROCESS_MEMORY`: Límite de memoria (e.g., 512MB por agente).
3.  `AssignProcessToJobObject` después de spawnear el proceso (o crear proceso suspendido, asignar, y reanudar).

*Nota*: `tokio::process::Command` no expone fácilmente el `HANDLE` nativo antes de spawnear.
Estrategia: Spawnear el proceso, obtener su `RawHandle` (`as_raw_handle`), y asignarlo al Job inmediatamente. Existe una pequeña ventana de tiempo donde el proceso corre sin sandbox, pero para límites de memoria/ciclo de vida es aceptable en MVP.

## 3. Integración (`forge-daemon`)

En `handlers.rs`, al hacer `start_agent`:
1.  Instanciar un `JobObject` (o equivalente del sandbox) para ese agente.
2.  Spawnear el proceso.
3.  Asignar el proceso al Job Object.
4.  Guardar el handle del Job Object en el `ProcessSupervisor` (o en una estructura paralela) para mantenerlo vivo mientras el agente corre.

## 4. Dependencias Nuevas
- `windows-sys` (features: `Win32_System_JobObjects`, `Win32_System_Threading`, `Win32_Foundation`) en `forge-sandbox`.

## 5. Plan de Implementación

1.  **Dependencias**: Actualizar `forge-sandbox/Cargo.toml`.
2.  **Módulo Windows**: Implementar `JobObject` wrapper en `forge-sandbox/src/windows.rs`.
3.  **Trait Sandbox**: Definir interfaz común en `lib.rs`.
4.  **Integración Daemon**:
    - Modificar `ProcessSupervisor` para almacenar el `JobObject`.
    - Actualizar `start_agent` para usar el sandbox.

## 6. Consideraciones Linux (Futuro)
- En Linux se usaría `cgroups` (v1 o v2). Por ahora se dejará un `impl` vacío o básico que solo hace spawn.
