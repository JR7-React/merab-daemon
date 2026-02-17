# Forge Engineering Standards & Rules

Este documento define los estándares de ingeniería para el proyecto Forge. El objetivo es mantener una base de código de **Grado Industrial**, escalable, mantenible y segura.

## 1. Estructura y Tamaño del Código

### Archivos
*   **Límite Suave**: 400 líneas. Al llegar aquí, considera refactorizar.
*   **Límite Duro**: 800 líneas. **Obligatorio** dividir el archivo en submódulos.
*   **Cohesión**: Un archivo debe contener una sola abstracción principal o un grupo de funciones estrechamente relacionadas (e.g., `memory.rs` solo maneja operaciones de memoria, no lógica de red).

### Funciones
*   **Límite**: Máximo 50-60 líneas por función.
*   **Complejidad Ciclomática**: Evita anidamiento profundo (más de 3 niveles de `if`/`loop`). Usa "Guard Clauses" (retorno temprano).

## 2. Robustez y Seguridad (Panic Free)

### Unwrap y Expect
*   🚫 **PROHIBIDO** usar `.unwrap()` o `.expect()` en código de producción (`forge-daemon`, librerías core).
    *   *Excepción*: Tests unitarios (`#[test]`) y prototipos rápidos (`examples/`).
*   ✅ **SIEMPRE** propagar errores usando `Result<T, E>` y el operador `?`.
*   ✅ Usa `anyhow::Result` para aplicaciones (daemon/cli) y `thiserror` para librerías (`forge-core`).

### Límites de Recursos
*   Nunca leas archivos enteros en memoria sin un límite (`take(limit)`).
*   Nunca aceptes conexiones o requests ilimitados (usa semáforos o límites de configuración).

## 3. Observabilidad

### Logging
*   Usa el crate `tracing` (no `log` ni `println!`).
*   **Niveles**:
    *   `ERROR`: El sistema falló y requiere atención humana inmediata o perdió datos.
    *   `WARN`: Algo inesperado pasó pero el sistema se recuperó (ej. retry).
    *   `INFO`: Hitos importantes del ciclo de vida (inicio, parada, conexión exitosa).
    *   `DEBUG`: Información útil para diagnosticar flujo (ej. "mensaje recibido", "cache miss").
    *   `TRACE`: Payload completo, dumps de estructuras.
*   **Contexto**: Usa `tracing::span!` o argumentos estructurados.
    *   ❌ Mal: `info!("Agent {} started", id);`
    *   ✅ Bien: `info!(agent.id = %id, "agent started");`

## 4. Estilo y Calidad (Rust)

*   **Rustfmt**: El código debe pasar `cargo fmt` siempre.
*   **Clippy**: El código no debe tener warnings de `cargo clippy`.
*   **Idioms**: Prefiere iteradores y map/filter sobre bucles `for` imperativos complejos.

## 5. Configuración

*   **Cero Hardcoding**: Puertos, timeouts, tamaños de buffer, paths y límites de memoria DEBEN ser configurables via `forge-config`.
*   **Defaults Sensatos**: El sistema debe funcionar "out of the box" con valores por defecto seguros.

## 6. Testing

*   **Unit Tests**: Cada módulo lógico (`forge-store`, algoritmos) debe tener tests unitarios en el mismo archivo.
*   **Integration Tests**: Los flujos críticos (A2A, Proxy) deben tener tests de integración en `tests/`.

---
**Nota**: Estas reglas aplican a todo código nuevo. El código existente que viole estas reglas se considera Deuda Técnica y debe ser refactorizado progresivamente.
