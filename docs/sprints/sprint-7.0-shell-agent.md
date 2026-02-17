# Sprint 7.0 — Shell Agent (forge-shell)

**Estado**: Completado
**Resultado**: Agente `forge-shell` implementado. Capacidad de ejecución de comandos lograda.

## Objetivo
Dotar a la IA de "manos" para ejecutar comandos del sistema operativo, permitiendo operaciones como compilación, testing, y gestión de versiones.

## Parte 1: Crate `forge-shell`

Se creó un nuevo crate binario `crates/forge-shell` que funciona como un servidor MCP stdio.
- **Tools**:
    - `shell.execute(command, args, cwd)`: Ejecuta un binario del sistema de forma asíncrona y captura `stdout`, `stderr` y `exit_code`.

## Parte 2: Registro y Uso

Se definió el manifiesto `shell-agent.toml`:
```toml
name = "forge-shell"
version = "0.1.0"
command = "target/debug/forge-shell.exe"
protocol = "mcp"
```

El flujo validado fue:
1.  **Registro**: `forge register shell-agent.toml`
2.  **Inicio**: `forge start <uuid>`
3.  **Chat/Orquestación**: La IA usó `shell.execute` para correr `cmd /c echo Hello Shell` y confirmar la salida.

## Seguridad
Actualmente el agente tiene permisos completos del usuario que ejecuta el daemon.
**Futuro**: Implementar listas blancas de comandos permitidos o ejecutar en un sandbox real.

## Archivos modificados/creados

| Archivo | Cambio |
|---------|--------|
| `crates/forge-shell/` | **NUEVO**: Crate del agente Shell |
| `shell-agent.toml` | **NUEVO**: Manifiesto de prueba |
| `crates/forge-shell/Cargo.toml` | Configuración de dependencias |
| `crates/forge-shell/src/main.rs` | Implementación de `tool_execute` |
