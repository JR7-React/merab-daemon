# Sprint 23 — `merab review` (Code Review Automático)

## Objetivo

`merab review` analiza los cambios de git del proyecto actual y produce feedback estructurado de código: bugs potenciales, problemas de estilo, sugerencias de mejora, y un resumen ejecutivo.

## Problema actual

El pipeline de Merab puede escribir código pero no tiene un modo específico de revisión. Para revisar código el usuario tendría que formular manualmente la tarea con `merab ask`, y el LLM no tendría contexto del diff automáticamente.

## Comportamiento objetivo

```bash
# Revisar cambios staged (git add pero no commiteados)
$ merab review
[merab] Analizando diff (3 archivos, +127 -45 líneas)...
[REVIEWER] Revisando cambios...

## Revisión de Código

### Resumen
3 archivos modificados. Cambios mayores en `src/auth.rs`.

### Problemas encontrados

**[CRÍTICO] src/auth.rs:47**
El token JWT no valida la fecha de expiración.
```rust
// Actual:
if token.user_id == expected_id { ... }
// Sugerido:
if token.user_id == expected_id && token.exp > Utc::now().timestamp() { ... }
```

**[ADVERTENCIA] src/api.rs:23**
`unwrap()` en código de producción. Considerar usar `?` con manejo de error.

### Sugerencias
- Agregar tests para el flujo de expiración de token
- El nombre `handle_req` no es descriptivo

### Veredicto: ⚠️ Cambios requieren revisión antes de mergear

Tokens: 2,340 input / 890 output — $0.002

# Revisar todos los cambios respecto a main (no solo staged)
$ merab review --branch main

# Revisar un archivo específico
$ merab review --file src/auth.rs

# Solo mostrar problemas críticos
$ merab review --critical

# Guardar review en un archivo
$ merab review --output review.md
```

## Implementación

### `crates/merab-cli/src/main.rs`

Agregar comando `Review`:

```rust
#[command(about = "Review code changes with AI")]
Review {
    /// Compare against this branch/commit (default: staged changes)
    #[arg(long)]
    branch: Option<String>,
    /// Review a specific file
    #[arg(long)]
    file: Option<String>,
    /// Show only critical issues
    #[arg(long)]
    critical: bool,
    /// Save review to this file
    #[arg(long)]
    output: Option<PathBuf>,
},
```

**Match arm:**

```rust
Commands::Review { branch, file, critical, output } => {
    let ready_client = bootstrap::ensure_ready(&cli.url).await?;

    // 1. Obtener el diff usando el cliente merab-git via RPC
    let diff = get_review_diff(&ready_client, branch.as_deref(), file.as_deref()).await?;

    if diff.trim().is_empty() {
        println!("No hay cambios para revisar.");
        println!("Usa 'git add' para stagear cambios, o '--branch <rama>' para comparar.");
        return Ok(());
    }

    let line_count = diff.lines().count();
    println!("[merab] Analizando diff ({} líneas)...", line_count);

    // 2. Construir task para el reviewer
    let task = build_review_task(&diff, critical);

    // 3. Llamar al pipeline con streaming de eventos
    let event_file = std::env::temp_dir()
        .join(format!("merab-review-{}.jsonl", std::process::id()));
    let event_file_str = event_file.to_string_lossy().to_string();

    let running = Arc::new(AtomicBool::new(true));
    let tail_handle = event_tail::start_event_tail(&event_file, running.clone());

    let result = ready_client.ai_orchestrate_stream(&task, &event_file_str).await;

    running.store(false, Ordering::Relaxed);
    let _ = tail_handle.join();
    let _ = std::fs::remove_file(&event_file);

    match result {
        Ok(resp) => {
            println!("\n{}", resp.content);
            if let Some(output_path) = output {
                std::fs::write(&output_path, &resp.content)?;
                println!("\nReview guardado en: {}", output_path.display());
            }
            if let Some(usage) = &resp.usage {
                println!("\nTokens: {} input / {} output", usage.input, usage.output);
                if let Some(cost) = merab_core::estimate_cost(&usage.model, usage.input, usage.output) {
                    println!("Costo estimado: ${:.4}", cost);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

#### Función `get_review_diff`

Usa el agente `merab-git` via RPC (`merab.callTool`) para obtener el diff:

```rust
async fn get_review_diff(
    client: &MerabClient,
    branch: Option<&str>,
    file: Option<&str>,
) -> anyhow::Result<String> {
    // Buscar el agente merab-git registrado
    let agents = client.list_agents().await?;
    let git_agent = agents.iter()
        .find(|a| a.name.contains("git"))
        .ok_or_else(|| anyhow::anyhow!(
            "Agente merab-git no encontrado. Ejecuta 'merab init' primero."
        ))?;

    let mut args = serde_json::json!({});
    if let Some(b) = branch {
        args["branch"] = serde_json::Value::String(b.to_string());
    }
    if let Some(f) = file {
        args["path"] = serde_json::Value::String(f.to_string());
    }

    let result = client.call_tool(&git_agent.id, "git.diff", args).await?;

    // El resultado es un JSON MCP; extraer el texto
    let diff = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(diff)
}
```

#### Función `build_review_task`

```rust
fn build_review_task(diff: &str, critical_only: bool) -> String {
    let focus = if critical_only {
        "Reporta SOLO problemas críticos (bugs, vulnerabilidades de seguridad, data races)."
    } else {
        "Reporta problemas críticos, advertencias, y sugerencias de mejora."
    };

    format!(
        r#"Eres un revisor de código senior. Revisa el siguiente git diff y produce un informe estructurado en markdown.

{}

Para cada problema encontrado, incluye:
- Severidad: [CRÍTICO|ADVERTENCIA|SUGERENCIA]
- Archivo y línea aproximada
- Descripción del problema
- Código sugerido (si aplica)

Termina con una sección "Veredicto" que diga si los cambios son seguros para mergear.

## Git Diff

```diff
{}
```"#,
        focus, diff
    )
}
```

### Verificar que `git.diff` en merab-git soporta comparación de branches

Revisar `crates/merab-git/src/main.rs`. Si `git.diff` no soporta el parámetro `branch`, agregar lógica:
- Sin `branch`: `git diff --staged` (cambios staged)
- Con `branch`: `git diff <branch>...HEAD`
- Con `path`: agregar `-- <path>` al final del comando

Si el agente merab-git necesita modificación, actualizar el handler de `git.diff` para aceptar parámetros opcionales `branch` y `path`.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-cli/src/main.rs` | Agregar `Review` variant y match arm |
| `merab-git/src/main.rs` | Verificar y mejorar `git.diff` con parámetros opcionales |

## Verificación

```bash
# En cualquier repositorio git con cambios:
git add -p   # stagear algo

merab review
# Debe mostrar análisis del diff staged

merab review --branch main
# Debe mostrar todos los cambios vs main

merab review --output review.md
# Debe crear el archivo review.md
```

## Notas para el modelo de IA

- **El diff puede ser muy grande.** Si supera ~8000 tokens, considerar truncar o dividir en archivos. Por ahora, enviar completo y dejar que el LLM maneje el límite de contexto.
- **El agente git debe estar corriendo.** `bootstrap::ensure_ready` lo garantiza si el agente está registrado.
- **El merab-git `git.diff`** actual (Sprint 2.4) puede no tener parámetros de branch — verificar y extender si es necesario.
- **Usar `ai_orchestrate_stream`** para que el usuario vea progreso mientras espera (puede tardar 10-30s).
- **No hardcodear el nombre del agente.** Buscar por nombre que contenga "git" en la lista de agentes.
- El formato del resultado MCP es `{"content": [{"type": "text", "text": "..."}]}` — extraer `.content[0].text`.
