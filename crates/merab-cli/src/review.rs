use anyhow::Result;
use merab_core::AgentSummary;

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
