# Sprint 2.2 — Message Passing

**Estado**: Completado
**Commit**: `c223911`

## Objetivo
Sistema de mensajería entre agentes usando el daemon como broker JSON-RPC. Soporte para mensajes directos y broadcast.

## Tipos (forge-core/src/message.rs)

```rust
pub enum MessageStatus { Pending, Delivered, Acknowledged }

pub struct Message {
    pub id: Uuid,
    pub from_agent: AgentId,
    pub to_agent: Option<AgentId>,  // None = broadcast
    pub content: String,
    pub status: MessageStatus,
    pub created_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}
```

## RPC Methods agregados

| Method | Params | Returns | Descripción |
|---|---|---|---|
| `forge.sendMessage` | `from, to, content` | `Message` | Envía mensaje directo |
| `forge.broadcastMessage` | `from, content` | `Message` | Broadcast a todos |
| `forge.getMessages` | `agent_id` | `Vec<Message>` | Mensajes pendientes para un agente |
| `forge.ackMessage` | `message_id` | `bool` | Marcar mensaje como acknowledged |

## CLI Commands agregados

```bash
forge send <from_id> <to_id> "contenido"
forge broadcast <from_id> "contenido"
forge messages <agent_id>
forge ack <message_id>
```

## SQLite — Tabla messages

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    from_agent TEXT NOT NULL,
    to_agent TEXT,                -- NULL = broadcast
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    acknowledged_at TEXT
)
```

## Store methods agregados

- `create_messages_table()` — Llamado en `Database::open()`
- `insert_message(msg)`
- `get_messages_for(agent_id)` — Retorna mensajes directos + broadcasts pendientes
- `acknowledge_message(message_id) -> bool`
- `delete_agent_messages(agent_id)` — Limpieza al unregister

## Flujo

1. Agente A envía mensaje a Agente B via `forge.sendMessage`
2. Daemon valida que ambos agentes existen
3. Mensaje se persiste en SQLite con status Pending
4. Agente B consulta mensajes via `forge.getMessages`
5. Agente B confirma recepción via `forge.ackMessage`
