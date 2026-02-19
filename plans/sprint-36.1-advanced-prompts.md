# Plan: Mejorar los System Prompts (Continuación a fondo)

El insight de revisar OpenClaw nos llevó a la fuente principal de los prompts de agentes modernos: **Claude Code** (extraídos por repositorios como Piebald-AI).

Las lecciones que vamos a añadir a `prompts.rs` y `planner.rs` son:

1. **Autocorrección Continua**: Los agentes no solo deben ejecutar tareas, deben evaluar los resultados de cada ejecución de herramienta.
2. **Conciencia del Proyecto (Project Memory)**: Obligar al `Engineer` y al `Coder` a buscar archivos como `CLAUDE.md`, `README.md` o `.cursorrules` en el proyecto antes de escribir código.
3. **Instrucciones para el Arquitecto (Planner)**: El Planner ahora será un "Elite AI Agent Architect". Sus tareas generadas incluirán instrucciones estrictas de comprobación para la siguiente persona.

## Tareas de CLI (UI Streaming)

- [x] Eliminar la restricción de 6 tareas en `chat_render.rs` (`tasks.iter().take(6)`).
- [x] Modificar `render_feed` en `chat_render.rs` para que autoscrollee al fondo (auto-scroll to bottom) durante el procesamiento.
- [x] Asegurarse de que `EventKind::Step` y los mensajes del sistema en `chat_handlers.rs` sean visibles en tiempo real actualizando el UI loop adecuadamente.
- [x] Modificar `ENGINEER_SYSTEM_PROMPT` para priorizar la búsqueda de contexto global (`CLAUDE.md`) y usar checks exhaustivos.
- [x] Modificar `CODER_SYSTEM_PROMPT` para implementar la autocorrección obligatoria después de llamadas de `patch`/`write`.
- [x] Implementar Paridad de Streaming Backend en `merab-daemon` y `merab-ai` consumiendo Server-Sent Events de la API de LLM.
- [x] Recompilar y validar cambios.
