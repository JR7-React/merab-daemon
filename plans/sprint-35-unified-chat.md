# Sprint 35: Unified Agentic Chat — Orquestación completa en TUI

## Problema

`merab chat` usa `ai_chat` (loop simple de 10 pasos con tool calls). `merab ask` usa `ai_orchestrate_stream` (Planner → DAG → personas → artifacts → sessions). Son dos motores completamente distintos. El chat es inútil comparado con `merab ask`.

**Objetivo:** Eliminar el motor simple del chat. `merab chat` debe usar el pipeline completo de orquestación (`ai_orchestrate_stream`) con visualización en tiempo real del plan, subtasks y personas en la TUI.

---

## Cambio central

Reemplazar `process_message()` en `chat_ui.rs` (loop local de `ai_chat` + tool calls) por una llamada a `client.ai_orchestrate_stream()` con event tailing a un canal `mpsc` en vez de stdout.

### Antes (actual)

```
handle_enter → process_message → client.ai_chat() → loop 10 pasos → tool_call manual
```

### Después

```
handle_enter → client.ai_orchestrate_stream(prompt, event_file)
             → spawn tail_events_to_channel(event_file, tx)
             → poll rx + terminal events en loop
             → cada ProgressEvent actualiza sidebar/feed en tiempo real
             → al terminar: muestra respuesta + artifacts + tokens
```

---

## Archivos a crear

| Archivo | ~Líneas | Contenido |
|---------|---------|-----------|
| `crates/merab-cli/src/event_channel.rs` | ~80 | `tail_events_to_channel()` — variante de `event_tail` que envía `ProgressEvent` por `mpsc::Sender` en vez de imprimir a stdout |

## Archivos a modificar

| Archivo | Cambio |
|---------|--------|
| `crates/merab-cli/src/chat_ui.rs` | Reemplazar `process_message()` por orquestación con event channel. Nuevo loop async que poll rx + terminal. |
| `crates/merab-cli/src/chat_render.rs` | Renderizar persona activa y modelo en sidebar tasks. Nuevo estilo para eventos de progreso. |
| `crates/merab-cli/src/main.rs` | Agregar `mod event_channel;` |

## Archivos a eliminar (código muerto)

| Código | Razón |
|--------|-------|
| `process_message()` en `chat_ui.rs` | Reemplazada por orquestación |

---

## Diseño detallado

### 1. `event_channel.rs` — Event tailing a mpsc

Reutiliza la lógica de `event_tail.rs` pero en vez de `display_event()` envía por canal.

```rust
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use merab_core::ProgressEvent;

use crate::event_tail::parse_event_line;

pub fn tail_events_to_channel(
    path: PathBuf,
    tx: std::sync::mpsc::Sender<ProgressEvent>,
    running: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut last_pos: u64 = 0;

        while running.load(Ordering::Relaxed) {
            if let Ok(file) = File::open(&path) {
                if let Ok(metadata) = file.metadata() {
                    if metadata.len() > last_pos {
                        if let Ok(mut file) = File::open(&path) {
                            let _ = file.seek(SeekFrom::Start(last_pos));
                            let reader = BufReader::new(file);
                            for line in reader.lines().flatten() {
                                if let Some(event) = parse_event_line(&line) {
                                    let is_done = event.kind == merab_core::EventKind::Done;
                                    let _ = tx.send(event);
                                    if is_done {
                                        running.store(false, Ordering::Relaxed);
                                        return;
                                    }
                                }
                            }
                        }
                        if let Ok(file) = File::open(&path) {
                            if let Ok(metadata) = file.metadata() {
                                last_pos = metadata.len();
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(80));
        }
    })
}
```

**Nota:** `parse_event_line` se hace `pub(crate)` en `event_tail.rs` (ya lo es).

### 2. Refactor de `handle_enter()` en `chat_ui.rs`

Reemplazar todo el bloque post-slash-commands por:

```rust
// 1. Construir prompt con historial de contexto
let full_prompt = if context.is_empty() {
    user_msg.clone()
} else {
    let history = context.iter()
        .map(|m| format!("[{}]: {}", match m.role {
            ChatRole::User => "User",
            ChatRole::Assistant => "Assistant",
            _ => "System",
        }, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Contexto previo de la conversación:\n{}\n\nTarea actual:\n{}", history, user_msg)
};

// 2. Crear event file temporal
let event_file = std::env::temp_dir()
    .join(format!("merab-chat-events-{}.jsonl", std::process::id()));
let event_file_str = event_file.to_string_lossy().to_string();

// 3. Lanzar orquestación en tokio task
let client_url = client.url().to_string();  // necesario para clonar
let task_str = full_prompt.clone();
let ef = event_file_str.clone();
let orchestrate_handle = tokio::spawn(async move {
    let c = MerabClient::new(&client_url).ok()?;
    c.ai_orchestrate_stream(&task_str, &ef).await.ok()
});

// 4. Lanzar event tail a channel
let (tx, rx) = std::sync::mpsc::channel::<ProgressEvent>();
let running = Arc::new(AtomicBool::new(true));
let tail_handle = event_channel::tail_events_to_channel(
    event_file.clone(), tx, running.clone(),
);

// 5. Loop: poll eventos + terminal events hasta que orchestrate termine
loop {
    // Procesar eventos del pipeline (non-blocking)
    while let Ok(event) = rx.try_recv() {
        match event.kind {
            EventKind::Planning => {
                tasks.push((false, "Planning...".to_string()));
            }
            EventKind::PlanReady => {
                if let Some(last) = tasks.last_mut() { last.0 = true; }
            }
            EventKind::SubtaskStart => {
                let persona = event.persona.as_deref().unwrap_or("Agent");
                tasks.push((false, format!("[{}] {}", persona, truncate(&event.message, 30))));
            }
            EventKind::Step => {
                let persona = event.persona.as_deref().unwrap_or("Agent");
                messages.push(ChatMessage::system(&format!("[{}] {}", persona, event.message)));
            }
            EventKind::SubtaskDone => {
                if let Some(last) = tasks.last_mut() { last.0 = true; }
            }
            EventKind::Error => {
                messages.push(ChatMessage::system(&format!("Error: {}", event.message)));
            }
            EventKind::Retrying => {
                messages.push(ChatMessage::system(&format!("⟳ {}", event.message)));
            }
            EventKind::Done => { /* handled by orchestrate future */ }
        }
    }

    // Redibujar TUI
    terminal.draw(|f| { /* layout + render_feed + render_sidebar + render_input */ })?;

    // Poll terminal events (short timeout para no bloquear)
    if event::poll(Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            // Solo permitir Ctrl+C para cancelar, scroll, etc.
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                running.store(false, Ordering::Relaxed);
                orchestrate_handle.abort();
                break;
            }
            // Scroll sigue funcionando durante procesamiento
            // ...
        }
    }

    // Verificar si la orquestación terminó
    if orchestrate_handle.is_finished() {
        break;
    }
}

// 6. Recoger resultado
running.store(false, Ordering::Relaxed);
let _ = tail_handle.join();
let _ = std::fs::remove_file(&event_file);

match orchestrate_handle.await {
    Ok(Some(resp)) => {
        messages.push(ChatMessage::assistant(&resp.content));
        context.push(ChatMessage::user(&user_msg));
        context.push(ChatMessage::assistant(&resp.content));

        if let Some(usage) = &resp.usage {
            *tokens_used += usage.input + usage.output;
            messages.push(ChatMessage::system(&format!(
                "Tokens: {} in / {} out | Modelo: {}",
                usage.input, usage.output, usage.model
            )));
        }
        if let Some(artifacts) = &resp.artifacts {
            let mut artifact_lines = Vec::new();
            for f in &artifacts.files_created { artifact_lines.push(format!("  + {}", f)); }
            for f in &artifacts.files_modified { artifact_lines.push(format!("  ~ {}", f)); }
            if !artifact_lines.is_empty() {
                messages.push(ChatMessage::system(&format!("Archivos:\n{}", artifact_lines.join("\n"))));
            }
        }

        // Persistir en conversación
        if !conv_id.is_empty() {
            let _ = client.conv_add_message(conv_id, "user", &user_msg).await;
            let _ = client.conv_add_message(conv_id, "assistant", &resp.content).await;
        }
    }
    _ => {
        messages.push(ChatMessage::system("Error: La orquestación falló o fue cancelada."));
    }
}

if let Some(last) = tasks.last_mut() { last.0 = true; }
*is_processing = false;
```

### 3. Visualización de personas y modelo en sidebar

En `chat_render.rs`, el sidebar ya muestra tasks con `✓`/`·`. Los eventos de progreso agregan tasks dinámicamente con formato:

```
Tasks
  ✓ Initialize environment
  ✓ Load MCP agents
  ✓ Planning...
  · [ENGINEER] Analizar estructura
  · [CODER] Implementar función
```

El modelo se muestra en el token summary del System message:
```
╭── System
│  Tokens: 1,234 in / 567 out | Modelo: claude-sonnet-4-20250514
╰──
```

### 4. Necesidad: `MerabClient::url()`

El `MerabClient` necesita exponer su URL para poder clonar el client en el tokio::spawn (ya que `HttpClient` no es `Send` entre tasks fácilmente). Agregar:

```rust
// client.rs
impl MerabClient {
    pub fn url(&self) -> &str { &self.url }
    // ... y guardar url como campo
}
```

Alternativa: pasar `url: String` como parámetro a `handle_enter` (ya se tiene en `run_app` → viene de `start_chat_session_with_history` → viene de `chat_cmd`).

---

## Qué se elimina

- `process_message()` completa (~35 líneas) — ya no se necesita el loop de `ai_chat` + tool calls manuales
- El import de `ai_chat` si ya no lo usa nadie más en el CLI

---

## Edge cases

| Caso | Manejo |
|------|--------|
| Ctrl+C durante orquestación | `running.store(false)` + `orchestrate_handle.abort()` |
| Daemon no responde | El `ai_orchestrate_stream` falla → se muestra error en feed |
| Tarea simple ("hola") | El planner genera 1 subtask trivial → funciona igual |
| Historial largo | Se trunca `context` a últimos N mensajes (o últimos N tokens) |
| Chat sin daemon | Ya lo maneja `start_chat_session` — el error sale antes |

---

## Orden de implementación

1. **`client.rs`** — Agregar campo `url` + método `url()` a `MerabClient`
2. **`event_channel.rs`** — Crear módulo con `tail_events_to_channel()`
3. **`main.rs`** — Agregar `mod event_channel;`
4. **`chat_ui.rs`** — Reemplazar `handle_enter` post-slash con loop de orquestación. Eliminar `process_message()`.
5. **`chat_render.rs`** — Ajustar sidebar para mostrar persona tags dinámicos
6. **Verificar** — `cargo build && cargo test`

---

## Verificación

```bash
cargo build    # compilación limpia
cargo test     # 0 failures

# Test manual:
merab chat
> Crea una función que calcule fibonacci
# Debe verse en sidebar:
#   ✓ Planning...
#   ✓ [ENGINEER] Analizar requerimiento
#   · [CODER] Implementar función
# En feed:
#   ╭── System
#   │  [CODER] Escribiendo función fibonacci...
#   ╰──
#   ╭── Merab
#   │  (respuesta final con el código)
#   ╰──
#   ╭── System
#   │  Tokens: 1,234 in / 567 out | Modelo: claude-sonnet-4-20250514
#   │  Archivos:
#   │    + src/fibonacci.rs
#   ╰──

> /sessions    # debe mostrar la sesión que se acaba de guardar
> /stats       # tokens acumulados
> hola         # tarea simple, planner genera 1 subtask
> Ctrl+C       # cancela orquestación en progreso
> /quit        # sale
```

---

## Notas

- **No se toca el daemon.** El pipeline ya existe completo en `ai_orchestrate_stream`. Solo cambia el consumidor (de stdout a TUI).
- **`merab ask` sigue funcionando** igual para uso no-interactivo (scripts, CI).
- **`event_tail.rs` se mantiene** — lo usa `merab ask`. El nuevo `event_channel.rs` es su gemelo para TUI.
- **Dificultad:** Media-Alta. El loop async que combina `mpsc::recv` + `crossterm::event::poll` + `tokio::spawn` es la parte delicada.
