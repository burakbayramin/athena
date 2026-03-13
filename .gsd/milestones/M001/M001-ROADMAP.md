# M001: Migration

**Vision:** Athena is a Rust CLI tool that acts as an AI orchestra conductor — it takes a software project idea (natural language, spec document, or existing codebase), analyzes it into smart development phases, and autonomously executes those phases by coordinating three AI agents (Claude, Gemini, Codex/Copilot) working in parallel on isolated modules.

## Success Criteria


## Slices

- [x] **S01: Foundation** `risk:medium` `depends:[]`
  > After this: Create the Cargo workspace scaffold with all 7 ath-* crates, workspace-level dependency management, and minimal compilable stubs for each crate.
- [x] **S02: Agent Clients** `risk:medium` `depends:[S01]`
  > After this: Define the foundational contracts for the agent layer: the AgentError enum, CircuitBreaker state machine, AgentBackend trait, and MockBackend test double.
- [x] **S03: Git Layer** `risk:medium` `depends:[S02]`
  > After this: Set up ath-git crate with dependencies, error types, commit message builder, and GitLayer struct with repository management.
- [x] **S04: Input Parsing** `risk:medium` `depends:[S03]`
  > After this: Set up the input parsing infrastructure: InputMode enum, InputError type, CLI flag extensions, and AgentRequest json_schema field.
- [x] **S05: Phase Decomposition** `risk:medium` `depends:[S04]`
  > After this: Plan types, DAG algorithms, and error types for phase decomposition.
- [x] **S06: Module Isolation** `risk:medium` `depends:[S05]`
  > After this: Create the foundation for module isolation: add the assigned_agent field to TaskSpec, define IsolationError types, and implement the static skill taxonomy routing table.
- [x] **S07: Phase Runner And Review** `risk:medium` `depends:[S06]`
  > After this: Create the PhaseState typestate machine and supporting error types for the phase runner.
- [x] **S08: Cli And Progress** `risk:medium` `depends:[S07]`
  > After this: Refactor the CLI surface so `ath`, `ath run`, `ath init`, and `ath report` have a stable, testable command shape that matches the locked Phase 8 UX decisions.
- [x] **S09: Reporting And Error Quality** `risk:medium` `depends:[S08]`
  > After this: Introduce Athena's durable run-report artifact so every successful run leaves behind structured output that `ath report` can load later.
- [x] **S10: Parallel Execution** `risk:medium` `depends:[S09]`
  > After this: Create four failing test stubs in coordinator.
