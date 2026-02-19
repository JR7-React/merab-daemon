use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show all current configuration values
    List,
    /// Get a specific configuration value (e.g. proxy.api_key)
    Get {
        /// Dot-separated key
        key: String,
    },
    /// Set a configuration value (writes to user config file)
    Set {
        /// Dot-separated key
        key: String,
        /// New value
        value: String,
    },
    /// Open the config file in $EDITOR
    Edit,
    /// Show the path of the active config file
    Path,
}

/// Retorna la ruta canónica del archivo de config del usuario.
/// Crea el directorio si no existe.
pub fn user_config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("Cannot determine config directory")?
        .join("merab");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.toml"))
}

pub fn config_path() -> Result<()> {
    let path = user_config_path()?;
    println!("{}", path.display());
    Ok(())
}

pub fn config_edit() -> Result<()> {
    let path = user_config_path()?;
    if !path.exists() {
        std::fs::write(&path, "")?;
    }
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "nano".to_string()
            }
        });
    std::process::Command::new(&editor).arg(&path).status()?;
    Ok(())
}

pub fn config_get(key: &str) -> Result<()> {
    let config = merab_config::MerabConfig::load()?;
    let json = serde_json::to_value(&config)?;
    let value = navigate_json(&json, key)?;
    // Ocultar API keys parcialmente
    let display = if key.contains("api_key") {
        mask_api_key(value.as_str().unwrap_or(""))
    } else {
        value.to_string().trim_matches('"').to_string()
    };
    println!("{}", display);
    Ok(())
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let path = user_config_path()?;

    // Leer TOML existente o crear tabla vacía
    let content = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };

    let mut doc: toml::Value = if content.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(&content).context("Failed to parse existing config.toml")?
    };

    set_toml_key(&mut doc, key, value)?;

    let serialized = toml::to_string_pretty(&doc)?;
    std::fs::write(&path, &serialized)?;

    let display = if key.contains("api_key") {
        mask_api_key(value)
    } else {
        value.to_string()
    };
    println!("Saved: {} = {}", key, display);
    Ok(())
}

pub fn config_list() -> Result<()> {
    let config = merab_config::MerabConfig::load()?;
    let json = serde_json::to_value(&config)?;

    println!("{:<45} {}", "KEY", "VALUE");
    println!("{}", "─".repeat(70));

    print_json_flat(&json, "");
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn navigate_json<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a serde_json::Value> {
    let mut current = value;
    for segment in key.split('.') {
        current = current
            .get(segment)
            .with_context(|| format!("Key '{}' not found in config", key))?;
    }
    Ok(current)
}

fn set_toml_key(doc: &mut toml::Value, key: &str, value: &str) -> Result<()> {
    let segments: Vec<&str> = key.split('.').collect();
    if segments.is_empty() {
        bail!("Empty key");
    }

    // Navegar hasta el penúltimo segmento, creando tablas intermedias si no existen
    let mut current = doc;
    for seg in &segments[..segments.len() - 1] {
        let table = current
            .as_table_mut()
            .with_context(|| format!("Expected table at segment '{}'", seg))?;
        if !table.contains_key(*seg) {
            table.insert(seg.to_string(), toml::Value::Table(toml::map::Map::new()));
        }
        current = table.get_mut(*seg).unwrap();
    }

    let last = segments[segments.len() - 1];
    let table = current
        .as_table_mut()
        .with_context(|| format!("Expected table at '{}'", &segments[..segments.len() - 1].join(".")))?;

    // Inferir tipo: intentar i64, luego f64, luego bool, luego string
    let toml_value = if let Ok(n) = value.parse::<i64>() {
        toml::Value::Integer(n)
    } else if let Ok(f) = value.parse::<f64>() {
        toml::Value::Float(f)
    } else if value == "true" {
        toml::Value::Boolean(true)
    } else if value == "false" {
        toml::Value::Boolean(false)
    } else {
        toml::Value::String(value.to_string())
    };

    table.insert(last.to_string(), toml_value);
    Ok(())
}

fn print_json_flat(value: &serde_json::Value, prefix: &str) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                print_json_flat(v, &full_key);
            }
        }
        serde_json::Value::Null => {
            println!("{:<45} (not set)", prefix);
        }
        other => {
            let raw = other.to_string();
            let display = if prefix.contains("api_key") {
                mask_api_key(raw.trim_matches('"'))
            } else {
                raw.trim_matches('"').to_string()
            };
            println!("{:<45} {}", prefix, display);
        }
    }
}

fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return "(not set)".to_string();
    }
    let len = key.len();
    if len <= 8 {
        return "***".to_string();
    }
    let prefix = &key[..8.min(len)];
    let suffix = if len > 3 { &key[len - 3..] } else { "" };
    format!("{}***{}", prefix, suffix)
}
