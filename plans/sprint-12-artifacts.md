# Sprint 12 — Rastreo de Artefactos

## Objetivo

Al terminar un `merab ask`, mostrar un resumen concreto de **qué cambió en el sistema**: qué archivos se crearon/modificaron, qué comandos corrieron y cuál fue su resultado. El resultado de Merab no es solo texto — es trabajo real.

## Problema actual

Cuando `fs-agent` escribe un archivo o `shell-agent` corre `cargo test`, ese resultado llega al orquestador como un string de texto. No hay registro estructurado de qué pasó. El usuario recibe una respuesta de texto que *describe* lo que se hizo, pero no hay evidencia verificable.

## Comportamiento objetivo

```
$ merab ask "añade validación de email al registro de usuarios"

[Planner] Descomponiendo tarea...
[Architect] Analizando código existente...
[Coder] Implementando validación...
[QA] Corriendo tests...

─────────────────────────────────────────
 Resumen de cambios
─────────────────────────────────────────
 Creados:
   + src/validation.rs

 Modificados:
   ~ src/user.rs
   ~ src/handlers/register.rs

 Comandos ejecutados:
   cargo test    → 31 passed, 0 failed ✓
   cargo clippy  → no warnings ✓

 Tiempo total: 42s
─────────────────────────────────────────
```

## Implementación

### 1. Tipos nuevos en `merab-core`

Archivo: `crates/merab-core/src/artifact.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ArtifactLog {
    pub files_created:  Vec<String>,
    pub files_modified: Vec<String>,
    pub commands:       Vec<CommandResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub command:   String,
    pub exit_code: i32,
    pub stdout:    String,
    pub success:   bool,
}
```

### 2. `ArtifactTracker` en el daemon

Archivo: `crates/merab-daemon/src/artifacts.rs`

Un wrapper alrededor de `McpManager` que intercepta los resultados de `fs-agent` y `shell-agent` y los registra en un `ArtifactLog`.

```rust
pub struct ArtifactTracker {
    log: Arc<Mutex<ArtifactLog>>,
}

impl ArtifactTracker {
    pub fn record_tool_result(&self, tool_name: &str, args: &Value, result: &Value) {
        match tool_name {
            "fs.write"      => self.record_file_write(args),
            "fs.read"       => {} // no registrar lecturas
            "shell.execute" => self.record_command(args, result),
            _               => {}
        }
    }

    pub fn finish(self) -> ArtifactLog { ... }
}
```

### 3. Inyectar tracker en `handle_ai_execute_plan`

`execute_tool_call` recibe un `ArtifactTracker` opcional y lo alimenta con cada resultado de tool.

### 4. Nuevo campo en `AiResponse`

```rust
pub struct AiResponse {
    pub content:   String,
    pub tool_call: Option<ToolCall>,
    pub artifacts: Option<ArtifactLog>,  // ← nuevo
}
```

### 5. CLI formatea el log

En `Commands::Ask`, después de recibir la respuesta, si hay `artifacts`, renderiza el resumen con colores.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `crates/merab-core/src/artifact.rs` | Nuevo — tipos `ArtifactLog`, `CommandResult` |
| `crates/merab-core/src/lib.rs` | Exportar `artifact` |
| `crates/merab-daemon/src/artifacts.rs` | Nuevo — `ArtifactTracker` |
| `crates/merab-daemon/src/rpc/ai_methods.rs` | Inyectar tracker en execute_plan |
| `crates/merab-ai/src/lib.rs` | Agregar `artifacts` a `AiResponse` |
| `crates/merab-cli/src/main.rs` | Renderizar resumen al final |

## Verificación

El test debe verificar que después de una ejecución que llama a `fs.write`, el `ArtifactLog` en la respuesta contiene la ruta del archivo escrito.
