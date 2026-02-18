# Sprint 13 — Contexto del Proyecto

## Objetivo

Merab debe entender en qué proyecto está trabajando antes de ejecutar cualquier tarea. El contexto del proyecto (lenguaje, estructura, convenciones) se inyecta automáticamente en los prompts de todos los agentes.

## Problema actual

Hoy, cuando el Coder escribe código, no sabe:
- Qué lenguaje usa el proyecto
- Cómo se llaman los módulos existentes
- Cuáles son las convenciones de nombres
- Qué dependencias ya están instaladas
- Qué tests existen

El LLM adivina desde cero en cada `merab ask`.

## Comportamiento objetivo

```bash
# Al iniciar por primera vez en un proyecto:
$ merab init
[✓] Proyecto detectado: Rust (merab v0.1.0)
[✓] Estructura: 13 crates, 47 archivos .rs
[✓] Dependencias clave: tokio, serde, jsonrpsee, rmcp
[✓] Tests: 56 pasando
[✓] Contexto guardado en shared memory

# En sesiones siguientes, el contexto se carga automáticamente.
# Ahora cuando el Coder recibe una tarea, ya sabe todo esto.
```

## Implementación

### 1. Detector de contexto de proyecto

Archivo: `crates/merab-cli/src/project_context.rs`

```rust
pub struct ProjectContext {
    pub name:         String,
    pub language:     String,       // "rust", "typescript", "python", ...
    pub root_path:    String,
    pub description:  String,       // del README
    pub key_files:    Vec<String>,  // archivos importantes
    pub dependencies: Vec<String>,  // del Cargo.toml / package.json
    pub test_summary: String,       // "56 tests passing"
}

impl ProjectContext {
    pub fn detect(root: &Path) -> Self {
        // 1. Detectar lenguaje por archivos (Cargo.toml, package.json, etc.)
        // 2. Leer README.md (primeros 500 chars)
        // 3. Leer manifiesto de dependencias
        // 4. Listar estructura de src/ (máximo 2 niveles)
    }

    pub fn to_prompt_string(&self) -> String {
        // Genera un bloque de texto para inyectar en system prompts
    }
}
```

### 2. Guardar contexto en shared memory

En `bootstrap::ensure_ready()`, después de iniciar el daemon y los agentes:

```rust
let ctx = ProjectContext::detect(&std::env::current_dir()?);
client.memory_put("project.context", ctx.to_json(), None).await?;
```

### 3. Inyectar en `build_dynamic_system_prompt`

En `ai_methods.rs`, leer el contexto de shared memory e incluirlo en el system prompt de cada agente:

```rust
pub async fn build_dynamic_system_prompt(
    base_prompt: &str,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,      // ← nuevo parámetro
) -> String {
    let mut prompt = base_prompt.to_string();

    // Inyectar contexto del proyecto
    if let Ok(Some(ctx)) = db.lock().await.get_memory("project.context") {
        prompt.push_str("\n\n## Project Context\n");
        prompt.push_str(&ctx.to_string());
    }

    // ... resto del prompt (tools disponibles)
    prompt
}
```

### 4. Nuevo comando `merab context`

Muestra el contexto detectado del proyecto actual:

```bash
$ merab context
Proyecto: merab
Lenguaje: Rust (edition 2024)
Descripción: Runtime local en Rust para orquestar agentes de IA...
Crates: 13
Dependencias clave: tokio 1, serde 1, jsonrpsee 0.24, rmcp 0.15
Tests: 56 passing
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `crates/merab-cli/src/project_context.rs` | Nuevo — `ProjectContext::detect()` |
| `crates/merab-cli/src/bootstrap.rs` | Guardar contexto en shared memory al iniciar |
| `crates/merab-cli/src/main.rs` | Agregar `Commands::Context` |
| `crates/merab-daemon/src/rpc/ai_methods.rs` | Leer contexto en `build_dynamic_system_prompt` |
| `crates/merab-daemon/src/rpc/server.rs` | Pasar `db` a `build_dynamic_system_prompt` |

## Verificación

```bash
# En el directorio de merab:
merab context
# Debe mostrar "Rust", "13 crates", las deps correctas

# En un directorio Node.js:
merab context
# Debe mostrar "TypeScript/JavaScript", deps de package.json
```

## Notas

- Si no se detecta ningún tipo de proyecto conocido, usar contexto genérico.
- El contexto se regenera automáticamente si `Cargo.toml` o `package.json` cambian (por timestamp).
- No incluir el contenido completo de archivos en el contexto — solo metadatos y estructura. Los agentes pueden leer archivos específicos via `fs.read` cuando los necesiten.
