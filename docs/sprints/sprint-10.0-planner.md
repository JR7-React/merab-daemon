# Sprint 10: Planificador Inteligente (LLM)

## Objetivo
Reemplazar el planificador heurístico "hardcoded" con un planificador basado en LLM real que analice las descripciones de las tareas y genere planes de ejecución estructurados y conscientes del contexto.

## Implementación

### 1. `PlannerAgent` (`crates/forge-daemon/src/planner.rs`)
- Se creó una estructura de agente especializada `PlannerAgent` que envuelve a `AiClient`.
- Se definió `PLANNER_SYSTEM_PROMPT` instruyendo a la IA para actuar como un Project Manager experto y generar un JSON estrictamente válido que coincida con el esquema de `Task`.
- Se implementó el método `decompose()` que:
    - Envía la descripción de la tarea del usuario al LLM.
    - Extrae el bloque JSON de la respuesta.
    - Deserializa el JSON en un árbol de estructuras `Task`.

### 2. Integración (`crates/forge-daemon/src/rpc/ai_methods.rs`)
- Se actualizaron `handle_ai_plan` (RPC) y `execute_tool_call` (manejo de herramientas internas) para usar `PlannerAgent`.
- Se eliminó la dependencia del antiguo `Planner` hardcoded.

## Verificación

### Prueba Manual
Se ejecutó `forge plan "Create a simple Python script to calculate Fibonacci sequence"`.

**Resultado:**
El sistema devolvió un plan JSON con sub-tareas específicas como:
1.  "Implementar un bucle o recursión para generar la secuencia de Fibonacci"
2.  "Probar el script con diferentes valores de entrada"

Esto confirma que el planificador es **inteligente** y se adapta a la petición específica, en lugar de devolver marcadores genéricos como "Investigación/Implementación".

### Uso Autónomo
Dado que `core.plan` ya se integró en el Sprint 9.2, la herramienta autónoma (`forge ask`) ahora se beneficia automáticamente de esta inteligencia. Cuando decide planificar, obtiene un desglose real y útil del problema.
