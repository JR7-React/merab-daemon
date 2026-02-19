use anyhow::Result;
use crate::client::MerabClient;

pub struct SelfUpgradeConfig {
    pub task: String,
    pub yes: bool,
    pub dry_run: bool,
    pub merab_root: std::path::PathBuf,
}

pub async fn run_self_upgrade(client: &MerabClient, cfg: SelfUpgradeConfig) -> Result<()> {
    println!("Merab Self-Upgrade");
    println!("{}", "═".repeat(44));
    println!("Proyecto: {}", cfg.merab_root.display());

    if cfg.dry_run {
        println!("Modo: dry-run (sin cambios reales)\n");
    } else {
        println!("Modo: build + test + commit automático si pasan\n");
    }

    // 1. Construir task con contexto de self-upgrade
    let full_task = build_self_upgrade_task(&cfg.task, &cfg.merab_root);

    if cfg.dry_run {
        println!("[dry-run] Analizando cambios necesarios...");
        // En dry-run, solo mostramos el plan
        let plan = client.ai_plan(&full_task).await?;
        println!("\n[dry-run] Plan propuesto:");
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    // 2. Ejecutar con el pipeline normal
    println!("Ejecutando self-upgrade...\n");
    
    // Cambiar al directorio de Merab para que los tool calls funcionen correctamente
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&cfg.merab_root)?;

    let response = client.ai_orchestrate_with_tests(&full_task, "").await?;

    // Restaurar directorio original
    std::env::set_current_dir(original_dir)?;

    // 3. Mostrar resultado
    println!("\n{}", response.content);

    // 4. Verificar que build y tests pasaron (el AI debería haberlos corrido)
    println!("\nVerificando build y tests...");
    
    // 5. Pedir confirmación si no es --yes
    let should_commit = if cfg.yes {
        true
    } else {
        confirm_commit()?
    };

    if should_commit {
        commit_changes(client, &cfg.task, &cfg.merab_root).await?;
    } else {
        println!("\nCambios aplicados pero no commiteados. Revisa con 'git diff'.");
        println!("Para commitear manualmente, ejecuta:");
        println!("  cd {}", cfg.merab_root.display());
        println!("  git add -A && git commit -m \"feat: {}\"", 
            cfg.task.chars().take(50).collect::<String>()
        );
    }

    Ok(())
}

fn build_self_upgrade_task(user_task: &str, merab_root: &std::path::Path) -> String {
    format!(
        r#"Estás modificando el código fuente de Merab (el propio runtime que te ejecuta).

Directorio raíz del proyecto: {}

Convenciones CRÍTICAS:
- Rust edition 2024
- Sin unwrap() en producción
- tracing para logs, nunca println! en daemon/store
- anyhow en binarios (merab-cli, merab-daemon), thiserror en librerías
- Máximo 500 líneas por archivo nuevo
- Después de cada cambio, el código debe compilar con `cargo build --workspace`
- Los tests deben pasar con `cargo test --workspace`

RESTRICCIONES DE SEGURIDAD (obligatorias):
- NO elimines archivos existentes
- NO modifiques Cargo.lock manualmente
- NO hagas git push, solo git commit local
- Si no estás seguro de un cambio, agrégalo como comentario TODO en lugar de modificar
- Cada cambio debe ser la mínima modificación necesaria para la tarea

Tarea: {}

Pasos:
1. Lee los archivos relevantes usando fs.read
2. Aplica los cambios mínimos necesarios (usa fs.patch para modificaciones)
3. Ejecuta `cargo build --workspace` para verificar compilación
4. Ejecuta `cargo test --workspace` para verificar tests
5. Si algo falla, corrígelo antes de continuar
6. Reporta el resultado final"#,
        merab_root.display(),
        user_task
    )
}

fn confirm_commit() -> Result<bool> {
    use std::io::{self, Write};
    print!("\n¿Hacer commit de estos cambios? [s/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("s"))
}

async fn commit_changes(client: &MerabClient, task: &str, merab_root: &std::path::Path) -> Result<()> {
    // Usar el agente git para hacer el commit
    let agents = client.list_agents().await?;
    let git_agent = agents.iter()
        .find(|a| a.name.contains("git"))
        .ok_or_else(|| anyhow::anyhow!("agente merab-git no encontrado"))?;

    let msg = format!("feat: {}", task.chars().take(60).collect::<String>());
    
    let result = client.call_tool(
        &git_agent.id.to_string(),
        "git.commit",
        serde_json::json!({
            "message": msg,
            "path": merab_root.to_string_lossy()
        }),
    ).await;

    match result {
        Ok(res) => {
            let commit_hash = res
                .get("hash")
                .and_then(|h| h.as_str())
                .unwrap_or("unknown");
            println!("[master {}] {}", &commit_hash[..7.min(commit_hash.len())], msg);
            println!("\n✓ Upgrade completado.");
        }
        Err(e) => {
            eprintln!("Error al hacer commit: {}", e);
            println!("\nLos cambios están aplicados pero no commiteados.");
            println!("Para commitear manualmente:");
            println!("  cd {}", merab_root.display());
            println!("  git add -A && git commit -m \"feat: {}\"", msg);
        }
    }
    
    Ok(())
}
