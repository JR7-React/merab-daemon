# Sprint 22 — Config CLI (`merab config`)

## Objetivo

El usuario puede leer y escribir la configuración de Merab desde la terminal sin editar TOML manualmente.

## Problema actual

Para cambiar la API key, el modelo, o cualquier setting, el usuario tiene que:
1. Saber que existe `~/.config/merab/config.toml` (o `merab.toml` en CWD)
2. Encontrar el archivo manualmente
3. Editarlo con un editor de texto
4. Reiniciar el daemon

Esto es un bloqueador de onboarding crítico: el primer comando que un usuario nuevo necesita es `merab config set proxy.api_key sk-ant-...`.

## Comportamiento objetivo

```bash
# Ver toda la configuración actual (merged: defaults + file + env)
$ merab config list
proxy.api_key       = sk-ant-... (from ~/.config/merab/config.toml)
proxy.upstream_url  = https://openrouter.ai/api/v1 (default)
ai.model            = qwen/qwen3-coder:free (from merab.toml)
ai.max_tokens       = 1024 (default)
...

# Leer un valor específico
$ merab config get proxy.api_key
sk-ant-...

$ merab config get ai.model
qwen/qwen3-coder:free

# Escribir un valor (lo guarda en ~/.config/merab/config.toml)
$ merab config set proxy.api_key sk-ant-ABC123
Saved: proxy.api_key = sk-ant-ABC123

$ merab config set ai.model claude-opus-4-6
Saved: ai.model = claude-opus-4-6

$ merab config set ai.personas.coder.model deepseek/deepseek-coder-v2:free
Saved: ai.personas.coder.model = deepseek/deepseek-coder-v2:free

# Abrir el archivo de configuración en el editor del sistema
$ merab config edit
# Abre ~/.config/merab/config.toml con $EDITOR o notepad

# Ver ruta del archivo de configuración
$ merab config path
~/.config/merab/config.toml
```

## Implementación

### `crates/merab-cli/src/main.rs`

Agregar subcomando `Config(ConfigCommands)`:

```rust
/// Manage Merab configuration
#[command(subcommand)]
Config(ConfigCommands),
```

```rust
#[derive(Subcommand)]
enum ConfigCommands {
    /// Show all current configuration values
    List,
    /// Get a specific configuration value
    Get {
        /// Dot-separated key (e.g. proxy.api_key, ai.model)
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
```

**Match arm** — no necesita daemon (sin `bootstrap::ensure_ready`):

```rust
Commands::Config(cmd) => {
    match cmd {
        ConfigCommands::List => config_list()?,
        ConfigCommands::Get { key } => config_get(&key)?,
        ConfigCommands::Set { key, value } => config_set(&key, &value)?,
        ConfigCommands::Edit => config_edit()?,
        ConfigCommands::Path => config_path()?,
    }
}
```

### `crates/merab-cli/src/config_cmd.rs` (nuevo)

Implementar las funciones de config. IMPORTANTE: no usar el RPC daemon para esto — opera directamente sobre el archivo.

```rust
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Retorna la ruta canónica del archivo de config del usuario.
/// Crea el directorio si no existe.
pub fn user_config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("Cannot determine config directory")?
        .join("merab");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.toml"))
}

pub fn config_path() -> Result<()> { ... }
pub fn config_edit() -> Result<()> { ... }  // usa $EDITOR o EDITOR env var
pub fn config_get(key: &str) -> Result<()> { ... }
pub fn config_set(key: &str, value: &str) -> Result<()> { ... }
pub fn config_list() -> Result<()> { ... }
```

#### `config_get` y `config_list`

Usar `merab_config::MerabConfig::load()` para obtener el config merged y extraer el valor por clave con notación de puntos. Para el display, mostrar el valor como string.

Mapeo de claves soportadas (mínimo viable):

```
proxy.api_key
proxy.upstream_url
proxy.port
ai.model
ai.max_tokens
ai.temperature
ai.max_orchestration_steps
ai.max_parallel_tasks
ai.max_fix_cycles
ai.max_retry_attempts
ai.retry_base_delay_ms
daemon.rpc_port
daemon.host
ai.personas.<nombre>.model
ai.personas.<nombre>.api_key
```

#### `config_set`

Leer el archivo TOML existente (o crear uno vacío si no existe), modificar el valor por clave con notación de puntos, y escribirlo de vuelta.

Estrategia: usar `toml_edit` crate para edición preservando comentarios, o parsear/serializar con `toml` + `serde_json::Value` para navegación por clave.

Si `toml_edit` no está en el workspace, usar esta estrategia alternativa:
1. Leer el archivo como `toml::Value` (serde)
2. Navegar la estructura por los segmentos del key (`proxy`, `api_key`)
3. Insertar/reemplazar el valor
4. Serializar y escribir

Para claves anidadas como `ai.personas.coder.model`:
- Segmentos: `["ai", "personas", "coder", "model"]`
- Navegar `config["ai"]["personas"]["coder"]["model"]`

#### `config_edit`

```rust
pub fn config_edit() -> Result<()> {
    let path = user_config_path()?;
    // Crear archivo vacío si no existe
    if !path.exists() {
        std::fs::write(&path, "")?;
    }
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(windows) { "notepad".to_string() }
            else { "nano".to_string() }
        });
    std::process::Command::new(&editor).arg(&path).status()?;
    Ok(())
}
```

### `crates/merab-cli/Cargo.toml`

Agregar la dependencia `toml_edit` para edición que preserva comentarios:

```toml
toml_edit = "0.22"
```

O usar el crate `toml` que ya está en el workspace y construir la edición manualmente. Preferir `toml_edit` si se puede agregar.

También agregar `dirs`:

```toml
dirs = "5"
```

Verificar si `dirs` ya está en el workspace antes de agregar.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-cli/Cargo.toml` | Agregar `toml_edit = "0.22"` y `dirs = "5"` si no están |
| `merab-cli/src/main.rs` | Agregar `ConfigCommands` enum y match arm |
| `merab-cli/src/config_cmd.rs` | Nuevo — implementar las 5 funciones |

## Verificación

```bash
# Sin daemon corriendo:
merab config path           # Debe mostrar la ruta
merab config list           # Debe mostrar todos los valores con defaults
merab config get ai.model   # Debe mostrar el modelo actual
merab config set proxy.api_key TEST_KEY_123
merab config get proxy.api_key   # Debe mostrar TEST_KEY_123
# Verificar que el archivo fue escrito:
cat ~/.config/merab/config.toml   # Debe contener api_key = "TEST_KEY_123"
```

## Notas para el modelo de IA

- **NO requiere daemon**: Este comando opera solo sobre archivos locales.
- `dirs::config_dir()` retorna `C:\Users\<user>\AppData\Roaming` en Windows, `~/.config` en Linux/Mac.
- Para `config_list`, obtener el config merged con `MerabConfig::load()` ya implementado en `merab-config/src/lib.rs`.
- El crate `dirs` puede que ya esté en el workspace. Verificar antes de agregar.
- `toml_edit` vs `toml`: Si es complejo agregar `toml_edit`, implementar con `toml::Value` (ya en workspace). La clave es navegar la estructura con los segmentos del key separados por `.`.
- Para valores que contengan `.` (como URLs), el set solo toma el último segmento como clave.
- Recordar agregar `mod config_cmd;` en `main.rs`.
