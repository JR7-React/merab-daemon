# Sprint 26 — Watch Mode (`merab watch`)

## Objetivo

`merab watch` monitorea cambios en archivos del proyecto y ejecuta automáticamente una tarea de IA cuando detecta modificaciones. Útil para: correr revisiones de código automáticas al guardar, regenerar tests al cambiar implementación, o validar convenciones continuamente.

## Problema actual

Todo el uso de Merab es reactivo (el usuario ejecuta un comando). No hay forma de integrarlo en el flujo de desarrollo continuo sin correr comandos manualmente después de cada cambio.

## Comportamiento objetivo

```bash
# Modo watch: ejecutar 'merab review' cada vez que cambie un archivo .rs
$ merab watch --pattern "**/*.rs" --task "revisa el código modificado y reporta problemas"
[merab watch] Observando **/*.rs en /home/user/project
[merab watch] Presiona Ctrl+C para detener.

# ... el usuario edita src/auth.rs ...

[merab watch] Cambio detectado: src/auth.rs
[Planner] Descomponiendo tarea...
[REVIEWER] Revisando cambios...
[REVIEWER] ✓ Done

## Código modificado

**[ADVERTENCIA] src/auth.rs:47**
Considera manejar el caso de token nulo explícitamente.
✓ Completado

# Modo de compilación automática:
$ merab watch --pattern "**/*.rs" --task "corre los tests y corrige los que fallen" --test

# Solo reportar, no bloquear la terminal (modo quiet):
$ merab watch --pattern "src/**" --task "review" --quiet &
```

## Implementación

### `crates/merab-cli/src/main.rs`

Agregar comando `Watch`:

```rust
#[command(about = "Watch files and run AI tasks on changes")]
Watch {
    /// Glob pattern of files to watch (default: "**/*")
    #[arg(long, short, default_value = "**/*")]
    pattern: String,

    /// Task to run when files change
    #[arg(long, short)]
    task: String,

    /// Also run tests and fix failures (like ask --test)
    #[arg(long)]
    test: bool,

    /// Debounce delay in seconds to batch rapid changes (default: 2)
    #[arg(long, default_value = "2")]
    debounce: u64,

    /// Suppress per-event output, only show errors
    #[arg(long)]
    quiet: bool,
},
```

**Match arm:**

```rust
Commands::Watch { pattern, task, test, debounce, quiet } => {
    use std::time::{Duration, Instant};

    let ready_client = bootstrap::ensure_ready(&cli.url).await?;
    let project_path = std::env::current_dir()?;

    println!("[merab watch] Observando {} en {}", pattern, project_path.display());
    println!("[merab watch] Tarea: \"{}\"", task);
    println!("[merab watch] Presiona Ctrl+C para detener.\n");

    let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();

    // Lanzar el watcher en un thread separado
    let pattern_clone = pattern.clone();
    let project_clone = project_path.clone();
    std::thread::spawn(move || {
        watch_files(&project_clone, &pattern_clone, tx);
    });

    // Loop principal: debounce + ejecutar tarea
    let debounce_dur = Duration::from_secs(debounce);
    let mut last_trigger = Instant::now() - debounce_dur; // permit first run
    let mut pending_files: Vec<PathBuf> = Vec::new();

    loop {
        // Recolectar eventos pendientes (no bloqueante)
        loop {
            match rx.try_recv() {
                Ok(path) => pending_files.push(path),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        if !pending_files.is_empty() && last_trigger.elapsed() >= debounce_dur {
            let changed: Vec<String> = pending_files.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            pending_files.clear();
            last_trigger = Instant::now();

            if !quiet {
                println!("[merab watch] Cambio detectado: {}", changed.join(", "));
            }

            // Construir tarea con contexto de qué archivos cambiaron
            let full_task = format!(
                "{}\n\nArchivos modificados:\n{}",
                task,
                changed.iter().map(|f| format!("- {}", f)).collect::<Vec<_>>().join("\n")
            );

            // Ejecutar el pipeline
            let event_file = std::env::temp_dir()
                .join(format!("merab-watch-{}.jsonl", std::process::id()));
            let event_file_str = event_file.to_string_lossy().to_string();

            let running = Arc::new(AtomicBool::new(true));
            let tail_handle = if !quiet {
                Some(event_tail::start_event_tail(&event_file, running.clone()))
            } else {
                None
            };

            let result = if test {
                ready_client.ai_orchestrate_with_tests(&full_task, &event_file_str).await
            } else {
                ready_client.ai_orchestrate_stream(&full_task, &event_file_str).await
            };

            running.store(false, Ordering::Relaxed);
            if let Some(handle) = tail_handle {
                let _ = handle.join();
            }
            let _ = std::fs::remove_file(&event_file);

            match result {
                Ok(resp) => {
                    if !quiet {
                        if let Some(usage) = &resp.usage {
                            println!("Tokens: {} | Costo: ${:.4}",
                                usage.input + usage.output,
                                merab_core::estimate_cost(&usage.model, usage.input, usage.output).unwrap_or(0.0));
                        }
                        println!();
                    }
                }
                Err(e) => eprintln!("[merab watch] Error: {}", e),
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
```

#### Función `watch_files`

Implementar polling de archivos (sin depender de `notify` crate para mantener las dependencias mínimas):

```rust
fn watch_files(
    project_path: &std::path::Path,
    pattern: &str,
    tx: std::sync::mpsc::Sender<PathBuf>,
) {
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    let mut last_modified: HashMap<PathBuf, SystemTime> = HashMap::new();
    let full_pattern = project_path.join(pattern).to_string_lossy().to_string();

    loop {
        // Obtener archivos que coinciden con el patrón
        if let Ok(paths) = glob::glob(&full_pattern) {
            for path in paths.flatten() {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        let prev = last_modified.get(&path).copied();
                        if prev != Some(modified) {
                            if prev.is_some() {
                                // Solo enviar si ya estaba tracked (no en el primer scan)
                                let _ = tx.send(path.clone());
                            }
                            last_modified.insert(path, modified);
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
```

### `crates/merab-cli/Cargo.toml`

Agregar dependencia `glob` para el patrón de archivos:

```toml
glob = "0.3"
```

Verificar si ya está antes de agregar.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-cli/src/main.rs` | Agregar `Watch` variant, match arm, y función `watch_files` |
| `merab-cli/Cargo.toml` | Agregar `glob = "0.3"` si no está |

## Verificación

```bash
# En un proyecto con archivos .rs:
merab watch --pattern "**/*.rs" --task "muestra los nombres de las funciones públicas en los archivos modificados"

# En otra terminal, editar un archivo:
echo "// cambio" >> src/main.rs

# El watch debe detectar el cambio y ejecutar la tarea automáticamente
# Después de ~2 segundos (debounce), mostrar output del LLM
```

## Notas para el modelo de IA

- **No usar `notify` crate**: Agregaría complejidad en Windows (diferencias entre backends). El polling cada 500ms es suficiente para uso normal.
- **`glob` crate**: Verificar si ya está en el workspace. Si no, agregar solo a `merab-cli/Cargo.toml`.
- **El debounce es crítico**: Los editores guardan múltiples veces en milisegundos (backup + actual). Sin debounce, el pipeline se lanzaría 3-5 veces por cada guardado.
- **El primer scan no dispara eventos**: La función `watch_files` toma una foto inicial y solo reporta cambios posteriores. Esto evita ejecutar la tarea al arrancar.
- **Ctrl+C**: El loop tokio se interrumpirá con Ctrl+C por el signal handler por defecto. No necesita manejo especial.
- **`watch_files` corre en thread de OS** (no async) porque usa `std::thread::sleep`. El canal `std::sync::mpsc` cruza el boundary sync/async correctamente.
- Si `glob` no puede importarse, usar `walkdir` crate o simplemente listar archivos recursivamente con `std::fs::read_dir`.
