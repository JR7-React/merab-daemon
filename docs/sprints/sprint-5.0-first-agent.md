# Sprint 5.0 — Primer Agente MCP (Echo)

**Estado**: Completado
**Resultado**: Agente `forge-echo` implementado y verificado con orquestación IA.

## Objetivo
Crear un agente de referencia ("Hello World") que implemente el protocolo MCP (Model Context Protocol) para validar que el Daemon puede conectar herramientas a la IA.

## Parte 1: Crate `forge-echo`

Se creó un nuevo crate binario `crates/forge-echo` que funciona como un servidor MCP stdio.
- **Protocolo**: MCP (JSON-RPC 2.0 sobre Stdin/Stdout).
- **Herramientas**: `echo` (repite el input).

## Parte 2: Registro y Ejecución

Se definió el manifiesto `echo-agent.toml`:
```toml
name = "forge-echo"
version = "0.1.0"
command = "target/debug/forge-echo.exe"
protocol = "mcp"
```

El flujo validado fue:
1.  **Registro**: `forge register echo-agent.toml`
2.  **Inicio**: `forge start <uuid>`
3.  **Uso**: `forge ask "Usa la herramienta echo..."`

## Verificación
La IA (via OpenRouter) invocó correctamente `echo` con argumentos JSON, el daemon ejecutó el proceso hijo, capturó la salida y se la devolvió a la IA para la respuesta final.

## Archivos modificados/creados

| Archivo | Cambio |
|---------|--------|
| `crates/forge-echo/` | **NUEVO**: Crate del agente |
| `echo-agent.toml` | **NUEVO**: Manifiesto de prueba |
| `crates/forge-echo/Cargo.toml` | Configuración de dependencias |
| `crates/forge-echo/src/main.rs` | Implementación del servidor MCP |
