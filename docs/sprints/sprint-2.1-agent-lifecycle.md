# Sprint 2.1 — Agent Lifecycle Real

**Estado**: Completado
**Commit**: `a12543a`

## Objetivo
Monitoreo real de procesos de agentes, restart policies, y supervisor que detecta cuando un proceso muere.

## Archivos clave

### `crates/forge-daemon/src/supervisor.rs`

`ProcessSupervisor` — gestiona procesos child de agentes:

```rust
pub struct ProcessSupervisor {
    handles: Arc<Mutex<HashMap<AgentId, ProcessHandle>>>,
    registry: Arc<AgentRegistry>,
    db: Arc<Mutex<Database>>,
}
```

**Métodos:**
- `start_monitoring(agent_id, child, manifest)` — Spawns un tokio task que espera `child.wait()`. Cuando el proceso termina, actualiza registry + DB. Si `restart_on_failure` está activo, relanza.
- `stop_agent(agent_id) -> bool` — Envía señal de shutdown, espera hasta 5s, mata el proceso.
- `shutdown_all()` — Para todos los agentes supervisados (usado en daemon exit).

### `crates/forge-daemon/src/process.rs`

`is_process_alive(pid: u32) -> bool` — Verifica si un proceso está vivo (Windows: `OpenProcess`, Unix: `kill(0)`).

### `crates/forge-daemon/src/main.rs` — Reconciliación

Al arrancar el daemon, `reconcile_stale_agents()` revisa agentes marcados como "Running" en DB. Si su proceso ya no existe, los marca como Failed. Esto maneja el caso de daemon restart donde agentes murieron.

## Flujo de start_agent

1. Handler crea `tokio::process::Command` desde manifest
2. Spawn el child process
3. Actualiza registry + DB con status Running y PID
4. Pasa el `Child` al supervisor via `start_monitoring()`
5. Supervisor espera en background a que el proceso termine

## Flujo de stop_agent

1. Handler llama `supervisor.stop_agent(uuid)`
2. Supervisor envía señal de shutdown via oneshot channel
3. Monitor task hace `child.kill()` + `child.wait()`
4. Si supervisor no tiene el agente (fallback), usa `taskkill /F` (Windows) o `SIGTERM` (Unix)
5. Actualiza registry + DB con status Stopped

## Restart on Failure

Si un agente tiene `restart_on_failure = true` en su manifest y termina con status Failed:
1. El monitor task espera 1 segundo
2. Re-spawn el proceso con la misma configuración
3. Registra nuevo monitoring

## Dependencias agregadas

```toml
# forge-daemon
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_System_Threading", "Win32_Foundation"] }

[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

## Registry methods agregados

- `update_status(id, status, pid)` — Actualiza status y pid
- `set_exit_info(id, exit_code)` — Guarda exit code y stopped_at
- `agents_mut()` — Acceso directo al HashMap (sync, para reconciliación al startup)

## Store methods agregados

- `update_agent_status(id, status, pid)`
- `update_agent_exit(id, status, exit_code)`
- `list_running_agents() -> Vec<AgentRecord>`
