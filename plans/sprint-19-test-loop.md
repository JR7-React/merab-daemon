# Sprint 19 — Test Loop (Auto-Fix)

## Objetivo

Después de que el pipeline escribe código, Merab corre los tests automáticamente. Si fallan, lanza un nuevo ciclo de corrección hasta que pasen o se agoten los intentos. Esto es el ciclo TDD autónomo.

## Problema actual

El pipeline escribe código pero nunca verifica que funcione. El usuario tiene que correr los tests manualmente y volver a pedir correcciones.

## Comportamiento objetivo

```bash
$ merab ask --test "implementa el módulo de autenticación"

[Planner] Descomponiendo tarea...
[Pipeline] Ejecutando 3 subtasks...

[coder] Implementando auth.rs... ✓
[qa] Escribiendo tests... ✓

[Test Runner] Corriendo tests...
  running 4 tests
  test auth::test_valid_token ... ok
  test auth::test_expired_token ... FAILED    ← falla

[Fix Loop 1/3] Enviando error al coder...
[coder] Corrigiendo lógica de expiración...

[Test Runner] Corriendo tests...
  test auth::test_expired_token ... ok        ← corregido
  test result: ok. 4 passed

✓ Todos los tests pasan. Completado en 2 ciclos.
```

## Implementación

### `crates/merab-core/src/test_result.rs` (nuevo)
```rust
pub struct TestRunResult {
    pub passed: u32,
    pub failed: u32,
    pub output: String,
    pub success: bool,
}
```

### `crates/merab-daemon/src/test_runner.rs` (nuevo)
```rust
/// Detecta el test runner del proyecto y lo ejecuta.
pub async fn run_tests(project_path: &str) -> TestRunResult {
    // Detecta: cargo test / npm test / pytest / go test / etc.
    // Ejecuta vía merab-shell agent
    // Parsea output para contar passed/failed
}
```

Detectar test runner:
- Si existe `Cargo.toml` → `cargo test`
- Si existe `package.json` con `scripts.test` → `npm test`
- Si existe `pytest.ini` o `pyproject.toml` con `[tool.pytest]` → `pytest`
- Si existe `go.mod` → `go test ./...`

### `crates/merab-daemon/src/rpc/ai_methods.rs`
Nueva función `handle_ai_orchestrate_with_tests()`:
```rust
/// Ejecuta pipeline + test loop hasta max_fix_cycles.
pub async fn handle_ai_orchestrate_with_tests(
    config: &MerabConfig,
    mcp_manager: &Arc<McpManager>,
    db: &Arc<Mutex<Database>>,
    task: String,
    max_fix_cycles: u32,
) -> Result<AiResponse, ErrorObjectOwned>
```

Flujo:
1. `handle_ai_orchestrate()` — ejecuta pipeline normal
2. `run_tests()` — corre los tests
3. Si pasan → terminar
4. Si fallan y `cycles < max_fix_cycles`:
   - Construir prompt: `"Los tests fallan:\n{output}\nCorrige el código."`
   - Volver a ejecutar el pipeline con ese prompt
   - Goto 2

### `crates/merab-config/src/lib.rs`
```rust
pub struct AiConfig {
    // ...campos actuales...
    pub max_fix_cycles: u32,    // default: 3
    pub auto_run_tests: bool,   // default: false (opt-in)
}
```

### `crates/merab-daemon/src/rpc/server.rs`
```
merab.ai.orchestrate ahora revisa config.ai.auto_run_tests
```
O bien agregar flag explícito en el RPC.

### `crates/merab-cli/src/main.rs`
```
Commands::Ask {
    question: String,
    #[arg(long)]
    test: bool,    // --test activa el loop
}
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-core/src/test_result.rs` | Nuevo — `TestRunResult` |
| `merab-daemon/src/test_runner.rs` | Nuevo — detectar y correr tests |
| `merab-daemon/src/rpc/ai_methods.rs` | `handle_ai_orchestrate_with_tests` |
| `merab-config/src/lib.rs` | `max_fix_cycles`, `auto_run_tests` |
| `merab-cli/src/main.rs` | Flag `--test` en `Ask` |

## Verificación

```bash
# En un proyecto Rust con tests existentes:
merab ask --test "agrega un método sum() a Calculator que falle en el primer intento"
# Debe: escribir código con bug → detectar fallo → corregir → pasar
```

## Notas

- `max_fix_cycles` default 3 para evitar loops infinitos y costos descontrolados.
- Si después de N ciclos los tests siguen fallando, reportar los errores al usuario sin lanzar más ciclos.
- El test runner usa el `merab-shell` agent, así que debe estar corriendo.
- No correr tests si el proyecto no tiene un runner detectado — reportar warning.
