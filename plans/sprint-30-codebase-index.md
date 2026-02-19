# Sprint 30 — Codebase Index Semántico

## Objetivo

Construir y mantener un índice de símbolos del proyecto (funciones, structs, traits, módulos, constantes) almacenado en SQLite. El AI puede consultar el índice para responder "¿qué funciones existen relacionadas con X?" sin necesidad de leer todos los archivos, superando la limitación del contexto window.

## Problema actual

El AI solo conoce lo que cabe en el contexto de cada llamada. En proyectos grandes no puede saber qué existe sin que el usuario le indique archivos específicos. Esto lo hace dependiente del usuario para orientación y limita su autonomía.

```bash
# Hoy: el AI necesita que el usuario diga "mira crates/merab-ai/src/client.rs"
merab ask "agrega retry a la llamada chat()"
# → El AI no sabe dónde está chat(), adivina o pregunta

# Con índice:
merab ask "agrega retry a la llamada chat()"
# → El AI consulta el índice: chat() está en merab-ai/src/client.rs:45
# → Lee solo ese archivo y actúa con precisión
```

## Comportamiento objetivo

```bash
# Construir/actualizar índice del proyecto actual
$ merab index build
Indexando crates/merab-ai/src/client.rs... 12 símbolos
Indexando crates/merab-daemon/src/rpc/server.rs... 38 símbolos
...
Índice listo: 423 símbolos en 31 archivos (0.8s)

# Buscar símbolo
$ merab index search "chat"
merab-ai/src/client.rs:45     fn chat(messages, config) -> AiResponse
merab-ai/src/client.rs:12     struct AiClient
merab-cli/src/chat_ui.rs:27   fn start_chat_session_with_history(...)
merab-cli/src/chat_cmd.rs:8   fn run_chat(client, flags)

# Buscar por tipo
$ merab index search --kind struct "Config"
merab-config/src/lib.rs:10    struct MerabConfig
merab-config/src/lib.rs:42    struct DaemonConfig
merab-config/src/lib.rs:60    struct AiConfig

# El índice se inyecta automáticamente en merab ask
$ merab ask "refactoriza handle_enter para reducir anidamiento"
# → El AI recibe: "Símbolos relevantes: handle_enter @ chat_ui.rs:180"
```

## Implementación

### Nuevo agente MCP: `crates/merab-index/`

Crate nuevo con binario `merab-index` que expone herramienta MCP `index.search`.

**`crates/merab-index/Cargo.toml`**

```toml
[package]
name = "merab-index"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "merab-index"
path = "src/main.rs"

[dependencies]
merab-core = { path = "../merab-core" }
merab-store = { path = "../merab-store" }
merab-transport = { path = "../merab-transport" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
anyhow = "1"
walkdir = "2"
regex = "1"
```

### Parser de símbolos: `crates/merab-index/src/parser.rs`

Usa regex para extraer símbolos de archivos Rust (sin tree-sitter para no añadir dependencias pesadas). Suficientemente preciso para funciones/structs/traits/impl.

```rust
use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Struct,
    Trait,
    Enum,
    Impl,
    Const,
    Type,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "fn"),
            SymbolKind::Struct   => write!(f, "struct"),
            SymbolKind::Trait    => write!(f, "trait"),
            SymbolKind::Enum     => write!(f, "enum"),
            SymbolKind::Impl     => write!(f, "impl"),
            SymbolKind::Const    => write!(f, "const"),
            SymbolKind::Type     => write!(f, "type"),
        }
    }
}

pub fn extract_symbols(file: &Path, content: &str) -> Vec<Symbol> {
    let file_str = file.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    let patterns: &[(&str, SymbolKind)] = &[
        (r"(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)\s*[<(]", SymbolKind::Function),
        (r"(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w+)", SymbolKind::Struct),
        (r"(?:pub(?:\([^)]*\))?\s+)?trait\s+(\w+)", SymbolKind::Trait),
        (r"(?:pub(?:\([^)]*\))?\s+)?enum\s+(\w+)", SymbolKind::Enum),
        (r"impl(?:<[^>]*>)?\s+(?:\w+::)*(\w+)", SymbolKind::Impl),
        (r"(?:pub(?:\([^)]*\))?\s+)?const\s+(\w+)", SymbolKind::Const),
        (r"(?:pub(?:\([^)]*\))?\s+)?type\s+(\w+)", SymbolKind::Type),
    ];

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("*") {
            continue;
        }
        for (pattern, kind) in patterns {
            let re = Regex::new(pattern).unwrap();
            if let Some(cap) = re.captures(trimmed) {
                let name = cap[1].to_string();
                symbols.push(Symbol {
                    name,
                    kind: kind.clone(),
                    file: file_str.clone(),
                    line: line_num + 1,
                    signature: trimmed.chars().take(120).collect(),
                });
                break;
            }
        }
    }

    symbols
}
```

### Store: `crates/merab-store/src/index.rs`

```rust
use rusqlite::params;
use crate::db::Database;
use crate::error::StoreError;

#[derive(Debug, Clone)]
pub struct IndexedSymbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub signature: String,
}

impl Database {
    pub fn index_clear_project(&self, project_path: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "DELETE FROM code_index WHERE project_path = ?1",
            params![project_path],
        )?;
        Ok(())
    }

    pub fn index_insert_symbol(
        &self,
        project_path: &str,
        symbol: &IndexedSymbol,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO code_index (project_path, name, kind, file, line, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![project_path, symbol.name, symbol.kind, symbol.file, symbol.line, symbol.signature],
        )?;
        Ok(())
    }

    pub fn index_search(
        &self,
        project_path: &str,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<IndexedSymbol>, StoreError> {
        let pattern = format!("%{}%", query.to_lowercase());
        let rows = match kind {
            Some(k) => self.conn.prepare(
                "SELECT name, kind, file, line, signature FROM code_index
                 WHERE project_path = ?1 AND LOWER(name) LIKE ?2 AND kind = ?3
                 ORDER BY name LIMIT ?4"
            )?.query_map(params![project_path, pattern, k, limit as i64], row_to_symbol)?,
            None => self.conn.prepare(
                "SELECT name, kind, file, line, signature FROM code_index
                 WHERE project_path = ?1 AND LOWER(name) LIKE ?2
                 ORDER BY name LIMIT ?3"
            )?.query_map(params![project_path, pattern, limit as i64], row_to_symbol)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::from)
    }
}

fn row_to_symbol(row: &rusqlite::Row) -> rusqlite::Result<IndexedSymbol> {
    Ok(IndexedSymbol {
        name: row.get(0)?,
        kind: row.get(1)?,
        file: row.get(2)?,
        line: row.get(3)?,
        signature: row.get(4)?,
    })
}
```

### Migración SQLite: `crates/merab-store/src/db.rs`

Agregar al `migrate()`:

```sql
CREATE TABLE IF NOT EXISTS code_index (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_path TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    file TEXT NOT NULL,
    line INTEGER NOT NULL,
    signature TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_code_index_project_name
    ON code_index(project_path, name);
```

### RPC: nuevos métodos en `merab-daemon`

En `server.rs` (trait):
```rust
#[method(name = "merab.index.build")]
async fn index_build(&self, project_path: String) -> RpcResult<u64>;

#[method(name = "merab.index.search")]
async fn index_search(&self, project_path: String, query: String, kind: Option<String>) -> RpcResult<Vec<IndexedSymbol>>;
```

Implementación en `crates/merab-daemon/src/rpc/index_impls.rs` (nuevo):

```rust
pub async fn index_build(rpc: &MerabRpc, project_path: String) -> RpcResult<u64> {
    use walkdir::WalkDir;
    use crate::parser::extract_symbols;

    let db = rpc.db.lock().await;
    db.index_clear_project(&project_path).map_err(to_rpc_error)?;

    let mut count = 0u64;
    for entry in WalkDir::new(&project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .filter(|e| !e.path().to_string_lossy().contains("/target/"))
    {
        let content = std::fs::read_to_string(entry.path())
            .unwrap_or_default();
        let symbols = extract_symbols(entry.path(), &content);
        for sym in &symbols {
            let indexed = IndexedSymbol {
                name: sym.name.clone(),
                kind: sym.kind.to_string(),
                file: entry.path().to_string_lossy().to_string(),
                line: sym.line,
                signature: sym.signature.clone(),
            };
            db.index_insert_symbol(&project_path, &indexed)
                .map_err(to_rpc_error)?;
        }
        count += symbols.len() as u64;
    }
    Ok(count)
}
```

### CLI: `merab index`

En `main.rs`:
```rust
/// Codebase symbol index
#[command(subcommand_required = true)]
Index {
    #[command(subcommand)]
    cmd: IndexCommands,
},
```

```rust
#[derive(Subcommand)]
pub enum IndexCommands {
    /// Build or rebuild the symbol index
    Build,
    /// Search symbols by name
    Search {
        query: String,
        #[arg(long, short)]
        kind: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
    },
}
```

Despacho en `index_cmd.rs` (nuevo):

```rust
pub async fn run_build(client: &MerabClient, project_path: &str) -> anyhow::Result<()> {
    println!("Indexando proyecto...");
    let count = client.index_build(project_path.to_string()).await?;
    println!("Índice listo: {} símbolos", count);
    Ok(())
}

pub async fn run_search(client: &MerabClient, project_path: &str, query: &str, kind: Option<String>, limit: usize) -> anyhow::Result<()> {
    let symbols = client.index_search(project_path.to_string(), query.to_string(), kind).await?;
    if symbols.is_empty() {
        println!("Sin resultados para '{}'", query);
        return Ok(());
    }
    for s in symbols.iter().take(limit) {
        println!("{:<8} {}:{}\n         {}", s.kind, s.file, s.line, s.signature);
    }
    Ok(())
}
```

### Inyección automática en `merab ask`

En `ai_methods.rs`, antes de `build_dynamic_system_prompt()`:

```rust
// Consultar índice con palabras clave del task
let keywords = extract_keywords(&task); // split por palabras significativas
let mut index_context = String::new();
for kw in keywords.iter().take(5) {
    if let Ok(symbols) = db.index_search(&project_path, kw, None, 5) {
        for s in symbols {
            index_context.push_str(&format!("  {} {} @ {}:{}\n",
                s.kind, s.name, s.file, s.line));
        }
    }
}
if !index_context.is_empty() {
    system_prompt.push_str(&format!("\n## Símbolos relevantes del proyecto\n{}", index_context));
}
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-store/src/db.rs` | Migración tabla `code_index` |
| `merab-store/src/index.rs` | Nuevo — métodos CRUD del índice |
| `merab-store/src/lib.rs` | Exportar `IndexedSymbol` |
| `merab-daemon/src/rpc/index_impls.rs` | Nuevo — `index_build`, `index_search` |
| `merab-daemon/src/rpc/server.rs` | Declarar métodos RPC nuevos |
| `merab-daemon/src/rpc/mod.rs` | Agregar `pub mod index_impls` |
| `merab-daemon/Cargo.toml` | Agregar `walkdir = "2"`, `regex = "1"` |
| `merab-cli/src/index_cmd.rs` | Nuevo — `run_build`, `run_search` |
| `merab-cli/src/main.rs` | Agregar `Index` variant + `mod index_cmd` |
| `merab-cli/src/client.rs` | Métodos `index_build`, `index_search` |
| `merab-daemon/src/rpc/ai_methods.rs` | Inyectar símbolos relevantes en prompt |

## Verificación

```bash
# Construir índice
merab index build
# → "Índice listo: NNN símbolos en NN archivos"

# Buscar función
merab index search "chat"
# → Debe mostrar chat() en client.rs y start_chat_session en chat_ui.rs

# Buscar solo structs
merab index search --kind struct "Config"
# → MerabConfig, DaemonConfig, AiConfig

# Verificar inyección en ask
merab ask "¿dónde está implementado el rate limit?"
# → El AI debe mencionar merab-ai/src/retry.rs sin que el usuario lo diga
```

## Notas para el modelo de IA

- **No usar tree-sitter**: añade complejidad de compilación innecesaria. Regex cubre el 90% de casos de Rust bien formateado.
- **Excluir `/target/`** siempre en el walker.
- **Limpiar el índice antes de rebuild**: `index_clear_project` antes de insertar, evitar duplicados.
- **`extract_keywords`** para inyección automática: simplemente `task.split_whitespace().filter(|w| w.len() > 4)`. No necesita NLP.
- **Los métodos RPC de índice** siguen el patrón de `index_impls.rs` — funciones libres que toman `&MerabRpc`.
- **Dificultad**: Media (~300 líneas nuevas total). El parser regex es la parte más delicada.
