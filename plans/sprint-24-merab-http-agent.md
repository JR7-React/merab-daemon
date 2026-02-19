# Sprint 24 — Agente HTTP (`merab-http`)

## Objetivo

Nuevo agente MCP `merab-http` que expone herramientas para que los LLMs puedan hacer peticiones HTTP: fetch de URLs, POST de JSON, download de archivos. Esto permite que el pipeline acceda a documentación online, APIs externas, y recursos web.

## Problema actual

Los agentes MCP actuales operan solo en el filesystem y sistema local. Si el LLM necesita consultar documentación de una API, verificar un endpoint, o descargar un recurso, no puede hacerlo. Esto limita fuertemente las tareas que puede completar autónomamente.

## Comportamiento objetivo

```bash
# El usuario no interactúa directamente con merab-http.
# El LLM lo usa automáticamente cuando necesita información web.

$ merab ask "implementa un cliente para la API de GitHub, consulta la documentación en https://docs.github.com/en/rest"
[Planner] Descomponiendo tarea...
[CODER] Consultando documentación...   # ← llama a http.fetch
[CODER] Implementando cliente...
[REVIEWER] Revisando código...
✓ Completado

# También puede usarse directamente:
$ merab call <http-agent-id> http.fetch '{"url": "https://api.github.com/users/torvalds"}'
```

## Herramientas expuestas

### `http.fetch`

```json
{
  "name": "http.fetch",
  "description": "Fetch content from a URL via HTTP GET. Returns text content.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "url": {
        "type": "string",
        "description": "The URL to fetch (required)"
      },
      "headers": {
        "type": "object",
        "description": "Optional HTTP headers as key-value pairs"
      },
      "timeout_secs": {
        "type": "integer",
        "description": "Timeout in seconds (default: 30)"
      },
      "max_bytes": {
        "type": "integer",
        "description": "Max response bytes to return (default: 65536 = 64KB)"
      }
    },
    "required": ["url"]
  }
}
```

### `http.post`

```json
{
  "name": "http.post",
  "description": "Send a JSON POST request to a URL.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "url": { "type": "string", "description": "Target URL (required)" },
      "body": { "type": "object", "description": "JSON body to send (required)" },
      "headers": { "type": "object", "description": "Optional HTTP headers" },
      "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default: 30)" }
    },
    "required": ["url", "body"]
  }
}
```

### `http.download`

```json
{
  "name": "http.download",
  "description": "Download a file from a URL and save it to disk.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "url": { "type": "string", "description": "URL to download (required)" },
      "dest": { "type": "string", "description": "Destination file path (required)" }
    },
    "required": ["url", "dest"]
  }
}
```

## Implementación

### `crates/merab-http/` (nuevo crate)

Estructura igual a los otros agentes MCP (merab-fs, merab-shell, merab-git):

```
crates/merab-http/
  Cargo.toml
  src/
    main.rs
```

#### `Cargo.toml`

```toml
[package]
name = "merab-http"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "merab-http"
path = "src/main.rs"

[dependencies]
rmcp = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
reqwest = { version = "0.12", features = ["json"] }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

#### `src/main.rs`

Seguir exactamente el mismo patrón que `merab-fs/src/main.rs`:

1. Definir struct `HttpAgent` que implementa `rmcp::ServerHandler`
2. Implementar `list_tools()` retornando los 3 tools con sus schemas
3. Implementar `call_tool()` con match sobre el nombre del tool
4. `main()` inicializa el servidor MCP sobre stdin/stdout

```rust
use rmcp::{ServerHandler, model::{Tool, CallToolResult, Content}, ...};
use reqwest::Client;
use serde_json::Value;

pub struct HttpAgent {
    client: Client,
}

impl HttpAgent {
    fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    async fn fetch(&self, url: &str, headers: Option<Value>, timeout_secs: u64, max_bytes: usize)
        -> anyhow::Result<String>
    {
        let mut builder = self.client.get(url)
            .timeout(std::time::Duration::from_secs(timeout_secs));

        if let Some(Value::Object(h)) = headers {
            for (k, v) in h {
                if let Some(v_str) = v.as_str() {
                    builder = builder.header(&k, v_str);
                }
            }
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {}: {}", status, &text[..200.min(text.len())]));
        }

        // Truncar si supera max_bytes para no saturar el contexto del LLM
        if text.len() > max_bytes {
            Ok(format!("{}\n\n[Truncado a {} bytes de {}]", &text[..max_bytes], max_bytes, text.len()))
        } else {
            Ok(text)
        }
    }

    async fn post_json(&self, url: &str, body: Value, headers: Option<Value>, timeout_secs: u64)
        -> anyhow::Result<String>
    {
        let mut builder = self.client.post(url)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .json(&body);

        if let Some(Value::Object(h)) = headers {
            for (k, v) in h {
                if let Some(v_str) = v.as_str() {
                    builder = builder.header(&k, v_str);
                }
            }
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {}: {}", status, &text));
        }
        Ok(text)
    }

    async fn download(&self, url: &str, dest: &str) -> anyhow::Result<String> {
        let bytes = self.client.get(url).send().await?.bytes().await?;
        std::fs::write(dest, &bytes)?;
        Ok(format!("Downloaded {} bytes to {}", bytes.len(), dest))
    }
}
```

### `Cargo.toml` (workspace root)

Agregar `merab-http` al array `members`:

```toml
members = [
    ...
    "crates/merab-http",
]
```

### `crates/merab-cli/src/bootstrap.rs`

Agregar `merab-http` a la lista de agentes built-in que se registran durante `init`:

```rust
// En la función que registra agentes built-in, agregar:
AgentManifest {
    name: "merab-http".to_string(),
    description: Some("HTTP client agent — fetch URLs, POST JSON, download files".to_string()),
    command: find_agent_binary("merab-http")?,
    protocol: ProtocolKind::Mcp,
    ..Default::default()
}
```

La función `find_agent_binary` ya existe para los otros agentes — revisar cómo encuentra los binarios y usar el mismo patrón.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `crates/merab-http/Cargo.toml` | Nuevo |
| `crates/merab-http/src/main.rs` | Nuevo — agente MCP con 3 tools |
| `Cargo.toml` (workspace) | Agregar `merab-http` a `members` |
| `crates/merab-cli/src/bootstrap.rs` | Registrar merab-http en init |

## Verificación

```bash
# Compilar el agente:
cargo build -p merab-http

# Verificar que se registra al hacer init:
merab init
merab list   # Debe aparecer merab-http

# Llamar directamente:
AGENT_ID=$(merab list | grep http | awk '{print $1}')
merab call $AGENT_ID http.fetch '{"url": "https://httpbin.org/get"}'
# Debe retornar el JSON de httpbin

# Verificar que el pipeline lo usa automáticamente:
merab ask "¿cuál es el status de https://httpbin.org/status/200 ?"
```

## Notas para el modelo de IA

- **Patrón**: Copiar la estructura de `crates/merab-fs/src/main.rs` o `crates/merab-git/src/main.rs` exactamente. El patrón MCP con `rmcp` es el mismo en todos los agentes.
- **`reqwest`** ya es una dependencia de `merab-ai`. Agregarla localmente al nuevo crate con las mismas features.
- **Seguridad**: El agente puede hacer peticiones HTTP a cualquier URL. Para MVP no filtrar — el LLM decide qué URLs usar.
- **Truncado**: CRÍTICO. Las respuestas HTML pueden ser enormes (MB). El default de `max_bytes: 65536` protege el contexto del LLM. El LLM puede pasar un `max_bytes` mayor si necesita más.
- **Timeout**: Default 30s. Peticiones a APIs lentas pueden bloquearse. El LLM puede pasar un timeout mayor.
- **Headers de auth**: Permitir que el LLM pase `Authorization: Bearer ...` para APIs autenticadas. No hardcodear nada.
- Verificar cómo registra los binarios `bootstrap.rs` — puede usar `std::env::current_exe()` para encontrar la ruta del directorio de binarios.
