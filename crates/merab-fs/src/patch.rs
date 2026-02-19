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
