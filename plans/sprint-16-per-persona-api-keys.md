# Sprint 16 — API Keys por Persona

## Objetivo

Cada persona (engineer, coder, reviewer, qa) puede usar una API key distinta de Anthropic. Distribuye rate limits y permite separar costos por rol.

## Problema actual

Todos los subtasks del DAG comparten una sola `ANTHROPIC_API_KEY`. Con 4 subtasks en paralelo, los rate limits se alcanzan rápido.

## Comportamiento objetivo

```toml
# merab.toml
[ai]
model = "claude-sonnet-4-6"
api_key = "sk-ant-GLOBAL"     # fallback si la persona no tiene key propia

[ai.personas.engineer]
model = "claude-sonnet-4-6"
api_key = "sk-ant-KEY-1"

[ai.personas.coder]
model = "claude-sonnet-4-6"
api_key = "sk-ant-KEY-2"

[ai.personas.reviewer]
model = "claude-opus-4-6"
api_key = "sk-ant-KEY-3"
```

Si una persona no tiene `api_key` configurada, usa la global de `[ai]`.

## Implementación

### `crates/merab-config/src/lib.rs`
```rust
pub struct PersonaModelConfig {
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

pub struct AiConfig {
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,   // ← nueva, opcional (fallback a env var)
    // ...resto igual
}
```

### `crates/merab-ai/src/types.rs`
```rust
pub struct AiClientConfig {
    pub proxy_url: String,
    pub model: String,
    pub api_key: Option<String>,   // ← nueva, None = usar env var
    // ...resto igual
}
```

### `crates/merab-ai/src/client.rs`
En `chat()`, si `config.api_key.is_some()`, sobreescribir el header `x-api-key` antes de la llamada al proxy.

### `crates/merab-daemon/src/rpc/ai_methods.rs`
`build_ai_client()` lee `config.ai.api_key` y lo pasa a `AiClientConfig`.

### `crates/merab-daemon/src/dag.rs`
`execute_subtask()` lee `config.ai.personas[persona].api_key` y lo usa al construir el cliente. Fallback a `config.ai.api_key`.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-config/src/lib.rs` | `api_key: Option<String>` en `AiConfig` y `PersonaModelConfig` |
| `merab-ai/src/types.rs` | `api_key: Option<String>` en `AiClientConfig` |
| `merab-ai/src/client.rs` | Usar api_key del config si está presente |
| `merab-daemon/src/rpc/ai_methods.rs` | Pasar api_key al construir cliente |
| `merab-daemon/src/dag.rs` | Per-persona api_key en execute_subtask |

## Verificación

```bash
# Configurar 2 keys diferentes en merab.toml
merab ask "crea un módulo con tests"
# Verificar en logs del daemon que engineer y coder usan keys distintas
# No debe haber rate limit errors con tareas paralelas grandes
```

## Notas

- Nunca loggear la API key completa — solo los últimos 4 caracteres para debug.
- Las keys en merab.toml tienen precedencia sobre variables de entorno.
- Si `api_key` es `None` en config, el proxy usa la env var `ANTHROPIC_API_KEY` como siempre.
