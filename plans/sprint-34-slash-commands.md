# Sprint 34: Slash Commands en el Chat Interactivo

## Contexto

El chat interactivo de Merab (`merab chat`) tiene 3 slash commands hardcodeados (`/quit`, `/reset`, `/clear`) en `chat_ui.rs:231-248`. El usuario debe salir del chat para usar comandos como `merab doctor`, `merab sessions`, `merab index build`, etc. Esto es engorroso.

**Objetivo:** Sistema extensible de slash commands dentro del chat, con autocompletado al escribir `/`, similar a Claude Code. El usuario nunca sale del chat para operaciones comunes.

---

## Archivos a crear

| Archivo | ~Líneas | Contenido |
|---------|---------|-----------|
| `crates/merab-cli/src/slash_commands/mod.rs` | ~130 | `SlashCommandDef`, registro con `LazyLock`, `parse_slash_input()`, `resolve_command()`, `get_completions()`, `execute_slash_command()` dispatch |
| `crates/merab-cli/src/slash_commands/help.rs` | ~30 | Handler `/help` — lista todos los slash commands |
| `crates/merab-cli/src/slash_commands/quit.rs` | ~10 | Handler `/quit` |
| `crates/merab-cli/src/slash_commands/clear.rs` | ~15 | Handler `/clear` |
| `crates/merab-cli/src/slash_commands/reset.rs` | ~15 | Handler `/reset` |
| `crates/merab-cli/src/slash_commands/sessions.rs` | ~30 | Handler `/sessions` — lista sesiones vía RPC |
| `crates/merab-cli/src/slash_commands/cont.rs` | ~30 | Handler `/continue` — carga sesión anterior |
| `crates/merab-cli/src/slash_commands/doctor.rs` | ~25 | Handler `/doctor` — health check inline |
| `crates/merab-cli/src/slash_commands/index.rs` | ~20 | Handler `/index` — indexa proyecto vía RPC |
| `crates/merab-cli/src/slash_commands/search.rs` | ~30 | Handler `/search` — busca símbolos vía RPC |
| `crates/merab-cli/src/slash_commands/stats.rs` | ~25 | Handler `/stats` — estadísticas del proyecto |
| `crates/merab-cli/src/slash_commands/context.rs` | ~20 | Handler `/context` — info del proyecto detectado |
| `crates/merab-cli/src/slash_commands/jobs.rs` | ~25 | Handler `/jobs` — lista jobs background |
| `crates/merab-cli/src/slash_commands/config.rs` | ~20 | Handler `/config` — muestra configuración actual |

## Archivos a modificar

| Archivo | Cambio |
|---------|--------|
| `crates/merab-cli/src/main.rs` | Agregar `mod slash_commands;` |
| `crates/merab-cli/src/chat_ui.rs` | Reemplazar slash commands hardcodeados por dispatch al nuevo sistema. Agregar estado de autocompletado + lógica Tab. Agregar llamada a `render_slash_popup()` en el draw |
| `crates/merab-cli/src/chat_render.rs` | Agregar rendering de `ChatRole::System` (amarillo) en `render_feed()`. Agregar función `render_slash_popup()` |

---

## Diseño

### `SlashCommandDef` + registro estático

```rust
pub struct SlashCommandDef {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage: &'static str,
    pub needs_args: bool,
}

static COMMANDS: LazyLock<Vec<SlashCommandDef>> = LazyLock::new(|| vec![...]);
```

### `SlashCommandResult`

```rust
pub enum SlashCommandResult {
    Output(Vec<String>),  // líneas a mostrar como mensaje System en el feed
    Exit,                 // salir del chat
    Silent,               // no mostrar nada (ej: /clear)
    Error(String),        // mostrar error
}
```

### Comandos a implementar (un archivo por handler)

| Comando | Alias | Args | Archivo handler |
|---------|-------|------|-----------------|
| `/help` | `/h` | no | `help.rs` |
| `/quit` | `/q`, `/exit` | no | `quit.rs` |
| `/clear` | — | no | `clear.rs` |
| `/reset` | — | no | `reset.rs` |
| `/sessions` | — | no | `sessions.rs` |
| `/continue` | — | `[id]` | `cont.rs` |
| `/doctor` | — | no | `doctor.rs` |
| `/index` | — | no | `index.rs` |
| `/search` | `/s` | `<query>` | `search.rs` |
| `/stats` | — | no | `stats.rs` |
| `/context` | `/ctx` | no | `context.rs` |
| `/jobs` | — | no | `jobs.rs` |
| `/config` | `/cfg` | no | `config.rs` |

Cada handler es una función `pub async fn` que recibe los parámetros necesarios (`client`, `args`, etc.) y retorna `SlashCommandResult`. El dispatch en `mod.rs` hace match del nombre resuelto y llama al handler correspondiente.

### Estructura de un handler típico

```rust
// slash_commands/sessions.rs
use crate::client::MerabClient;
use super::SlashCommandResult;

pub async fn handle(client: &MerabClient) -> SlashCommandResult {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    match client.session_list(&cwd, 10).await {
        Ok(sessions) => {
            let mut lines = vec!["Sesiones recientes:".to_string()];
            for s in &sessions {
                lines.push(format!("  {} — {} [{}]", s.id, s.task, s.status));
            }
            if sessions.is_empty() {
                lines.push("  (ninguna)".to_string());
            }
            SlashCommandResult::Output(lines)
        }
        Err(e) => SlashCommandResult::Error(format!("No se pudieron listar sesiones: {e}")),
    }
}
```

### Autocompletado

- Cuando input empieza con `/` y tiene >1 char → filtrar comandos por prefijo
- Tab → acepta la sugerencia seleccionada
- Popup renderizado encima del input con `render_slash_popup()`
- Estilo: borde cyan, selección resaltada

### Output inline

- Los resultados se agregan al feed como `ChatMessage::system(content)`
- `render_feed()` renderiza `ChatRole::System` con estilo amarillo (╭── System)
- Los mensajes System NO se persisten en la conversación (no se llama `conv_add_message`)

---

## Orden de implementación

1. **`slash_commands/mod.rs`** — Tipos, registro, parse, resolve, completions, dispatch
2. **`slash_commands/*.rs`** — Un archivo por handler (help, quit, clear, reset, sessions, cont, doctor, index, search, stats, context, jobs, config)
3. **`chat_render.rs`** — Rendering de System + popup de autocompletado
4. **`chat_ui.rs`** — Reemplazar slash commands hardcodeados, agregar autocompletado
5. **`main.rs`** — `mod slash_commands;`
6. **Verificar** — `cargo build && cargo test`

---

## Integración en `chat_ui.rs`

En `handle_enter()` (línea 231-248), reemplazar los 3 bloques `if` por:

```rust
if input.starts_with('/') {
    if let Some((cmd, args)) = slash_commands::parse_slash_input(input) {
        let result = match slash_commands::resolve_command(cmd) {
            Some(name) => slash_commands::execute_slash_command(
                name, args, client, context, messages, tasks, scroll_offset,
            ).await,
            None => SlashCommandResult::Error(format!("Comando desconocido: /{cmd}. Usa /help.")),
        };
        match result {
            SlashCommandResult::Exit => return Ok(true),
            SlashCommandResult::Output(lines) => {
                messages.push(ChatMessage::system(&lines.join("\n")));
            }
            SlashCommandResult::Error(msg) => {
                messages.push(ChatMessage::system(&format!("Error: {msg}")));
            }
            SlashCommandResult::Silent => {}
        }
        input.clear();
        return Ok(false);
    }
}
```

Nota: en vez de crear un `ChatState` struct (que requeriría refactorizar toda la función), los handlers async reciben directamente los parámetros que necesitan (`client`, `context`, etc). Esto minimiza el diff.

En `run_app()`, agregar al key handler:
- Calcular completions cuando input empieza con `/`
- Tab para aceptar completion
- Llamar a `render_slash_popup()` en el draw closure cuando hay completions

---

## Verificación

```bash
cargo build    # compilación limpia
cargo test     # 74+ tests, 0 failures

# Test manual:
merab chat
> /help            # tabla de comandos
> /sessions        # lista sesiones
> /doctor          # health check inline
> /search Client   # busca símbolos
> /se<Tab>         # autocompleta a /search
> /unknown         # error: comando desconocido
> hola             # chat normal sigue funcionando
> /quit            # sale
```
