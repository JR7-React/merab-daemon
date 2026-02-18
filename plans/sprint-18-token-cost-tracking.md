# Sprint 18 — Token y Cost Tracking

## Objetivo

Merab registra cuántos tokens consume cada sesión y cuánto cuesta. El usuario puede ver el gasto acumulado por proyecto y por persona.

## Problema actual

No hay visibilidad sobre el consumo de tokens ni el costo. Con múltiples subtasks en paralelo (Sprint 14) y múltiples API keys (Sprint 16), saber cuánto cuesta cada tarea es crítico.

## Comportamiento objetivo

```bash
$ merab ask "implementa el módulo de pagos"
# ... trabaja ...
# ✓ Completado
# Tokens: 12,450 input / 3,210 output — ~$0.08

$ merab sessions
# ID     FECHA       ESTADO     TOKENS     COSTO    TAREA
# a3f9   2026-02-18  completada  15,660    $0.08    implementa pagos
# b7c2   2026-02-17  completada   8,230    $0.04    refactoriza users

$ merab stats
# Proyecto: merab
# Total sesiones: 12
# Total tokens: 187,400
# Costo estimado: $0.94
# Persona más costosa: coder (45% del total)
```

## Implementación

### `crates/merab-core/src/session.rs`
```rust
pub struct Session {
    // ...campos actuales...
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cost_usd: f64,          // calculado al guardar
}

pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub model: String,
}
```

### `crates/merab-core/src/pricing.rs` (nuevo)
```rust
/// Calcula costo en USD para un modelo dado.
pub fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    // Tabla de precios por modelo (actualizar manualmente)
    // claude-sonnet-4-6: $3/$15 per 1M tokens
    // claude-opus-4-6:  $15/$75 per 1M tokens
    // ...
}
```

### `crates/merab-ai/src/types.rs`
```rust
pub struct AiResponse {
    // ...campos actuales...
    pub usage: Option<TokenUsage>,  // ← nueva, viene de la API de Anthropic
}
```

### `crates/merab-ai/src/client.rs`
Parsear `usage.input_tokens` y `usage.output_tokens` de la respuesta de Anthropic y llenar `AiResponse.usage`.

### `crates/merab-daemon/src/dag.rs`
Acumular `TokenUsage` de cada subtask. Devolver total en `ExecutionResult`.

### `crates/merab-store/src/db.rs`
Agregar `tokens_input`, `tokens_output`, `cost_usd` a la tabla `sessions`.

### `crates/merab-cli/src/main.rs`
- `Commands::Ask`: mostrar tokens y costo al terminar
- `Commands::Sessions`: columnas TOKENS y COSTO
- `Commands::Stats` (nuevo): resumen agregado del proyecto

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-core/src/session.rs` | Campos `tokens_input`, `tokens_output`, `cost_usd` |
| `merab-core/src/pricing.rs` | Nuevo — tabla de precios por modelo |
| `merab-ai/src/types.rs` | `usage: Option<TokenUsage>` en `AiResponse` |
| `merab-ai/src/client.rs` | Parsear usage de respuesta Anthropic |
| `merab-daemon/src/dag.rs` | Acumular usage por subtask |
| `merab-store/src/db.rs` | Nuevas columnas en sessions |
| `merab-cli/src/main.rs` | Mostrar tokens/costo, comando `stats` |

## Verificación

```bash
merab ask "escribe un hello world"
# Debe mostrar: Tokens: ~50 input / ~20 output — ~$0.001

merab stats
# Debe mostrar resumen acumulado del proyecto
```

## Notas

- Los precios son estimaciones — la API de Anthropic da el costo real en el response header `anthropic-cost-tokens`.
- Actualizar la tabla de precios en `pricing.rs` cuando cambien los precios oficiales.
- No calcular costo si el modelo no está en la tabla — mostrar "N/A".
