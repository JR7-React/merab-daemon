# Sprint 29 — Unit Tests para módulos sin cobertura

## Objetivo

Agregar tests unitarios a los tres módulos implementados en sprints 17-20 que quedaron sin cobertura automatizada: `event_tail.rs`, `test_runner.rs`, y `jobs.rs` (daemon). Con esto todos los módulos de producción tendrán al menos un test.

## Problema actual

Los sprints 17, 19 y 20 son funcionales pero sus módulos clave no tienen `#[cfg(test)]`. Si alguien refactoriza `EventSink`, `TestRunner`, o `JobManager`, los bugs no se detectarán hasta runtime.

```
merab-cli/src/event_tail.rs       — 0 tests
merab-daemon/src/test_runner.rs   — 0 tests
merab-daemon/src/jobs.rs          — 0 tests
```

## Tests a agregar

### 1. `merab-cli/src/event_tail.rs`

Función a testear: lógica de parsing de líneas NDJSON en `process_event_line`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_thinking_event() {
        // Línea NDJSON con kind = "thinking"
        let line = r#"{"kind":"thinking","persona":"Coder","message":"Analizando..."}"#;
        let result = parse_event_line(line);
        assert!(result.is_some());
        let ev = result.unwrap();
        assert_eq!(ev.persona.as_deref(), Some("Coder"));
    }

    #[test]
    fn test_parse_done_event() {
        let line = r#"{"kind":"done"}"#;
        let result = parse_event_line(line);
        assert!(result.is_some());
    }

    #[test]
    fn test_invalid_json_returns_none() {
        let result = parse_event_line("not json at all");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_line_returns_none() {
        let result = parse_event_line("");
        assert!(result.is_none());
    }
}
```

**Nota:** Si `parse_event_line` no existe como función separada, extraerla del loop de lectura para poder testearla. El loop principal no cambia.

### 2. `merab-daemon/src/test_runner.rs`

Función a testear: `TestRunner::detect()` y parsing de output de tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_cargo_project() {
        // Directorio con Cargo.toml → debe detectar Cargo
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        let runner = TestRunner::detect(tmp.path());
        assert!(matches!(runner, TestRunner::Cargo));
    }

    #[test]
    fn test_detect_npm_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();
        let runner = TestRunner::detect(tmp.path());
        assert!(matches!(runner, TestRunner::Npm));
    }

    #[test]
    fn test_detect_unknown_project() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = TestRunner::detect(tmp.path());
        assert!(matches!(runner, TestRunner::Unknown));
    }

    #[test]
    fn test_parse_cargo_test_output_all_pass() {
        let output = "test result: ok. 5 passed; 0 failed; 0 ignored";
        let result = TestRunResult::from_output(output, true);
        assert!(result.passed);
        assert_eq!(result.passed_count, 5);
        assert_eq!(result.failed_count, 0);
    }

    #[test]
    fn test_parse_cargo_test_output_with_failures() {
        let output = "test result: FAILED. 3 passed; 2 failed; 0 ignored";
        let result = TestRunResult::from_output(output, false);
        assert!(!result.passed);
        assert_eq!(result.failed_count, 2);
    }
}
```

**Nota:** Requiere `tempfile = "3"` en `[dev-dependencies]` de `merab-daemon/Cargo.toml`.

### 3. `merab-daemon/src/jobs.rs` o `merab-store/src/jobs.rs`

Función a testear: ciclo de vida de un job en la base de datos.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().expect("test db")
    }

    #[test]
    fn test_job_lifecycle() {
        let db = test_db();
        let id = "test-job-1";

        // Crear
        db.create_job(id, "cargo test --workspace").unwrap();
        let job = db.get_job(id).unwrap().unwrap();
        assert_eq!(job.status, "pending");

        // Iniciar
        db.start_job(id).unwrap();
        let job = db.get_job(id).unwrap().unwrap();
        assert_eq!(job.status, "running");

        // Completar
        db.complete_job(id, Some("ok")).unwrap();
        let job = db.get_job(id).unwrap().unwrap();
        assert_eq!(job.status, "done");
    }

    #[test]
    fn test_job_cancel() {
        let db = test_db();
        db.create_job("job-cancel", "tarea").unwrap();
        db.cancel_job("job-cancel").unwrap();
        let job = db.get_job("job-cancel").unwrap().unwrap();
        assert_eq!(job.status, "cancelled");
    }

    #[test]
    fn test_list_jobs_returns_recent_first() {
        let db = test_db();
        db.create_job("job-a", "tarea a").unwrap();
        db.create_job("job-b", "tarea b").unwrap();
        let jobs = db.list_jobs(10).unwrap();
        assert!(jobs.len() >= 2);
    }
}
```

**Nota:** Requiere `Database::open_in_memory()` si no existe. Ver si ya existe en `merab-store/src/db.rs` — en los tests de sessions ya se usa un helper similar.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-cli/src/event_tail.rs` | Extraer `parse_event_line()` + agregar `#[cfg(test)]` |
| `merab-daemon/src/test_runner.rs` | Agregar `#[cfg(test)]` con 5 tests |
| `merab-daemon/Cargo.toml` | Agregar `tempfile = "3"` en `[dev-dependencies]` |
| `merab-store/src/jobs.rs` | Agregar `#[cfg(test)]` con 3 tests de ciclo de vida |

## Verificación

```bash
cargo test --workspace
# Debe mostrar: ~67+ tests, 0 failed
# Nuevos tests:
#   merab_cli::event_tail::tests::* (4 tests)
#   merab_daemon::test_runner::tests::* (5 tests)
#   merab_store::jobs::tests::* (3 tests)
```

## Notas para el modelo de IA

- **Verificar si `Database::open_in_memory()` existe** en `merab-store/src/db.rs` antes de implementarlo. Si ya existe (para tests de sessions), usarlo directamente.
- **`parse_event_line`**: si la lógica de parseo está inline en el loop de `event_tail.rs`, extraerla a una función `pub(crate) fn parse_event_line(line: &str) -> Option<ProgressEvent>` sin cambiar el loop.
- **No agregar `tempfile`** si los tests de `test_runner` pueden usar `std::env::temp_dir()` con limpieza manual — pero `tempfile` es más limpio.
- **Mantener bajo 500 líneas** en todos los archivos modificados.
- Dificultad: **Fácil** (~60 líneas de tests nuevos en total).
