# Sprint 28 — Persistencia de Chat (`merab chat --resume`)

## Objetivo

`merab chat` guarda el historial de la conversación en la base de datos. Al cerrar y volver a abrir, el usuario puede retomar exactamente donde quedó. Las conversaciones son por proyecto (CWD).

## Problema actual

Cada vez que el usuario abre `merab chat`, empieza desde cero. El LLM no recuerda nada de conversaciones anteriores. Perder contexto en mitad de una sesión de trabajo (cierre accidental, terminal cerrada) obliga a repetir todo el contexto.

## Comportamiento objetivo

```bash
# Chat normal (guarda automáticamente)
$ merab chat
[merab] Chat iniciado. Sesión: chat-abc123
> explica la arquitectura del daemon
El daemon de Merab es un servidor RPC que...
> y el proxy?
El proxy intercepta las llamadas al LLM y...
> /exit

# Reanudar la última conversación del proyecto
$ merab chat --resume
[merab] Retomando sesión chat-abc123 (5 mensajes, hace 2h)
> y cuál es el flujo completo de una tarea?
Continuando desde donde estábamos: el proxy intercepta...

# Listar conversaciones guardadas
$ merab chat --list
ID          FECHA               MSGS   PREVIEW
chat-abc1   2026-02-18 14:30    12     "explica la arquitectura..."
chat-def2   2026-02-17 10:15     8     "cómo añadir un nuevo agente..."

# Reanudar una conversación específica
$ merab chat --resume chat-def2

# Iniciar nueva sesión (ignorar historial)
$ merab chat --new
```

## Implementación

### `crates/merab-store/src/conversations.rs` (nuevo)

Agregar tabla `conversations` y módulo de persistencia:

```sql
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    title       TEXT,   -- preview del primer mensaje
    message_count INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS conversation_messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    role            TEXT NOT NULL,  -- 'user' | 'assistant'
    content         TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

CREATE INDEX IF NOT EXISTS idx_conv_project
    ON conversations(project_path, updated_at DESC);
```

```rust
impl Database {
    pub fn create_conversation(&self, id: &str, project_path: &str) -> Result<(), rusqlite::Error>;
    pub fn add_message(&self, conv_id: &str, role: &str, content: &str) -> Result<(), rusqlite::Error>;
    pub fn get_conversation_messages(&self, conv_id: &str) -> Result<Vec<ConvMessage>, rusqlite::Error>;
    pub fn list_conversations(&self, project_path: &str, limit: u32) -> Result<Vec<ConvSummary>, rusqlite::Error>;
    pub fn get_last_conversation(&self, project_path: &str) -> Result<Option<ConvSummary>, rusqlite::Error>;
    pub fn update_conversation_title(&self, conv_id: &str, title: &str) -> Result<(), rusqlite::Error>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConvMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConvSummary {
    pub id: String,
    pub project_path: String,
    pub title: Option<String>,
    pub message_count: u32,
    pub created_at: String,
    pub updated_at: String,
}
```

### `merab-store/src/lib.rs`

Agregar `pub mod conversations;`

### `merab-store/src/db.rs`

Agregar las nuevas tablas al `migrate()`.

### RPC: Nuevos métodos en el daemon

#### `merab-daemon/src/rpc/server.rs`

Agregar métodos RPC para conversaciones:

- `merab.conv.create(project_path) → ConvSummary`
- `merab.conv.addMessage(conv_id, role, content) → bool`
- `merab.conv.getMessages(conv_id) → Vec<ConvMessage>`
- `merab.conv.list(project_path, limit) → Vec<ConvSummary>`
- `merab.conv.getLast(project_path) → Option<ConvSummary>`

#### `merab-cli/src/client.rs`

Agregar métodos correspondientes:

```rust
pub async fn conv_create(&self, project_path: &str) -> Result<ConvSummary>;
pub async fn conv_add_message(&self, conv_id: &str, role: &str, content: &str) -> Result<bool>;
pub async fn conv_get_messages(&self, conv_id: &str) -> Result<Vec<ConvMessage>>;
pub async fn conv_list(&self, project_path: &str, limit: u32) -> Result<Vec<ConvSummary>>;
pub async fn conv_get_last(&self, project_path: &str) -> Result<Option<ConvSummary>>;
```

### `crates/merab-cli/src/main.rs`

Actualizar `Chat` variant:

```rust
#[command(about = "Interactive AI chat (saves history)")]
Chat {
    /// Resume the last conversation for this project
    #[arg(long)]
    resume: bool,
    /// Resume a specific conversation by ID prefix
    #[arg(long)]
    session: Option<String>,
    /// Start a new conversation (ignore history)
    #[arg(long)]
    new: bool,
    /// List saved conversations
    #[arg(long)]
    list: bool,
},
```

**Lógica del match arm:**

```rust
Commands::Chat { resume, session, new, list } => {
    let ready_client = bootstrap::ensure_ready(&cli.url).await?;

    if list {
        // Mostrar tabla de conversaciones
        let project_path = std::env::current_dir()?.to_string_lossy().to_string();
        let convs = ready_client.conv_list(&project_path, 20).await?;
        print_conv_list(&convs);
        return Ok(());
    }

    // Determinar qué conversación usar
    let conv_id = if new || (!resume && session.is_none()) {
        // Nueva conversación
        let project_path = std::env::current_dir()?.to_string_lossy().to_string();
        let conv = ready_client.conv_create(&project_path).await?;
        println!("[merab] Chat iniciado. Sesión: {}", &conv.id[..8]);
        conv.id
    } else if let Some(ref prefix) = session {
        // Buscar por prefijo
        let project_path = std::env::current_dir()?.to_string_lossy().to_string();
        find_conv_by_prefix(&ready_client, &project_path, prefix).await?
    } else {
        // Retomar la última
        let project_path = std::env::current_dir()?.to_string_lossy().to_string();
        match ready_client.conv_get_last(&project_path).await? {
            Some(conv) => {
                println!("[merab] Retomando sesión {} ({} mensajes)", &conv.id[..8], conv.message_count);
                conv.id
            }
            None => {
                println!("[merab] No hay conversaciones previas. Iniciando nueva...");
                let conv = ready_client.conv_create(&project_path).await?;
                conv.id
            }
        }
    };

    // Cargar historial existente
    let history = ready_client.conv_get_messages(&conv_id).await?;
    let context: Vec<merab_ai::ChatMessage> = history.iter()
        .map(|m| if m.role == "user" {
            merab_ai::ChatMessage::user(&m.content)
        } else {
            merab_ai::ChatMessage::assistant(&m.content)
        })
        .collect();

    // Iniciar chat con historial
    chat_ui::start_chat_session_with_history(&ready_client, &conv_id, context).await?;
}
```

### `crates/merab-cli/src/chat_ui.rs`

Modificar para aceptar historial y guardar mensajes:

```rust
pub async fn start_chat_session_with_history(
    client: &MerabClient,
    conv_id: &str,
    history: Vec<ChatMessage>,
) -> Result<()> {
    // Loop existente de chat, pero:
    // 1. Inicializar `messages` con el historial en vez de vacío
    // 2. Después de cada intercambio (user + assistant), llamar:
    //    client.conv_add_message(conv_id, "user", &user_input).await?;
    //    client.conv_add_message(conv_id, "assistant", &response).await?;
}
```

La función existente `start_chat_session` puede llamar a la nueva con historial vacío.

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-store/src/conversations.rs` | Nuevo — tablas y métodos CRUD |
| `merab-store/src/lib.rs` | Exportar `conversations` |
| `merab-store/src/db.rs` | Agregar tablas en `migrate()` |
| `merab-daemon/src/rpc/server.rs` | 5 nuevos métodos RPC `merab.conv.*` |
| `merab-cli/src/client.rs` | 5 nuevos métodos cliente |
| `merab-cli/src/main.rs` | Actualizar `Chat` variant con flags |
| `merab-cli/src/chat_ui.rs` | Modificar para persistencia |

## Verificación

```bash
# Nueva conversación:
merab chat
> di hola
Hola! ¿En qué puedo ayudarte?
> /exit

# Retomar:
merab chat --resume
# Debe ver el "Hola!" previo en contexto
> ¿qué te dije antes?
# El LLM debe recordar "di hola"

# Listar:
merab chat --list
# Debe mostrar la conversación recién creada
```

## Notas para el modelo de IA

- **Migración de DB**: Agregar las tablas en el `migrate()` existente de `db.rs`. Usar `CREATE TABLE IF NOT EXISTS` para idempotencia.
- **IDs de conversación**: Usar UUID v4 como ya se hace con sesiones.
- **Título**: El primer mensaje del usuario (truncado a 50 chars) se usa como título. Actualizar con `update_conversation_title` después del primer intercambio.
- **Límite de historial**: En `start_chat_session_with_history`, si el historial tiene más de 50 mensajes, usar solo los últimos 20 para no saturar el contexto. Informar al usuario.
- **`start_chat_session` existente**: Mantenerla funcionando (sin persistencia) para compatibilidad. La nueva función es `start_chat_session_with_history`.
- El patrón de los RPC methods es idéntico a `merab.session.*` ya implementado — copiar exactamente la misma estructura.
