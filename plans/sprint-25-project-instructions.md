# Sprint 25 — Instrucciones de Proyecto (`merab.md`)

## Objetivo

Merab lee automáticamente un archivo `merab.md` (o `AGENTS.md`) en la raíz del proyecto y lo inyecta como contexto en cada sesión de IA. Esto permite que el LLM conozca las convenciones, restricciones, y stack técnico del proyecto sin que el usuario tenga que especificarlo en cada tarea.

## Problema actual

El sistema de contexto (Sprint 13) detecta el lenguaje y stack del proyecto automáticamente, pero no hay forma de que el usuario deje instrucciones permanentes para el agente. Cada `merab ask` empieza desde cero sin conocer las reglas del proyecto.

## Comportamiento objetivo

```bash
# El usuario crea un archivo merab.md en su proyecto:
$ cat merab.md
# Proyecto: API Backend de Pagos

## Stack
- Rust 2024 edition
- PostgreSQL con SQLx
- Axum para HTTP

## Convenciones
- Sin `unwrap()` en código de producción
- Tests obligatorios para toda función pública
- Commits en español con formato: feat/fix/refactor(scope): mensaje

## Reglas
- Nunca escribir SQL raw — siempre usar SQLx macros
- Los endpoints deben validar input con el tipo correcto de Axum extractor

# Ahora al usar merab ask, el LLM conoce estas reglas:
$ merab ask "implementa endpoint POST /payments"
[Planner] Descomponiendo tarea...
[CODER] Implementando con Axum y SQLx...  # ← conoce el stack automáticamente
✓ Completado

# Ver qué instrucciones está usando merab:
$ merab context
Proyecto detectado: payments-api (Rust)
Instrucciones de proyecto: merab.md (847 chars)
---
# Proyecto: API Backend de Pagos
...
```

## Implementación

### Convención de nombres de archivo (orden de precedencia)

Merab busca el primer archivo que encuentre en la raíz del proyecto CWD:

1. `merab.md`
2. `AGENTS.md`
3. `.merab/instructions.md`

Si no existe ninguno, continúa sin instrucciones (comportamiento actual).

### `crates/merab-core/src/project_instructions.rs` (nuevo)

```rust
use std::path::{Path, PathBuf};

const CANDIDATES: &[&str] = &["merab.md", "AGENTS.md", ".merab/instructions.md"];

/// Busca y lee el archivo de instrucciones del proyecto.
pub struct ProjectInstructions {
    pub path: PathBuf,
    pub content: String,
}

impl ProjectInstructions {
    /// Busca el primer archivo de instrucciones en la ruta dada.
    pub fn load(project_path: &Path) -> Option<Self> {
        for candidate in CANDIDATES {
            let path = project_path.join(candidate);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if !content.trim().is_empty() {
                        return Some(Self { path, content });
                    }
                }
            }
        }
        None
    }
}
```

Exportar desde `merab-core/src/lib.rs`:

```rust
pub mod project_instructions;
pub use project_instructions::ProjectInstructions;
```

### `crates/merab-cli/src/project_context.rs`

Modificar `ProjectContext::detect()` para incluir las instrucciones:

```rust
pub struct ProjectContext {
    // ...campos existentes...
    pub instructions: Option<String>,
    pub instructions_path: Option<PathBuf>,
}

impl ProjectContext {
    pub fn detect(cwd: &Path) -> Self {
        let mut ctx = /* detección existente */;

        // Cargar instrucciones de proyecto
        if let Some(instr) = merab_core::ProjectInstructions::load(cwd) {
            ctx.instructions = Some(instr.content);
            ctx.instructions_path = Some(instr.path);
        }
        ctx
    }
}
```

`ProjectContext::display()` debe mostrar el path y un preview de las instrucciones cuando existan.

### `crates/merab-cli/src/bootstrap.rs`

Al llamar `ensure_ready`, almacenar las instrucciones de proyecto en la memoria compartida del daemon:

```rust
pub async fn ensure_ready(url: &str) -> Result<MerabClient> {
    // ...código existente de bootstrap...

    // Almacenar instrucciones de proyecto en shared memory
    let cwd = std::env::current_dir()?;
    if let Some(instr) = merab_core::ProjectInstructions::load(&cwd) {
        let _ = client.memory_put(
            "project.instructions",
            serde_json::Value::String(instr.content),
            None,  // sin TTL — persiste hasta que se sobreescriba
        ).await;
    }

    Ok(client)
}
```

### `crates/merab-daemon/src/rpc/ai_methods.rs`

`build_dynamic_system_prompt` ya lee `project.context` de la memoria compartida. Agregar lectura de `project.instructions`:

```rust
pub async fn build_dynamic_system_prompt(
    base_prompt: &str,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
) -> String {
    let mut prompt = base_prompt.to_string();

    {
        let store = db.lock().await;

        // Contexto de proyecto (existente)
        if let Ok(Some(serde_json::Value::String(ctx))) = store.get_memory("project.context") {
            prompt.push_str("\n\n## Project Context\n");
            prompt.push_str(&ctx);
        }

        // Instrucciones de proyecto (nuevo)
        if let Ok(Some(serde_json::Value::String(instr))) = store.get_memory("project.instructions") {
            prompt.push_str("\n\n## Project Instructions\n");
            prompt.push_str("The following instructions are set by the project owner and MUST be followed:\n\n");
            prompt.push_str(&instr);
        }
    }

    // ...resto de la función (tools)...
}
```

### `merab.md` de ejemplo (en la raíz de merab/ mismo)

Crear el archivo `merab.md` en el repo de Merab como ejemplo funcional y documentación viva:

```markdown
# Merab — Runtime de Agentes IA

## Stack
- Rust 2024 edition, workspace con 13+ crates
- RPC via jsonrpsee (HTTP JSON-RPC)
- SQLite via rusqlite (merab-store)
- Agentes MCP via rmcp

## Convenciones de código
- Sin `unwrap()` en código de producción — usar `?` y `anyhow`
- `tracing` para logs (no `println!` ni `eprintln!`)
- `anyhow` en binarios, `thiserror` en librerías
- Tests en el mismo archivo o en `tests/`

## Estructura de crates
- `merab-core`: tipos compartidos y lógica de negocio sin dependencias pesadas
- `merab-daemon`: servidor RPC + proxy + orquestación IA
- `merab-cli`: cliente de línea de comandos
- `merab-store`: persistencia SQLite
- `merab-ai`: cliente HTTP para LLMs
- `merab-config`: carga de configuración
- `merab-fs/shell/git/echo/http`: agentes MCP built-in

## Commits
- Formato: tipo(scope): descripción en español
- Co-authored: añadir Co-Authored-By cuando asista IA
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-core/src/project_instructions.rs` | Nuevo — búsqueda de merab.md |
| `merab-core/src/lib.rs` | Exportar `ProjectInstructions` |
| `merab-cli/src/project_context.rs` | Incluir instrucciones en `ProjectContext` |
| `merab-cli/src/bootstrap.rs` | Almacenar instrucciones en shared memory |
| `merab-daemon/src/rpc/ai_methods.rs` | Inyectar instrucciones en el system prompt |
| `merab.md` (raíz del repo) | Nuevo — instrucciones del propio proyecto Merab |

## Verificación

```bash
# Crear archivo de instrucciones:
echo "# Test\n## Reglas\n- Siempre usar snake_case" > merab.md

merab context
# Debe mostrar "Instrucciones de proyecto: merab.md"

merab ask "crea una función que sume dos números"
# El código generado debe usar snake_case (la regla del archivo)

# Sin merab.md:
rm merab.md
merab ask "crea una función sum"
# Debe funcionar igual pero sin las instrucciones
```

## Notas para el modelo de IA

- **Orden de prioridad**: `merab.md` > `AGENTS.md` > `.merab/instructions.md`. Usar el primero que exista.
- **La inyección ocurre en `build_dynamic_system_prompt`** que ya existe en `ai_methods.rs`. Solo agregar el bloque de lectura de `project.instructions`.
- **No sobrescribir instrucciones previas**: si `ensure_ready` se llama múltiples veces, el `memory_put` sobreescribe el valor. Esto es correcto — siempre refleja el estado actual del archivo.
- **Límite de tamaño**: Si las instrucciones superan ~4000 caracteres, truncar con un warning. Instrucciones muy largas consumen contexto valioso.
- **TTL = None**: Las instrucciones no expiran — persisten en la DB del daemon hasta que se actualicen o el daemon se reinicie.
- El archivo `merab.md` del repo de Merab sirve doble propósito: documenta el proyecto Y sirve como demostración del feature.
