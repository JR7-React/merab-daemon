# Sprint 9: Multi-Agent Pipeline & Planning

## Goal
Implement a sophisticated "Planner" that decomposes complex user requests into a structured plan of subtasks, and assigns these subtasks to specialized agents (or the same generalist agent in a loop).

## Tasks
- [x] Create core data structures (`Task`, `Planner`) in `forge-core`
- [x] Register module in `lib.rs`
- [ ] Implement `Planner::decompose` using LLM (instead of hardcoded)
- [ ] Integrate Planner into `handle_ai_orchestrate` in `forge-daemon`
- [ ] Create specialized "persona" prompts (Coder, Reviewer, QA)
- [ ] Implement the execution loop that iterates through subtasks
- [ ] Add `forge.ai.plan` RPC method to expose planning capability
