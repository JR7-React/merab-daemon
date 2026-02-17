# Sprint 8.0 — Interactive Chat TUI (forge-cli)

**Estado**: Completado
**Resultado**: `forge chat` ahora tiene una interfaz interactiva tipo "Claude-Code".

## Objetivo
Mejorar la experiencia de usuario (UX) del chat con la IA, pasando de una CLI básica a una TUI rica con soporte de Markdown, historial y feedback visual.

## Componentes Implementados

### 1. Interactive Chat Loop (`crates/forge-cli/src/chat_ui.rs`)
- **Rustyline**: Implementado para input con historial (flechas arriba/abajo), edición de línea y atajos (Ctrl-C, Ctrl-D).
- **Mascot Splash**: Se muestra la mascota de Forge al iniciar el chat (arte ASCII coloreado).
- **Markdown Rendering**: Integración de `termimad` para renderizar las respuestas de la IA con formato (negritas, código, tablas).

### 2. Comandos Slash
- `/reset`: Limpia el contexto de la conversación.
- `/q`, `quit`, `exit`: Sale del chat.

### 3. Dependencias Nuevas
- `rustyline`: Manejo de input readline avanzado.
- `termimad`: Motor de renderizado Markdown para terminal.
- `crossterm`: Control de colores y cursor.
- `ratatui`: Tipos para la mascota (reutilizada de forge-tui).

## Uso
```bash
forge chat
```
- Escribe tu consulta.
- Usa `ArrowUp`/`ArrowDown` para navegar el historial.
- `/reset` para olvidar la conversación actual.
- `Ctrl-C` o `quit` para salir.

## Archivos Clave
- `crates/forge-cli/src/chat_ui.rs`: Lógica de la interfaz de usuario.
- `crates/forge-cli/src/main.rs`: Integración con el comando CLI.
- `crates/forge-tui/src/lib.rs`: Exposición del módulo `splash`.
