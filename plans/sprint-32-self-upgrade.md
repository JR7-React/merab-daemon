# Sprint 32 — Self-Upgrade (Merab mejora su propio código)

## Objetivo

`merab self-upgrade "descripción"` permite al AI modificar el propio código fuente de Merab, correr los tests, compilar, y si todo pasa, hacer commit automático. El runtime se auto-mejora usando sus propias herramientas. Esto es la concreción más directa del objetivo de revolucionar la programación.

## Problema actual

Merab puede modificar cualquier proyecto de usuario, pero para modificarse a sí mismo necesita que el usuario ejecute manualmente `merab ask` desde el directorio correcto y con el contexto adecuado. No hay un comando dedicado que encapsule el loop completo: modificar → compilar → testear → commit.

## Comportamiento objetivo

```bash
# Desde cualquier directorio
$ merab self-upgrade "agrega un flag --verbose a merab doctor para mostrar más detalles"

Merab Self-Upgrade
══════════════════════════════════════════════
Proyecto: /home/user/dev/merab
Modo: build + test + commit automático si pasan

[Coder]     Leyendo doctor.rs...
[Coder]     Aplicando patch a doctor.rs (+45 líneas)
[Coder]     Aplicando patch a main.rs (+3 líneas)
[Engineer]  Compilando workspace...
[QA]        Corriendo tests... 67 passed, 0 failed ✓
[Reviewer]  Revisando cambios...

✓ Build: OK
✓ Tests: 67/67 pasaron
✓ Review: sin problemas críticos

¿Hacer commit de estos cambios? [s/N]: s
[master 4f2a1b9] feat: add --verbose flag to merab doctor

Upgrade completado.
```

```bash
# Modo no-interactivo (para pipelines)
$ merab self-upgrade "..." --yes
# Hace commit automático si pasan build + tests

# Solo proponer cambios sin aplicar
$ merab self-upgrade "..." --dry-run
# Muestra el plan y los diffs pero no modifica archivos
```

## Implementación

### Prerequisitos

Este sprint requiere que estén implementados:
- Sprint 19 (test loop) ✓
- Sprint 31 (diff editing) — recomendado para ediciones precisas

### `crates/merab-cli/src/self_upgrade.rs` (nuevo)

```rust
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
    }

    // 1. Construir task con contexto de self-upgrade
    let full_task = build_self_upgrade_task(&cfg.task, &cfg.merab_root);

    // 2. Ejecutar con test loop habilitado
    let response = client.ai_orchestrate_with_tests(
        full_task,
        None, // event_file: el streaming normal se encarga
    ).await?;

    if cfg.dry_run {
        println!("\n[dry-run] Cambios propuestos:");
        println!("{}", response.content);
        return Ok(());
    }

    // 3. Verificar que build y tests pasaron
    println!("\n{}", response.content);

    // 4. Pedir confirmación si no es --yes
    let should_commit = if cfg.yes {
        true
    } else {
        confirm_commit()?
    };

    if should_commit {
        commit_changes(client, &cfg.task, &cfg.merab_root).await?;
    } else {
        println!("Cambios aplicados pero no commiteados. Revisa con 'git diff'.");
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
- tracing para logs, nunca println!
- anyhow en binarios (merab-cli, merab-daemon), thiserror en librerías
- Máximo 500 líneas por archivo nuevo
- Después de cada cambio, el código debe compilar con `cargo build --workspace`
- Los tests deben pasar con `cargo test --workspace`

Tarea: {}

Pasos:
1. Lee los archivos relevantes usando file.read
2. Aplica los cambios mínimos necesarios (usa file.patch para modificaciones)
3. Verifica que `cargo build --workspace` pasa
4. Verifica que `cargo test --workspace` pasa
5. Si algo falla, corrígelo antes de continuar"#,
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
    ).await?;

    let commit_hash = result
        .get("hash")
        .and_then(|h| h.as_str())
        .unwrap_or("unknown");

    println!("[master {}] {}", &commit_hash[..7.min(commit_hash.len())], msg);
    println!("\nUpgrade completado.");
    Ok(())
}
```

### `merab-git`: agregar herramienta `git.commit`

En `crates/merab-git/src/tools.rs`:

```rust
"git.commit" => {
    let path = args["path"].as_str().unwrap_or(".");
    let message = args["message"].as_str()
        .ok_or_else(|| anyhow!("missing message"))?;
    let repo = git2::Repository::open(path)?;
    // stage all modified tracked files
    let mut index = repo.index()?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = repo.signature()?;
    let parent = repo.head()?.peel_to_commit()?;
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;
    Ok(json!({ "hash": oid.to_string(), "message": message }))
}
```

### CLI en `main.rs`

```rust
/// Upgrade Merab itself using AI
SelfUpgrade {
    /// What to add or change
    task: String,
    /// Auto-commit if build and tests pass
    #[arg(long)]
    yes: bool,
    /// Show proposed changes without applying
    #[arg(long)]
    dry_run: bool,
},
```

Match arm:

```rust
Commands::SelfUpgrade { task, yes, dry_run } => {
    // Detectar root de merab automáticamente
    // Primero: variable de entorno MERAB_SOURCE_PATH
    // Segundo: buscar Cargo.toml con name="merab-cli" hacia arriba desde cwd
    let merab_root = detect_merab_root()?;
    let cfg = self_upgrade::SelfUpgradeConfig { task, yes, dry_run, merab_root };
    self_upgrade::run_self_upgrade(&client, cfg).await?;
}
```

```rust
fn detect_merab_root() -> anyhow::Result<std::path::PathBuf> {
    // 1. Variable de entorno explícita
    if let Ok(p) = std::env::var("MERAB_SOURCE_PATH") {
        return Ok(std::path::PathBuf::from(p));
    }
    // 2. Buscar hacia arriba desde cwd
    let mut current = std::env::current_dir()?;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)?;
            if content.contains("merab-cli") && content.contains("[workspace]") {
                return Ok(current);
            }
        }
        if !current.pop() {
            break;
        }
    }
    Err(anyhow::anyhow!(
        "No se encontró el directorio raíz de Merab.\n\
         Configura MERAB_SOURCE_PATH=/ruta/a/merab"
    ))
}
```

### Guardrails de seguridad

Añadir en `build_self_upgrade_task`:

```rust
// Instrucciones de seguridad explícitas:
r#"
RESTRICCIONES DE SEGURIDAD (obligatorias):
- NO elimines archivos existentes
- NO modifiques Cargo.lock manualmente
- NO hagas git push, solo git commit local
- Si no estás seguro de un cambio, agrégalo como comentario TODO en lugar de modificar
- Cada cambio debe ser la mínima modificación necesaria para la tarea
"#
```

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-cli/src/self_upgrade.rs` | Nuevo — lógica completa de self-upgrade |
| `merab-cli/src/main.rs` | Agregar `SelfUpgrade` variant, `mod self_upgrade`, `detect_merab_root` |
| `merab-git/src/tools.rs` | Agregar herramienta `git.commit` |

## Verificación

```bash
# Dry run primero (seguro, no modifica nada)
MERAB_SOURCE_PATH=/ruta/merab merab self-upgrade \
  "agrega --verbose a merab doctor" --dry-run
# → Muestra plan sin aplicar cambios

# Self-upgrade real (con confirmación manual)
MERAB_SOURCE_PATH=/ruta/merab merab self-upgrade \
  "agrega un campo uptime_seconds a SystemStatus"
# → Aplica cambios, corre build + tests, pide confirmación

# Verificar que el resultado compila
cargo build --workspace
cargo test --workspace
```

## Notas para el modelo de IA

- **`detect_merab_root`** debe ser robusto: buscar el `Cargo.toml` workspace que contenga `"merab-cli"` como miembro, no solo cualquier `Cargo.toml`.
- **`git.commit` en merab-git** usa `git2` crate que ya debería estar en dependencias. Si no, añadir `git2 = "0.19"`.
- **El task de self-upgrade** debe inyectar el contenido de `CLAUDE.md` del proyecto Merab como contexto adicional — el AI necesita conocer todas las convenciones.
- **No ejecutar en modo `--bg`**: self-upgrade es interactivo por diseño (pide confirmación). El flag `--yes` es para automatización explícita.
- **Dificultad**: Media (~180 líneas). La complejidad real está en el prompting correcto del task.
