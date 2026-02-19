use anyhow::Result;

use crate::client::MerabClient;

pub async fn get_review_diff(
    client: &MerabClient,
    branch: Option<&str>,
    file: Option<&str>,
) -> Result<String> {
    let agents = client.list_agents().await?;
    let git_agent = agents
        .iter()
        .find(|a| a.name.contains("git"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Agente merab-git no encontrado. Ejecuta 'merab init' primero."
            )
        })?;

    let mut args = serde_json::json!({});
    if let Some(b) = branch {
        args["branch"] = serde_json::Value::String(b.to_string());
    }
    if let Some(f) = file {
        args["path"] = serde_json::Value::String(f.to_string());
    }

    let result = client
        .call_tool(&git_agent.id.to_string(), "git.diff", args)
        .await?;

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

pub fn build_review_task(diff: &str, critical_only: bool) -> String {
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

use std::path::PathBuf;

pub async fn run_review(
    url: &str,
    branch: Option<String>,
    file: Option<String>,
    critical: bool,
    output: Option<PathBuf>,
) -> Result<()> {
    use crate::{bootstrap, event_tail};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let ready_client = bootstrap::ensure_ready(url).await?;

    let diff = get_review_diff(&ready_client, branch.as_deref(), file.as_deref()).await?;

    if diff.trim().is_empty() {
        println!("No hay cambios para revisar.");
        println!("Usa 'git add' para stagear cambios, o '--branch <rama>' para comparar.");
        return Ok(());
    }

    let line_count = diff.lines().count();
    println!("[merab] Analizando diff ({} líneas)...", line_count);

    let task = build_review_task(&diff, critical);

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
                if let Some(cost) = merab_core::pricing::estimate_cost(&usage.model, usage.input, usage.output) {
                    println!("Costo estimado: ${:.4}", cost);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
