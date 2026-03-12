# Feature Research

**Domain:** Multi-agent AI orchestration CLI (code generation / software development automation)
**Researched:** 2026-03-12
**Confidence:** MEDIUM-HIGH (ecosystem rapidly evolving; core feature categories stable, specific tooling details may shift)

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete or untrustworthy.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Natural language project input | Entry point for any AI dev tool — users expect to describe what they want, not write config files | LOW | Parse free-form description into structured intent; the hard part is structured output from the LLM, not the CLI input itself |
| Structured phase/task decomposition | Every competing tool (MetaGPT, CrewAI, LangGraph) breaks work into discrete steps — users know this pattern | HIGH | This is Athena's core value prop; getting dependency ordering right is the hard problem |
| Named agent roles with explicit assignments | CrewAI popularized role-based agents (Product Manager, Architect, Engineer, QA); users now expect this | MEDIUM | Maps naturally to Claude/Gemini/Codex specialization model |
| Per-phase git commits | Non-negotiable for any code generation tool — users need to track what changed and when | LOW | Straightforward `git commit` after each phase; commit message should include phase name and agent |
| API key configuration via env vars and config file | Every CLI tool that touches external APIs supports both; missing either is a friction point | LOW | `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`; support `.env` file and `~/.athena/config.toml` |
| Visible progress reporting | Users firing off a long-running multi-agent task need to know it's working, not hung | MEDIUM | Real-time phase progress, current agent, current task; terminal-friendly output (not just final report) |
| Structured final report | MetaGPT, CrewAI all produce summary output; users expect an artifact they can inspect | LOW | Phase table, agent assignments, review results, outcome per phase |
| Error reporting with actionable context | When an agent fails or a review blocks, users need to know what failed and why | MEDIUM | Distinguish API errors, review failures, and malformed outputs; surface the specific blocking reason |
| Typed/validated inter-agent communication | GitHub Engineering Blog (2025): unstructured data exchange is the #1 cause of multi-agent workflow failure | HIGH | Agents must exchange structured JSON schemas, not raw prose; validated at every boundary |
| Phase-level review/quality gate | The cross-agent review pattern (Generator + Critic) is now a documented standard pattern (Google ADK, LangGraph, MetaGPT) | HIGH | A different agent reviews each phase output before progression; blocking gate, not advisory |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valued when present.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Intelligent dependency analysis | Most tools execute sequentially or manually-specified DAGs; auto-detecting which modules depend on which is novel | HIGH | Static analysis of the project spec + LLM-assisted dependency inference; prevents incorrect parallelization |
| Skill-based agent routing | Athena assigns tasks to Claude vs Gemini vs Codex based on task type (architecture, research, boilerplate) — no competitor does this with heterogeneous models | HIGH | Requires a routing layer with a skill taxonomy and model capability map that must be maintained as models evolve |
| True module isolation per agent | Git worktrees per agent ensure zero file-level merge conflicts during parallel execution — this is emerging (ccswarm, Claude Code) but not standard | MEDIUM | Each agent owns a strict file list; orchestrator enforces boundaries; worktrees or separate working dirs |
| Cross-model peer review | Agent A's output reviewed by Agent B (different vendor, different model) catches blind spots single-model self-review misses | MEDIUM | Already planned in PROJECT.md; the differentiator is cross-vendor not just cross-agent |
| Automatic retry loop on review failure | Phase blocked? Athena automatically re-prompts the original agent with the reviewer's feedback until resolved or max-retries hit | MEDIUM | Most tools require human intervention to unblock; fully autonomous retry is a DX win |
| Dry-run / planning mode | Show the proposed phase plan, agent assignments, and dependency graph without executing — lets users inspect and validate before incurring API cost | LOW | Overstory (open source orchestrator) has this; users cited cost opacity as a top pain point |
| Token usage and cost reporting | Per-phase token counts and estimated cost per agent; surfaces what each phase actually cost | MEDIUM | Users cited pricing opacity as a major frustration (Stack Overflow 2025 survey); BYO-key tools should be maximally transparent |
| Parallelization of independent phases | 2025 research shows ~1.3x speedup from parallel agent execution; users with background execution aspirations value this | HIGH | Requires accurate dependency analysis first; unsafe parallelization causes integration failures |
| Spec-driven input (file-based) | Kiro, Tessl, GitHub Spec Kit trend: `requirements.md`, `design.md`, `tasks.md` as orchestration input; resonates with structured developers | LOW | Accept both free-form text and structured spec files as project input |
| Resume interrupted execution | LangGraph checkpointing popularized this; long runs can be interrupted by API timeouts, rate limits, or user Ctrl-C | HIGH | Requires persistent phase state between runs; SQLite or local checkpoint file |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems for Athena's scope and v1 goals.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Web dashboard / visual workflow UI | Developers want to "see" agent activity; CrewAI's Agent Operations Platform, LangGraph Studio both exist | Violates v1 CLI-only scope; massive scope expansion; adds auth, server, frontend maintenance burden; 83% of multi-agent users are senior+ engineers already comfortable with terminal output | Rich terminal output with structured tables; dry-run mode prints the DAG as ASCII; final report is human-readable |
| Built-in API key management / key rotation | Users ask for managed secrets to simplify setup | Requires a secrets service, encryption at rest, and a trust model; fundamentally a SaaS feature; creates liability for key security | Env vars + config file; document `.env` gitignore best practices; let users use their own secrets manager |
| Real-time agent conversation visibility (full chat transcript) | AutoGen's transparent turn-by-turn logs are appealing for debugging | Generates enormous terminal noise for non-power-users; token-level streaming from three concurrent agents is unreadable; bloats log files | Verbose flag `--verbose` shows full transcripts on demand; default output is phase-level summaries only |
| Human-in-the-loop approval at every step | CrewAI and AutoGen both support HITL; users from those tools expect it | Destroys the "autonomous" value prop; if every step requires approval, users are just doing the work themselves | Reserve HITL for the planning/dry-run phase only; execution is autonomous by design; users review the plan, not every agent action |
| Plugin/extension system for custom agents | Power users always ask for extensibility early | Premature abstraction before the core protocol is stable; plugin APIs become permanent commitments; maintenance burden compounds fast | Hardcode three agents in v1; design the agent trait/interface cleanly in Rust so extension is possible later without a public API commitment now |
| Automatic PR creation / GitHub integration | Feels like the natural completion of a code-generation workflow | Adds OAuth, GitHub API dependency, branch management logic; out of scope per PROJECT.md; direct commits to local repo is the stated design | Commit directly to local repo per phase; user pushes and opens PR themselves with full control |
| Self-hosted LLM support (Ollama, local models) | Privacy-conscious users want no data leaving their machine | Local models are significantly weaker for complex decomposition and code generation; would require model capability benchmarking to route tasks correctly; complicates the agent capability map | Document which model tiers are supported; note local model support as a v2+ consideration once routing is stable |
| Multi-user / team collaboration | Team workflows are a natural next step after single-user validation | Real-time collaboration requires a server, shared state store, conflict resolution between concurrent human users, and auth — an entirely different product | Stay single-user for v1; validate the core orchestration; team features are a post-PMF product decision |

---

## Feature Dependencies

```
[Natural Language Input]
    └──requires──> [Structured Phase Decomposition]
                       └──requires──> [Dependency Analysis]
                                          └──requires──> [Agent Routing / Skill Map]
                                                             └──requires──> [Module Isolation]
                                                                                └──requires──> [Parallel Execution]

[Structured Phase Decomposition]
    └──requires──> [Typed Inter-Agent Communication]
                       └──requires──> [Cross-Agent Review Gate]
                                          └──requires──> [Automatic Retry Loop]

[API Key Configuration]
    └──requires──> [All Agent API Calls]

[Per-Phase Git Commits]
    └──requires──> [Module Isolation] (know what each agent wrote before committing)

[Dry-Run / Planning Mode]
    └──enhances──> [Structured Phase Decomposition] (same decomposition, no execution)

[Token Usage Reporting]
    └──enhances──> [All Agent API Calls] (track per-call)

[Resume Interrupted Execution]
    └──requires──> [Structured Phase Decomposition] (know which phases completed)
    └──requires──> [Per-Phase Git Commits] (durable checkpoint marker)

[Visible Progress Reporting]
    └──enhances──> [All execution phases]

[Verbose Mode / Full Transcript]
    └──enhances──> [Cross-Agent Review Gate] (show reviewer's exact feedback)
    └──conflicts──> [Default Clean Output] (must be opt-in flag, not default)
```

### Dependency Notes

- **Dependency analysis requires structured decomposition first:** You cannot route tasks to agents or isolate modules until the phase structure exists. This is the foundational blocker — everything downstream depends on getting decomposition right.
- **Module isolation is a prerequisite for parallel execution:** Running agents in parallel on the same working directory causes real-time merge conflicts. Isolation (via file ownership list or git worktrees) must be established before parallelization is enabled.
- **Cross-agent review requires typed communication:** If inter-agent messages are unstructured prose, the reviewer cannot reliably parse what it is reviewing. Typed schemas must be enforced before review gates are meaningful.
- **Resume requires phase state persistence:** If Athena crashes mid-execution, it needs a checkpoint file or database to know which phases completed and what was committed. This is a non-trivial addition and should be deferred past v1.
- **Dry-run enhances but does not require anything:** It is a read-only projection of the decomposition output. It can be added at any phase of development with low risk.
- **Token reporting conflicts with minimal-output defaults:** Streaming verbose token data conflicts with the default clean progress output. Must be a flag, not default behavior.

---

## MVP Definition

### Launch With (v1)

Minimum viable to validate the core concept: intelligent orchestration produces better code than a single agent.

- [ ] Natural language and spec file input — the only way users describe their project
- [ ] Structured phase decomposition with dependency ordering — Athena's core value; without this, everything else is just sequential prompting
- [ ] Agent routing (Claude → architecture/logic, Gemini → research/APIs, Codex → boilerplate generation) — the heterogeneous model approach is the differentiator
- [ ] Module isolation via file ownership list — prevents parallel write conflicts; full git worktree isolation can come later
- [ ] Sequential phase execution with cross-agent review gate — the quality gate is load-bearing; ship it from day one even if parallelism comes later
- [ ] Automatic retry loop on review failure (max 3 attempts) — makes the system autonomous; without this, every review failure requires human intervention
- [ ] Per-phase git commits — basic traceability; non-negotiable
- [ ] Terminal progress reporting (phase name, current agent, status) — users will abandon a tool that appears hung
- [ ] Structured final report (phase table, agent assignments, review outcomes) — the "receipt" of what was built
- [ ] API key config via env vars and config file — standard BYO-key UX
- [ ] Typed inter-agent message schemas — foundational reliability; skip this and cross-agent review becomes unreliable
- [ ] Error reporting with phase-level context — distinguishes API errors from review failures from schema violations

### Add After Validation (v1.x)

Add once v1 proves the core loop works and users are retained.

- [ ] Dry-run / planning mode — add once decomposition is stable and trustworthy; users will want to inspect plans before paying API costs
- [ ] Token usage and cost reporting per phase — add once users ask "why did that cost so much?"; high-value, low-complexity addition
- [ ] True parallel phase execution for independent modules — add once isolation is proven reliable; requires dependency graph correctness first
- [ ] Git worktree isolation (vs. file-list isolation) — upgrade isolation mechanism once parallel execution is added; cleaner boundaries
- [ ] Verbose mode / full agent transcript flag — add when users request debugging capability; `--verbose` flag on existing output

### Future Consideration (v2+)

Defer until post-PMF — these require either significant new architecture or a clear user signal.

- [ ] Resume interrupted execution — requires persistent checkpoint state; meaningful complexity addition; defer until users report losing work
- [ ] Spec-driven input format (requirements.md / design.md structured files) — add once the free-form NL input is well-understood; standardize after usage patterns emerge
- [ ] Plugin system for custom agents — defer until the three-agent model is proven; premature extensibility is a maintenance trap
- [ ] Local/self-hosted model support — defer until local models reach capability parity for complex decomposition tasks

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Structured phase decomposition | HIGH | HIGH | P1 |
| Cross-agent review gate | HIGH | HIGH | P1 |
| Module isolation (file ownership) | HIGH | MEDIUM | P1 |
| Agent routing (Claude/Gemini/Codex) | HIGH | MEDIUM | P1 |
| Natural language input parsing | HIGH | MEDIUM | P1 |
| Per-phase git commits | HIGH | LOW | P1 |
| API key config (env + file) | HIGH | LOW | P1 |
| Terminal progress reporting | HIGH | LOW | P1 |
| Typed inter-agent schemas | HIGH | MEDIUM | P1 |
| Automatic retry on review failure | HIGH | MEDIUM | P1 |
| Structured final report | MEDIUM | LOW | P1 |
| Error reporting with context | MEDIUM | LOW | P1 |
| Dry-run / planning mode | HIGH | LOW | P2 |
| Token usage and cost reporting | MEDIUM | MEDIUM | P2 |
| Parallel phase execution | MEDIUM | HIGH | P2 |
| Git worktree isolation | MEDIUM | MEDIUM | P2 |
| Verbose mode / full transcript | LOW | LOW | P2 |
| Resume interrupted execution | MEDIUM | HIGH | P3 |
| Spec-driven input format | MEDIUM | LOW | P3 |
| Plugin system for agents | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## Competitor Feature Analysis

| Feature | CrewAI | MetaGPT | LangGraph | AutoGen | Athena (planned) |
|---------|--------|---------|-----------|---------|-----------------|
| Role-based agent assignment | Yes — core pattern | Yes — simulates software company roles (PM, Architect, Engineer, QA) | Implicit via node design | Conversational personas, dynamic | Yes — fixed roles mapped to Claude/Gemini/Codex strengths |
| Task decomposition / planning | Task-oriented, manual or hierarchical manager | SOPs encode a fixed decomposition pattern | Graph nodes, manually specified by developer | Emerges through multi-turn conversation | Yes — LLM-driven decomposition from NL input |
| Dependency analysis | Manual (task dependencies declared by user) | Not automatic — fixed SOP pattern | Manual (graph edges declared by developer) | None — sequential conversation | Yes — automatic, inferred from project spec |
| Parallel execution | Yes — crew coordination | Yes — parallel subtask decomposition | Yes — DAG-based | Limited — sequential turns | v1: sequential; v1.x: parallel independent phases |
| Cross-agent review gate | Yes — hierarchical manager validates | Yes — peer review in SOP | Configurable — decision nodes | Yes — agents can critique each other in conversation | Yes — mandatory blocking gate, cross-vendor |
| Module isolation | No — agents share codebase | No explicit isolation | No — shared state | No | Yes — strict file ownership list per agent |
| Heterogeneous model routing | No — single LLM provider typically | No | No | Partial — different backends per agent | Yes — skill-based routing across Claude, Gemini, Codex |
| Git integration | No built-in | No built-in | No built-in | No built-in | Yes — per-phase commits, local repo |
| Typed inter-agent schemas | Partial — structured task outputs | Partial — structured documents | Partial — state schema | No — conversation prose | Yes — validated JSON schemas at every boundary |
| Dry-run / planning mode | No | No | No | No | v1.x |
| Cost / token reporting | No (enterprise dashboard only) | No | No (LangSmith external) | No | v1.x |
| Resume interrupted execution | No | No | Yes — checkpointing | No | v2+ |
| Single binary distribution | No — Python package | No — Python package | No — Python package | No — Python package | Yes — Rust compiled binary |
| CLI-first / no server required | No — enterprise requires platform | No | No — LangGraph Studio is web | No | Yes |

---

## Sources

- [CrewAI vs LangGraph vs AutoGen: Choosing the Right Multi-Agent AI Framework — DataCamp](https://www.datacamp.com/tutorial/crewai-vs-langgraph-vs-autogen)
- [LangGraph vs CrewAI vs AutoGen: Top 10 AI Agent Frameworks — o-mega.ai](https://o-mega.ai/articles/langgraph-vs-crewai-vs-autogen-top-10-agent-frameworks-2026)
- [CrewAI vs LangGraph vs AutoGen vs OpenAgents (2026) — OpenAgents Blog](https://openagents.org/blog/posts/2026-02-23-open-source-ai-agent-frameworks-compared)
- [AutoGen vs LangGraph vs CrewAI: Which Agent Framework Actually Holds Up in 2026? — DEV Community](https://dev.to/synsun/autogen-vs-langgraph-vs-crewai-which-agent-framework-actually-holds-up-in-2026-3fl8)
- [Multi-agent workflows often fail. Here's how to engineer ones that don't. — GitHub Blog](https://github.blog/ai-and-ml/generative-ai/multi-agent-workflows-often-fail-heres-how-to-engineer-ones-that-dont/)
- [MetaGPT: Meta Programming for a Multi-Agent Collaborative Framework — arXiv](https://arxiv.org/abs/2308.00352)
- [What is MetaGPT? — IBM Think](https://www.ibm.com/think/topics/metagpt)
- [AI Agent Orchestration Patterns — Azure Architecture Center, Microsoft Learn](https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/ai-agent-design-patterns)
- [Anti-Patterns in Multi-Agent Gen AI Solutions — Medium / Arman Kamran](https://medium.com/@armankamran/anti-patterns-in-multi-agent-gen-ai-solutions-enterprise-pitfalls-and-best-practices-ea39118f3b70)
- [Git Worktrees: The Secret Weapon for Running Multiple AI Coding Agents in Parallel — Medium](https://medium.com/@mabd.dev/git-worktrees-the-secret-weapon-for-running-multiple-ai-coding-agents-in-parallel-e9046451eb96)
- [ccswarm: Multi-agent orchestration using Claude Code with Git worktree isolation — GitHub](https://github.com/nwiizo/ccswarm)
- [10 Things Developers Want from their Agentic IDEs in 2025 — RedMonk](https://redmonk.com/kholterhoff/2025/12/22/10-things-developers-want-from-their-agentic-ides-in-2025/)
- [Developer's guide to multi-agent patterns in ADK — Google Developers Blog](https://developers.googleblog.com/developers-guide-to-multi-agent-patterns-in-adk/)
- [The Rise of Agentic Testing: Multi-Agent Systems for Robust Software Quality Assurance — arXiv](https://arxiv.org/abs/2601.02454)
- [5 Key Trends Shaping Agentic Development in 2026 — The New Stack](https://thenewstack.io/5-key-trends-shaping-agentic-development-in-2026/)
- [Best AI Coding Agents for 2026: Real-World Developer Reviews — Faros AI](https://www.faros.ai/blog/best-ai-coding-agents-2026)

---
*Feature research for: Multi-agent AI orchestration CLI (Athena)*
*Researched: 2026-03-12*
