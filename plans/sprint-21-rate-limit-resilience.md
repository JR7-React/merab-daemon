# Sprint 21 — Rate Limit Resilience

## Objetivo

Merab maneja automáticamente los errores de rate limit (HTTP 429) de la API de Anthropic con retry exponencial, sin que el usuario tenga que intervenir. Especialmente importante con tareas paralelas (Sprint 14) y múltiples API keys (Sprint 16).

## Problema actual

Si un subtask recibe un 429, falla inmediatamente y el pipeline entero se cancela. El usuario ve un error críptico.

## Comportamiento objetivo

```bash
$ merab ask "tarea con 6 subtasks en paralelo"

[coder] Iniciando...
[coder] Rate limit alcanzado, reintentando en 15s...   ← transparente para el usuario
[coder] Reintentando (1/3)...
[coder] ✓ Completado

# El usuario solo ve el delay, no un error
```

Con múltiples keys (Sprint 16), antes de reintentar con backoff, intentar con otra key disponible:
```
[coder] Rate limit en key-1, rotando a key-2...
[coder] ✓ Completado sin espera
```

## Implementación

### `crates/merab-ai/src/retry.rs` (nuevo)
```rust
pub struct RetryConfig {
    pub max_attempts: u32,        // default: 3
    pub base_delay_ms: u64,       // default: 1000
    pub max_delay_ms: u64,        // default: 60_000
    pub backoff_multiplier: f64,  // default: 2.0
}

/// Ejecuta una función async con retry exponencial en 429/503.
pub async fn with_retry<F, T>(
    config: &RetryConfig,
    f: F,
) -> Result<T, AiError>
where
    F: Fn() -> Future<Output = Result<T, AiError>>
```

Delays: 1s → 2s → 4s → 8s... con jitter aleatorio ±20% para evitar thundering herd.

### `crates/merab-ai/src/client.rs`
Envolver `chat()` con `with_retry()`. Detectar errores 429 y 503 como retriables; el resto propagar inmediatamente.

### `crates/merab-config/src/lib.rs`
```rust
pub struct AiConfig {
    // ...campos actuales...
    pub max_retry_attempts: u32,   // default: 3
    pub retry_base_delay_ms: u64,  // default: 1000
}
```

### `crates/merab-daemon/src/dag.rs`
Si hay múltiples personas con API keys distintas (Sprint 16) y una da 429, intentar con la key de otra persona disponible antes del backoff.

```rust
/// Key rotation — intentar con otra key antes del backoff.
async fn try_with_key_rotation(
    personas: &[(&str, &str)],  // (persona, api_key)
    task: &SubTask,
) -> Result<SubtaskResult>
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-ai/src/retry.rs` | Nuevo — retry con backoff exponencial |
| `merab-ai/src/client.rs` | Usar retry en `chat()` |
| `merab-config/src/lib.rs` | Config de retry |
| `merab-daemon/src/dag.rs` | Key rotation antes de backoff (requiere Sprint 16) |

## Verificación

```bash
# Test manual: poner una key inválida y verificar que reintenta
# Test de integración: mockear 429 en el proxy y verificar backoff

# En merab-daemon/tests/ agregar:
# test_retry_on_429()  — verifica que reintenta hasta max_attempts
# test_no_retry_on_400() — verifica que errores de cliente no se reintentan
```

## Notas

- Solo reintentar en: 429 (rate limit), 503 (service unavailable), timeout de red.
- NO reintentar en: 400, 401, 403, 404 — son errores del cliente, reintentar no ayuda.
- Loggear cada retry con `tracing::warn` para visibilidad.
- Sprint independiente de Sprint 16 — funciona con una sola key también.
