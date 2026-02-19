# Sprint 31 — Edición por Diffs

## Objetivo

En lugar de reescribir archivos completos, el AI produce diffs unificados precisos que se aplican quirúrgicamente. Reduce el consumo de tokens, minimiza errores de edición, y hace el output del AI más fácil de revisar.

## Problema actual

Hoy cuando el AI modifica un archivo, la herramienta `file.write` reemplaza el contenido completo. Esto tiene tres problemas:

1. **Tokens**: el AI necesita repetir todo el archivo aunque solo cambie 3 líneas
2. **Errores silenciosos**: si el AI "olvida" una función al reescribir, desaparece sin aviso
3. **Review difícil**: el usuario no puede ver qué cambió fácilmente — todo es "nuevo"

```bash
# Hoy: para cambiar una línea en un archivo de 300 líneas
# → El AI envía 300 líneas completas al tool call (gasto innecesario)

# Con diffs:
# → El AI envía solo el hunk de 5 líneas que cambia
```

## Comportamiento objetivo

```bash
$ merab ask "cambia el timeout de conexión de 30s a 60s en el cliente HTTP"

# El AI llama: file.patch con:
--- a/crates/merab-transport/src/http.rs
+++ b/crates/merab-transport/src/http.rs
@@ -45,7 +45,7 @@ impl HttpClient {
     pub fn new(url: &str) -> Self {
         Self {
             client: reqwest::Client::builder()
-                .timeout(Duration::from_secs(30))
+                .timeout(Duration::from_secs(60))
                 .build()
                 .expect("valid client"),
         }

✓ Patch aplicado: crates/merab-transport/src/http.rs (+1 -1)
```

## Implementación

### Nueva herramienta MCP en `merab-fs`: `file.patch`

En `crates/merab-fs/src/tools.rs` agregar:

```rust
"file.patch" => {
    let file_path = args["path"].as_str()
        .ok_or_else(|| anyhow!("missing path"))?;
    let diff = args["diff"].as_str()
        .ok_or_else(|| anyhow!("missing diff"))?;
    apply_patch(file_path, diff).await
}
```

### Motor de patch: `crates/merab-fs/src/patch.rs` (nuevo)

```rust
use anyhow::{anyhow, Result};
use std::path::Path;

/// Aplica un diff unificado a un archivo.
/// El diff puede ser parcial (solo hunks) o completo (con encabezado --- +++).
pub async fn apply_patch(file_path: &str, diff: &str) -> Result<serde_json::Value> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(anyhow!("Archivo no encontrado: {}", file_path));
    }

    let original = tokio::fs::read_to_string(path).await?;
    let patched = apply_unified_diff(&original, diff)?;
    tokio::fs::write(path, &patched).await?;

    let added = patched.lines().count() as i64 - original.lines().count() as i64;
    Ok(serde_json::json!({
        "success": true,
        "path": file_path,
        "lines_delta": added,
        "message": format!("Patch aplicado: {} ({:+} líneas)", file_path, added)
    }))
}

/// Parsea y aplica hunks de diff unificado.
/// Soporta formato estándar @@ -L,N +L,N @@ con líneas +/-/ (contexto).
pub fn apply_unified_diff(original: &str, diff: &str) -> Result<String> {
    let original_lines: Vec<&str> = original.lines().collect();
    let hunks = parse_hunks(diff)?;

    let mut result: Vec<String> = Vec::new();
    let mut orig_pos = 0usize; // posición actual en original (0-indexed)

    for hunk in &hunks {
        // Validar que el hunk es aplicable
        let orig_start = hunk.orig_start.saturating_sub(1);
        if orig_start < orig_pos {
            return Err(anyhow!("Hunk overlaps previous changes at line {}", hunk.orig_start));
        }

        // Copiar líneas de original antes del hunk
        for i in orig_pos..orig_start {
            result.push(original_lines.get(i)
                .ok_or_else(|| anyhow!("Line {} out of bounds", i))?
                .to_string());
        }

        // Aplicar el hunk
        orig_pos = orig_start;
        for line in &hunk.lines {
            match line.chars().next() {
                Some(' ') => {
                    // Contexto: verificar que coincide
                    let expected = line[1..].trim_end_matches('\r');
                    let actual = original_lines.get(orig_pos)
                        .ok_or_else(|| anyhow!("Context line {} missing", orig_pos + 1))?;
                    if *actual != expected {
                        return Err(anyhow!(
                            "Context mismatch at line {}: expected {:?}, got {:?}",
                            orig_pos + 1, expected, actual
                        ));
                    }
                    result.push(actual.to_string());
                    orig_pos += 1;
                }
                Some('+') => {
                    result.push(line[1..].trim_end_matches('\r').to_string());
                }
                Some('-') => {
                    orig_pos += 1; // saltar línea eliminada
                }
                _ => {}
            }
        }
    }

    // Copiar el resto del archivo
    for i in orig_pos..original_lines.len() {
        result.push(original_lines[i].to_string());
    }

    Ok(result.join("\n") + if original.ends_with('\n') { "\n" } else { "" })
}

struct Hunk {
    orig_start: usize,
    lines: Vec<String>,
}

fn parse_hunks(diff: &str) -> Result<Vec<Hunk>> {
    let mut hunks = Vec::new();
    let mut current: Option<Hunk> = None;

    for line in diff.lines() {
        if line.starts_with("@@") {
            if let Some(h) = current.take() {
                hunks.push(h);
            }
            let orig_start = parse_hunk_header(line)?;
            current = Some(Hunk { orig_start, lines: Vec::new() });
        } else if line.starts_with("---") || line.starts_with("+++") {
            // encabezado del diff, ignorar
        } else if let Some(ref mut h) = current {
            h.lines.push(line.to_string());
        }
    }
    if let Some(h) = current {
        hunks.push(h);
    }
    Ok(hunks)
}

fn parse_hunk_header(header: &str) -> Result<usize> {
    // @@ -45,7 +45,7 @@  →  extraer 45 del -45
    let re = regex::Regex::new(r"@@ -(\d+)")?;
    let cap = re.captures(header)
        .ok_or_else(|| anyhow!("Invalid hunk header: {}", header))?;
    Ok(cap[1].parse()?)
}
```

### Actualizar el system prompt del AI

En `merab-daemon/src/rpc/ai_methods.rs`, en `build_dynamic_system_prompt()`:

```rust
// Agregar instrucción de preferir patches:
r#"
## Herramientas de edición de archivos

Tienes dos herramientas para modificar archivos:
- `file.write`: reemplaza el contenido completo del archivo. Usa SOLO para archivos nuevos.
- `file.patch`: aplica un diff unificado. Usa para MODIFICAR archivos existentes.

Para modificaciones, SIEMPRE prefiere `file.patch` sobre `file.write`.
Formato de diff:
```
@@ -<línea_orig>,<n_orig> +<línea_nueva>,<n_nuevo> @@
 contexto (sin prefijo)
-línea eliminada
+línea agregada
 contexto
```
"#
```

### Agregar `file.patch` al registro de herramientas del daemon

En `merab-daemon/src/rpc/ai_methods.rs`, en la lista de herramientas disponibles:

```rust
Tool {
    name: "file.patch".into(),
    description: "Apply a unified diff to an existing file. Prefer this over file.write for modifications.".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Absolute path to the file" },
            "diff": { "type": "string", "description": "Unified diff in standard format (@@ -L,N +L,N @@)" }
        },
        "required": ["path", "diff"]
    }),
}
```

### Actualizar `ArtifactTracker`

Cuando se aplica `file.patch`, registrar el archivo en `files_modified` del tracker (ya lo hace `file.write`, necesita hacerlo también `file.patch`).

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-fs/src/patch.rs` | Nuevo — motor de patch unificado |
| `merab-fs/src/tools.rs` | Agregar handler para `file.patch` |
| `merab-fs/Cargo.toml` | Agregar `regex = "1"` si no existe |
| `merab-daemon/src/rpc/ai_methods.rs` | Agregar `file.patch` al tool registry + instrucción en system prompt |

## Verificación

```bash
# Test directo del patch
merab ask "en el archivo X cambia la función Y para que retorne Option en vez de Result"
# → El AI debe usar file.patch con un hunk de ~10 líneas, no file.write con 200

# Verificar que el archivo resultante compila
cargo build
# → debe pasar sin errores

# Test con archivo nuevo (debe seguir usando file.write)
merab ask "crea un archivo utils.rs con una función helper"
# → El AI usa file.write (correcto para archivos nuevos)
```

## Tests unitarios (incluir en este sprint)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_simple_addition() {
        let original = "line1\nline2\nline3\n";
        let diff = "@@ -1,3 +1,4 @@\n line1\n+new line\n line2\n line3\n";
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "line1\nnew line\nline2\nline3\n");
    }

    #[test]
    fn test_apply_simple_deletion() {
        let original = "line1\nline2\nline3\n";
        let diff = "@@ -1,3 +1,2 @@\n line1\n-line2\n line3\n";
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "line1\nline3\n");
    }

    #[test]
    fn test_context_mismatch_returns_error() {
        let original = "line1\nline2\nline3\n";
        let diff = "@@ -1,2 +1,2 @@\n wrong_context\n-line2\n+replaced\n";
        assert!(apply_unified_diff(original, diff).is_err());
    }

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -45,7 +45,8 @@").unwrap(), 45);
        assert_eq!(parse_hunk_header("@@ -1,3 +1,4 @@ fn foo()").unwrap(), 1);
    }
}
```

## Notas para el modelo de IA

- **El motor de patch debe ser conservador**: si el contexto no coincide exactamente, retornar error en lugar de aplicar el patch parcialmente. Es mejor fallar ruidosamente que corromper el archivo.
- **No usar la crate `patch`** de crates.io — tiene demasiadas dependencias. Implementar el parser mínimo directamente.
- **El system prompt es crucial**: sin la instrucción explícita, el AI seguirá usando `file.write` por defecto. La instrucción debe ser clara y aparecer cerca del inicio del prompt de herramientas.
- **Archivos nuevos siguen usando `file.write`**: no cambiar ese behavior. El patch solo aplica para modificaciones.
- **Dificultad**: Media (~200 líneas). La parte más compleja es el motor de patch; el resto es wiring.
