# Sprint 6.0 — FileSystem Agent (forge-fs)

**Estado**: Completado
**Resultado**: Agente `forge-fs` implementado y capaz de leer, listar y escribir archivos.

## Objetivo
Dotar a la IA de capacidades para interactuar con el sistema de archivos local, permitiendo que orqueste tareas de auditoría, lectura de código y (en el futuro) codificación autónoma.

## Parte 1: Crate `forge-fs`

Se creó un nuevo crate binario `crates/forge-fs` que funciona como un servidor MCP stdio.
- **Tools**:
    - `fs.list(path, recursive)`: Exploración de estructura de directorios.
    - `fs.read(path)`: Lectura de contenido de archivos.
    - `fs.write(path, content)`: Escritura de archivos (crea directorios padres automáticamente).
    - `fs.search(base_path, pattern)`: Búsqueda usando globs (e.g. `**/*.rs`).

## Parte 2: Registro y Uso

Se definió el manifiesto `fs-agent.toml`:
```toml
name = "forge-fs"
version = "0.1.0"
description = "FileSystem Agent (list, read, write, search)"
command = "target/debug/forge-fs.exe"
protocol = "mcp"
```

El flujo validado fue:
1.  **Registro**: `forge register fs-agent.toml`
2.  **Inicio**: `forge start <uuid>`
3.  **Chat/Orquestación**: La IA usó `fs.read` para inspeccionar `crates/forge-fs/Cargo.toml` y listar sus dependencias.

## Seguridad
El agente opera relativo al directorio de trabajo actual (CWD) donde se lanza el proceso hijo. Se implementó una resolución básica de paths (`resolve_path`) para evitar escapes simples del root, aunque se recomienda usar sandboxing a nivel de SO en versiones futuras.

## Archivos modificados/creados

| Archivo | Cambio |
|---------|--------|
| `crates/forge-fs/` | **NUEVO**: Crate del agente FS |
| `fs-agent.toml` | **NUEVO**: Manifiesto de prueba |
| `crates/forge-fs/Cargo.toml` | Configuración de dependencias (`walkdir`, `glob`) |
| `crates/forge-fs/src/main.rs` | Implementación del servidor MCP y herramientas FS |
