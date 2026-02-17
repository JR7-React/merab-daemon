# Sprint 9: Multi-Agent Pipeline & Planning

**Estado**: Completado
**Resultado**: Sistema de personas especializadas y ejecución de planes multi-agente.

## Goal
Implement a sophisticated "Planner" that decomposes complex user requests into a structured plan of subtasks, and assigns these subtasks to specialized agents (personas).

## Tasks
- [x] Create core data structures (`Task`, `Planner`, `Persona`) in `forge-core`
- [x] Register module in `lib.rs`
- [x] Implement `Planner::decompose` using LLM (Sprint 10.0)
- [x] Integrate Planner into `handle_ai_orchestrate` in `forge-daemon`
- [x] Create specialized "persona" prompts (Engineer, Coder, Reviewer, QA)
- [x] Implement the execution loop that iterates through subtasks
- [x] Add `forge.ai.executePlan` RPC method to execute plans with personas

## Personas

| Persona | System Prompt | Use Case |
|---------|---------------|----------|
| **Engineer** | General-purpose AI engineer | Analysis, research, exploration |
| **Coder** | Specialized in writing code | Implementation, features, bug fixes |
| **Reviewer** | Specialized in code review | Quality assurance, finding issues |
| **QA** | Specialized in testing | Writing tests, validation, edge cases |

## Modelos por Persona

Cada persona puede usar un modelo diferente para optimizar costos/calidad:

```toml
[ai]
model = "meta-llama/llama-3.2-3b-instruct:free"  # Default

[ai.personas.engineer]
model = "meta-llama/llama-3.2-3b-instruct:free"  # Rápido para general

[ai.personas.coder]
model = "meta-llama/llama-3.2-3b-instruct:free"  # Rápido para código

[ai.personas.reviewer]
model = "qwen/qwen-2.5-7b-instruct:free"         # Más capaz para reviews

[ai.personas.qa]
model = "google/gemma-2-9b-it:free"              # Balance para tests
```

Los modelos gratuitos actuales de OpenRouter son ideales para estudiantes.

## Architecture

```
User Request → PlannerAgent (LLM)
                    ↓
            Task with subtasks
            (each assigned a persona)
                    ↓
         forge.ai.executePlan
                    ↓
    ┌───────────────────────────────┐
    │ For each subtask:             │
    │   1. Get persona's prompt     │
    │   2. Build AiClient with it   │
    │   3. Execute multi-step loop  │
    │   4. Pass context to next     │
    └───────────────────────────────┘
                    ↓
            ExecutionResult
```

## Files Modified/Created

| File | Change |
|------|--------|
| `crates/forge-core/src/multi_agent_pipeline.rs` | Added `Persona` enum, `persona` field to `Task` |
| `crates/forge-core/src/lib.rs` | Exported `Persona`, `PipelineTask`, `PipelineTaskStatus` |
| `crates/forge-config/src/lib.rs` | Added `PersonaModelConfig`, `personas` field to `AiConfig`, `get_model_for_persona()` |
| `crates/forge-daemon/src/prompts.rs` | Added `CODER_SYSTEM_PROMPT`, `REVIEWER_SYSTEM_PROMPT`, `QA_SYSTEM_PROMPT`, `get_persona_prompt()` |
| `crates/forge-daemon/src/planner.rs` | Updated prompt to assign personas to subtasks |
| `crates/forge-daemon/src/rpc/ai_methods.rs` | Added `handle_ai_execute_plan()` with persona-based execution + model routing |
| `crates/forge-daemon/src/rpc/server.rs` | Added `forge.ai.executePlan` RPC method |
| `forge.toml` | Added persona-specific model configuration |

## RPC Methods Added

| Method | Description |
|--------|-------------|
| `forge.ai.executePlan` | Execute a plan JSON with persona-based subtask execution |

## Usage Example

```bash
# 1. Create a plan
forge plan "Implement a Fibonacci function with tests"

# 2. Execute the plan (each subtask runs with its assigned persona)
forge execute-plan '<plan-json>'
```

## Verification

```bash
cargo build --workspace   # 0 errors
cargo test --workspace    # All tests pass
```
