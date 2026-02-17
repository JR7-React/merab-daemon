# Sprint 4.0 — AI Coordinator

**Estado**: Completado
**Resultado**: Integración de LLM funcional, nuevos comandos CLI, tests de integración pasando.

## Objetivo
Implementar el Coordinador IA en Forge, permitiendo que el daemon interactúe con modelos de lenguaje (via OpenRouter/Local Proxy) y orqueste herramientas. Crear la infraestructura base para agentes autónomos impulsados por IA.

## Parte 1: Crate `forge-ai`

Se creó un nuevo crate dedicado a la interacción con LLMs.

- **Tipos**: Definición de `ChatMessage`, `ChatRole`, `AiResponse`, `ToolCall`.
- **Cliente**: `AiClient` que se comunica con el proxy local (por defecto `http://localhost:8001`).
- **Error**: `AiError` mapeado a códigos RPC específicos.

## Parte 2: Daemon RPC (`forge-daemon`)

Se expusieron nuevos métodos RPC para capacidades de IA:

- **`forge.ai.chat(message, context_json)`**:
    - Permite conversaciones interactivas.
    - Recibe el contexto (historial de chat) como un string JSON serializado para evitar limitaciones en la macro RPC con tipos complejos anidados.
    - Retorna `AiResponse` con contenido y posibles llamadas a herramientas.

- **`forge.ai.orchestrate(task)`**:
    - Endpoint para tareas "one-shot".
    - Diseñado para recibir una instrucción y decidir el siguiente paso o herramienta a ejecutar.

**Nota Técnica**: La decisión de enviar `context` como `String` (JSON) en `ai_chat` fue tomada tras encontrar problemas de compilación (`E0282`, `E0599`) al intentar usar `Vec<ChatMessage>` o `serde_json::Value` directamente en la firma del macro `#[rpc(server)]`.

## Parte 3: CLI (`forge-cli`)

Nuevos comandos para interactuar con la IA desde la terminal:

- **`forge chat`**:
    - Inicia un bucle REPL (Read-Eval-Print Loop).
    - Mantiene el historial de conversación localmente.
    - Envía mensajes al daemon y muestra respuestas en streaming (simulado por ahora como bloque completo) y uso de herramientas.

- **`forge ask "<pregunta>"`**:
    - Comando rápido para consultas directas.
    - Utiliza `ai_orchestrate`.

## Parte 4: Tests de Integración

Se expandió `crates/forge-daemon/tests/rpc_integration.rs` para incluir:

- **`test_ai_chat_method_exists`**: Verifica que el endpoint `ai.chat` es alcanzable y maneja correctamente la serialización/deserialización del contexto JSON, validando que no retorna "Method not found".
- **`test_ai_orchestrate_method_exists`**: Verifica la disponibilidad del endpoint de orquestación.

## Archivos modificados/creados

| Archivo | Cambio |
|---------|--------|
| `crates/forge-ai/` | **NUEVO**: Crate completo (lib, types, client, error) |
| `crates/forge-config/src/lib.rs` | Agregada configuración `[ai]` |
| `crates/forge-core/src/error.rs` | Agregado `AiError` |
| `crates/forge-daemon/Cargo.toml` | Dependencia `forge-ai` |
| `crates/forge-daemon/src/rpc/mod.rs` | Módulo `ai_methods` |
| `crates/forge-daemon/src/rpc/server.rs` | Implementación endpoints RPC AI |
| `crates/forge-daemon/src/rpc/ai_methods.rs` | **NUEVO**: Lógica de manejo de requests AI |
| `crates/forge-cli/Cargo.toml` | Dependencia `forge-ai` |
| `crates/forge-cli/src/client.rs` | Métodos cliente `ai_chat`, `ai_orchestrate` |
| `crates/forge-cli/src/main.rs` | Comandos `Chat`, `Ask` |
| `crates/forge-daemon/tests/rpc_integration.rs` | Tests de integración AI |

## Verificación

```bash
cargo fmt --all
cargo clippy --workspace
cargo test --workspace
```
Todos los checks y tests pasaron exitosamente.
