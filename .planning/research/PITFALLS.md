# Pitfalls Research

**Domain:** Multi-agent AI orchestration CLI (coordinating Claude, Gemini, Codex for software development)
**Researched:** 2026-03-12
**Confidence:** HIGH (multiple authoritative sources, official API docs verified, peer-reviewed research referenced)

---

## Critical Pitfalls

### Pitfall 1: Untyped Cross-Agent Handoffs (The "Messy JSON" Problem)

**What goes wrong:**
Agents exchange natural language or loosely-typed JSON where field names shift, data types mismatch, and required fields go missing. Athena passes output from Claude's architecture phase to Gemini's documentation phase, but Gemini receives ambiguous text it re-interprets differently than intended. Downstream agents then act on corrupted semantics. This is the #1 source of cascading failures — 42% of multi-agent failures stem from specification issues, most of which are interface mismatch problems.

**Why it happens:**
Developers treat agents like chat sessions rather than distributed system components. The orchestrator "passes the baton" by shoving raw agent output into the next prompt with no validation. Each agent summarizes and re-interprets the previous output, losing precision with every hop.

**How to avoid:**
Define strict typed schemas (Rust structs with serde) at every agent boundary. Agent output is never passed raw — it is deserialized into a validated struct before being forwarded. Invalid output triggers a retry or escalation, not propagation. Treat schema violations as contract failures: retry, repair, or halt — never forward bad state.

**Warning signs:**
- Agent N+1 asks clarifying questions about output from Agent N
- Phase outputs have different module/file naming than the decomposition plan specified
- Review agent flags structure-level issues rather than logic issues (it's reasoning from wrong premises)
- Token costs spike unexpectedly (agents consuming large context to re-orient)

**Phase to address:** Phase implementing agent communication layer (core orchestration); schema contracts must be established before any agent integration work begins.

---

### Pitfall 2: Runaway Token Costs with No Budget Enforcement

**What goes wrong:**
A single misconfigured review loop, an agent that gets stuck re-generating output, or a phase decomposition that spawns more cross-review calls than expected can multiply token costs 3-10x. Because each LLM call in an agentic workflow accumulates context across steps, a 3-agent pipeline with cross-review generates 6+ API calls per phase, each with growing context windows. A real-world case study showed a 300% cost spike — from $1,200 to $4,800/month — from one workflow change. For Athena, if the review loop fails to converge and re-runs 5 times instead of 2, costs multiply silently.

**Why it happens:**
Token pricing seems trivial per call (fractions of a cent) until multiplied across phases, agents, retries, and cross-review passes. Most orchestration systems have no hard budget enforcement — they rely on developers noticing high bills after the fact.

**How to avoid:**
Implement per-run token budget tracking from day one. Before each phase, estimate total tokens needed (input context + expected output per agent + review calls). Enforce a hard cap (configurable via config file). Track cumulative token usage in a run manifest. If a phase is about to exceed its budget, halt and report — never silently continue. Provide dry-run mode that estimates costs without making API calls.

**Warning signs:**
- Review loops run more than 2-3 iterations without convergence
- Context windows growing larger each phase (previous phase outputs accumulating)
- Same module being regenerated multiple times
- No token counter visible in run output

**Phase to address:** Phase implementing API client layer — budget tracking must be built into the API abstraction, not bolted on later.

---

### Pitfall 3: Agent Drift — Quality Degradation Across Extended Runs

**What goes wrong:**
Research (arxiv:2601.04170) quantified this precisely: multi-agent systems exhibit "progressive degradation of agent behavior, decision quality, and inter-agent coherence over extended interaction sequences." After a median of 73 interactions, systems show: 42% reduction in task success rate, 3.2x increase in required interventions, 63% longer completion times. For Athena, this means early phases produce quality code, but later phases (operating with accumulated context pollution) produce code that is inconsistent with earlier architecture decisions, uses different naming conventions, or contradicts established interfaces.

**Why it happens:**
Three mechanisms compound each other: (1) context pollution — accumulated irrelevant information dilutes signal; (2) distributional shift — later phases encounter edge cases diverging from the agent's training distribution; (3) autoregressive reinforcement — agents' outputs become future inputs, compounding small errors through feedback loops. Anthropic's internal testing showed quality drop-off begins at approximately 70% context utilization.

**How to avoid:**
Never pass full phase history to subsequent phases — pass structured summaries only. Maintain an explicit "project manifest" (Rust struct, serialized to disk) that captures architectural decisions, file ownership, interface contracts, and module names. This manifest is the agent's ground truth, not the conversation history. Agents receive the manifest + their specific task, not the entire prior dialogue. Reset agent context between phases; preserve decisions via the manifest, not via context window accumulation.

**Warning signs:**
- Later phases introduce naming inconsistencies (different variable/function names than earlier phases)
- Review agents flag increasing numbers of issues in later phases
- Generated code imports modules with different names than those created earlier
- Token input sizes growing linearly with phase count

**Phase to address:** Phase implementing phase state management and the project manifest structure — must be designed before multi-phase execution is implemented.

---

### Pitfall 4: Infinite Review Loops Without Convergence Guards

**What goes wrong:**
The cross-review model (Agent A reviews Agent B's output) can deadlock when: the reviewing agent has different quality standards than the producing agent, the producing agent cannot satisfy the reviewer's criteria within the project constraints, or the reviewer is hallucinating issues that do not exist. Without a convergence limit, Athena will loop indefinitely, consuming tokens and time. In the worst case, a 3-agent review with no convergence guard burns through API budget and never produces output.

**Why it happens:**
Cross-review is designed to catch blind spots, but "different model perspectives" can mean genuinely incompatible quality judgments. Claude might architect something that Gemini flags as underdocumented; Gemini's suggestions might be rejected by Claude as over-engineered — creating a genuine oscillation. Without a maximum iteration count and an escalation path, the loop is unbounded.

**How to avoid:**
Enforce a maximum review iteration count (default: 3, configurable). After max iterations without pass: (1) log the review disagreement with full context, (2) produce the best available output with a flagged warning, (3) continue to next phase. Distinguish between "blocking" review failures (security issues, API contract violations) and "advisory" failures (style, documentation gaps). Only block on blocking failures. Track the delta between review iterations — if issues are not decreasing, declare convergence failure early.

**Warning signs:**
- Any review loop exceeding 2 iterations
- Review comments addressing the same issue repeatedly
- Review output becoming longer and more detailed (not shorter — a sign of escalating disagreement)
- Phase wall-clock time significantly exceeding estimate

**Phase to address:** Phase implementing cross-review gate logic — convergence guards are not optional; they must be in the initial implementation.

---

### Pitfall 5: Module Isolation Boundary Violations Under Shared Dependencies

**What goes wrong:**
Athena's module isolation strategy (each agent owns specific files) breaks down when agents need to reference shared interfaces, utility functions, or type definitions. Agent A (Claude) generating backend logic needs to define an interface. Agent B (Codex) generating the API layer needs to implement that interface. If both agents are running in parallel, Agent B may generate code against a different interface assumption than what Agent A produces. The result: compilation errors, runtime type mismatches, or silently incompatible implementations that pass review but fail integration.

**Why it happens:**
Parallel execution assumes true independence, but software modules are not truly independent — they share type definitions, error codes, configuration schemas, and utility functions. The "module isolation" architecture prevents file-level conflicts but does not prevent semantic conflicts at interface boundaries.

**How to avoid:**
Phase decomposition must identify shared contracts before parallel assignment. Any interface or type that crosses agent boundaries must be defined in a "contracts phase" before parallel work begins. Store interface contracts in the project manifest (serialized Rust types or language-specific interface files). Agents receive their assigned module + the contracts file; they may not modify the contracts file. Run a contracts validation step at phase merge time before commit. For parallel agents: generate contracts → validate contracts → assign parallel work → merge → integration test.

**Warning signs:**
- Any two agents assigned modules that share imports or function calls
- Phase decomposition assigns "API layer" and "service layer" to different agents without a contracts step
- Compilation errors at phase merge time
- Review agents approving code that calls functions with different signatures

**Phase to address:** Phase implementing phase decomposition and dependency analysis — the contracts-first pattern must be enforced at decomposition time, not discovered at integration.

---

### Pitfall 6: API Reliability Assumptions (No Retry/Circuit Breaker Architecture)

**What goes wrong:**
Anthropic's API has documented 500 (internal error) and 529 (overloaded) transient error codes. OpenAI and Google have equivalent patterns. An autonomous multi-agent run that hits a 529 during phase 4 of 8 has lost all prior work if there is no retry layer. Worse: naive retry logic (immediate retry, no backoff) can trigger 429 rate-limit errors on top of the original error, creating a cascading failure that kills the entire run.

**Why it happens:**
CLI tools are often built with happy-path assumptions. Developers test on lightly-loaded APIs and assume reliability. Production runs on real projects hit API overload at exactly the worst time — when large context payloads are being sent for complex phases.

**How to avoid:**
Implement a proper retry layer in the Rust HTTP client: exponential backoff (start at 1s, cap at 60s) for 429/500/529; no retry for 400/401/403/404 (these are client errors). Respect `Retry-After` response headers when present. Implement a circuit breaker per API provider: after N consecutive failures, fail fast with a clear error rather than continuing to retry. Persist phase state to disk before each API call so a failed run can be resumed rather than restarted. For long phases, use streaming APIs to avoid idle connection timeouts.

**Warning signs:**
- No explicit retry configuration in API client code
- Error handling that catches all errors and retries regardless of type
- No phase state persistence between API calls
- Run configuration has no timeout or max-retry settings

**Phase to address:** Phase implementing API client abstraction — retry logic, circuit breaking, and phase state persistence must be in the initial API client implementation.

---

### Pitfall 7: Prompt Engineering That Produces Inconsistent Agent Roles

**What goes wrong:**
Athena assigns agents based on strengths: Claude for architecture/logic, Gemini for research/docs/APIs, Codex for code generation/boilerplate. But without tightly constrained role prompts, agents override their assigned roles: Claude starts generating boilerplate instead of architecture, Gemini starts modifying code instead of documenting it. This produces overlapping and conflicting output. Worse: an agent assigned "documentation" that actually modifies source files violates the module isolation guarantee.

**Why it happens:**
LLMs are trained to be helpful. If an agent sees a bug while writing documentation, it will fix the bug — even if its role is documentation only. Without explicit constraints on what the agent is and is not allowed to do, agents "helpfully" exceed their scope, undermining the isolation architecture.

**How to avoid:**
Role prompts must be prescriptive about both what the agent does and what it explicitly must not do. For each agent assignment: define the deliverable (what to produce), the boundary (what files/modules are in scope), the prohibition (what files/actions are out of scope), and the output format (typed schema, not freeform). Test role prompts independently before integration. Use structured output enforcement (JSON schema) to prevent agents from returning freeform text that the orchestrator cannot parse. Treat prompt changes as code changes — version them.

**Warning signs:**
- Agent output includes files outside its assigned module list
- Agents adding "helpful" fixes or improvements to out-of-scope areas
- Output schema violations (agent returning structured text instead of JSON)
- Different runs of the same phase producing structurally different outputs

**Phase to address:** Phase implementing agent prompt templates — role constraints must be designed alongside the module isolation strategy, not independently.

---

### Pitfall 8: Phase Decomposition Quality — Garbage In, Garbage Out

**What goes wrong:**
Athena's core value is intelligent phase decomposition. If the decomposition itself is wrong — phases ordered incorrectly, dependencies mis-identified, granularity too coarse or too fine — every downstream phase inherits the error. A phase assigned to "implement auth" before "define user model" is structurally unsound. A phase granularity that assigns one agent "the entire backend" prevents parallelization and forces the agent to exceed context limits.

**Why it happens:**
Phase decomposition is itself an LLM call, subject to the same hallucination and reasoning errors as any other call. The orchestrator's own reasoning about the project may be wrong. Because decomposition happens first and errors propagate forward, decomposition errors have the highest impact of any failure mode.

**How to avoid:**
Validate decomposition output against a set of structural rules before execution begins: no phase may reference output from a later phase; no single agent task may exceed estimated context limits; all phases must have explicitly listed inputs and outputs; shared contracts must appear before any phase that references them. Run a dry-validation pass that checks these rules without making any other API calls. Surface decomposition output to the user for confirmation on the first run of a project (with option to auto-approve on subsequent runs). Implement decomposition scoring: flag phases with circular dependencies, unbounded scope, or missing input specifications.

**Warning signs:**
- Phase list contains circular references in dependency graph
- A single agent task encompasses multiple architectural layers
- Phase inputs reference artifacts that no prior phase produces
- Decomposition output has no explicit module boundary list

**Phase to address:** Phase implementing core decomposition engine — structural validation rules must be part of the initial decomposition, not a later addition.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Pass raw agent output between phases (no schema) | Faster initial implementation | Cascading failures as system scales; impossible to debug which agent introduced bad state | Never — schema contracts are foundational |
| Single global retry count (no per-error-type logic) | Simpler error handling | Retrying 401 auth errors forever; not retrying 529 overload errors that would recover | Never — error types have different retry semantics |
| No token budget tracking | Faster to ship | Silent cost spikes; impossible to cost-estimate runs; accidental $100+ charges | MVP only if run scope is small and known |
| Hardcoded agent role strings in prompt templates | Fast iteration | Prompt changes are code changes with no version history; behavior regressions are invisible | Never — prompts must be versioned artifacts |
| No phase state persistence | Simpler implementation | Any API failure restarts the entire run from scratch; unusable for long projects | Never — state persistence protects user investment |
| Infinite review loop (no convergence guard) | "Thorough" review feeling | Deadlocks, runaway token costs, stuck runs | Never — convergence guards are safety mechanisms |
| Module isolation via naming convention only (no enforcement) | Easier agent prompting | Agents write to out-of-scope files; silent isolation violations | Never — enforcement must be mechanical, not honor-based |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Anthropic API | Not handling 529 (overloaded) distinctly from 500 (internal error) | Both are transient and should retry, but 529 warrants longer backoff since it indicates system-wide load |
| Anthropic API | Using prefill (assistant message prefix) with Opus 4.6 | Opus 4.6 explicitly does not support prefilling — use structured outputs or `output_config.format` instead |
| Anthropic API | Making large non-streaming requests on flaky networks | Use streaming API for any request expected to take >30 seconds; set TCP keepalive |
| OpenAI/Codex API | Assuming same error code semantics as Anthropic | OpenAI 429 includes both rate limit and quota exhausted — these require different responses |
| All three APIs | Sharing a single retry budget across all providers | Provider-specific circuit breakers prevent one failing provider from exhausting the shared retry budget |
| Git integration | Agents committing directly to main branch | All agent work should go to isolated branches/worktrees, merged only after review gate passes |
| Git integration | Using `git add .` in agent-generated commits | Only stage files within the agent's assigned module boundaries; staging everything breaks isolation guarantees |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Growing context per phase (accumulating all prior phase output) | Token cost grows quadratically with phase count; later phases slower and worse quality | Pass structured manifest + task-specific context only; never full history | By phase 4-5 on a medium project (7+ phases total) |
| Sequential agent calls where parallel is safe | Run time = sum of all agent times; underutilizes parallel execution | Athena's dependency analysis must identify truly independent tasks and run them concurrently via Tokio tasks | Immediately visible on any project with 5+ phases |
| Blocking on synchronous HTTP in async Rust | Tokio executor thread starvation; apparent deadlock under load | Use `reqwest` with async features; never use blocking HTTP clients inside async contexts | At 2+ concurrent agent calls |
| No streaming for large code generation tasks | Network idle timeouts kill long-running requests silently | Enable streaming for any request with `max_tokens > 4000`; set TCP keepalive | On slow networks or requests that take >60s |
| Retrying immediately after 429 without reading `Retry-After` | Triggers acceleration limits per Anthropic docs; compounds the problem | Read `Retry-After` header; if not present, use exponential backoff starting at 5s | Any run with high concurrency or large context payloads |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Writing API keys to disk in plaintext (e.g., in run logs or state files) | Key exposure in log files, which may be shared in bug reports or stored in version control | Never log API keys; redact keys from all output; read only from env vars or config file with mode 600 |
| Agent-generated code containing injected instructions (prompt injection via codebase) | If Athena analyzes an existing codebase, adversarial comments/docs could hijack agent behavior | Treat all codebase input as untrusted user data; do not pass raw file content directly into system prompt; use structured extraction |
| Passing user API keys between agents (e.g., Agent B using Agent A's credentials) | Key misuse, unexpected charges on wrong account | Each provider has one client instance initialized once from env config; keys never appear in inter-agent messages |
| No output sandboxing — agents writing arbitrary files | An agent that "helpfully" writes outside its module directory could corrupt the project | Enforce file write restrictions at the orchestrator level; validate every file path before write against allowed module list |
| Trusting agent-generated git commit messages verbatim | Agents may include sensitive context (API responses, internal paths) in commit messages | Review and sanitize commit messages; strip content that looks like credentials, API responses, or file system paths |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent progress during long multi-phase runs | User cannot tell if the tool is working or stuck; abandons run | Stream phase progress to stdout in real time: current phase, agent assigned, tokens used, estimated remaining |
| No cost estimate before run starts | User is surprised by $10-50 API charges on large projects | Implement dry-run mode that estimates token count and cost per phase without calling APIs; show estimate and prompt for confirmation |
| Opaque failure messages ("Agent returned error") | User cannot diagnose which agent failed, why, or how to fix it | Include: which agent, which phase, error type, retry count, whether retryable, suggested action |
| No resume capability after partial failure | User must restart entire run after phase 6 of 8 fails | Persist phase state to disk; support `--resume` flag that picks up from last successful phase |
| Dumping raw LLM output to terminal | Walls of unstructured text; impossible to audit | Structured phase report: what was built, which files, what the reviewer said, any warnings |

---

## "Looks Done But Isn't" Checklist

- [ ] **Retry logic:** Verify that 400/401/403 errors are NOT retried (they are client errors that will never resolve), and 429/500/529 ARE retried with backoff
- [ ] **Cross-review:** Verify that review output is parsed and validated — not just trusting the reviewer said "PASS" in natural language
- [ ] **Module isolation:** Verify that the orchestrator checks file paths in agent output against the allowed module list before writing — not relying on agent compliance
- [ ] **Token tracking:** Verify that token counts are accumulated across the full run, not just per-call — per-call tracking misses the compounding effect
- [ ] **Convergence guards:** Verify that review loops have a hard maximum iteration count enforced in code, not just documented in prompts
- [ ] **Phase state:** Verify that a simulated mid-run crash can be recovered with `--resume` without re-running prior phases
- [ ] **Cost estimate:** Verify that dry-run mode produces an estimate within 20% of actual run cost (test on a small project)
- [ ] **Decomposition validation:** Verify that the decomposition validator catches circular phase dependencies before execution begins
- [ ] **API key safety:** Verify that API keys never appear in log output, state files, or generated commit messages

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Cascading failure from bad cross-agent handoff | HIGH | Identify first phase where schema violation occurred; re-run from that phase with fixed prompt; requires phase state persistence |
| Runaway token cost from infinite review loop | MEDIUM | Kill the process; inspect review log to identify oscillation cause; adjust review prompt or lower review strictness for affected phase; re-run from that phase |
| Agent drift in later phases producing inconsistent code | HIGH | Manual review of all phases after drift onset; update project manifest with explicit decisions that drift erased; re-run affected phases with manifest as ground truth |
| API key exposure in logs | HIGH | Rotate all exposed API keys immediately; audit log files for distribution; sanitize logs going forward |
| Module isolation violation (agent wrote outside boundary) | MEDIUM | Identify out-of-scope files; determine if they conflict with the owning agent's output; either discard or integrate with explicit merge review |
| Decomposition produced circular dependencies | LOW | Re-run decomposition with explicit instruction to resolve circular dependencies; validate before proceeding |
| Phase state lost (no persistence) | HIGH | Full re-run from start; no shortcut; build state persistence before this happens |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Untyped cross-agent handoffs | Agent communication layer (core orchestration) | Integration test: inject malformed output from Agent A; verify Agent B rejects and retries rather than propagating |
| Runaway token costs | API client abstraction | Cost test: run a known project in dry-run mode; verify estimate matches actual within 20%; verify budget cap halts the run |
| Agent drift across phases | Phase state management / project manifest design | Regression test: run a 6+ phase project; verify module names are consistent between phase 1 and phase 6 output |
| Infinite review loops | Cross-review gate implementation | Loop test: configure a review prompt guaranteed to fail; verify run halts at max iterations, not earlier and not later |
| Module isolation boundary violations | Phase decomposition + agent prompt templates | Boundary test: simulate agent writing to out-of-scope file; verify orchestrator rejects the write |
| API reliability / no retry architecture | API client abstraction (must be earliest implementation phase) | Fault injection: simulate 529 errors; verify exponential backoff; verify correct phases resume after recovery |
| Inconsistent agent role prompts | Agent prompt template design | Prompt test: run same prompt 5 times; verify structural output consistency; verify agent does not modify out-of-scope files |
| Bad phase decomposition | Core decomposition engine | Validation test: provide a project with circular dependencies; verify validator catches it before any API call is made |

---

## Sources

- [Multi-agent workflows often fail. Here's how to engineer ones that don't. — GitHub Blog](https://github.blog/ai-and-ml/generative-ai/multi-agent-workflows-often-fail-heres-how-to-engineer-ones-that-dont/)
- [Why Your Multi-Agent System is Failing: Escaping the 17x Error Trap — Towards Data Science](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)
- [Why Multi-Agent AI Systems Fail and How to Fix Them — Galileo](https://galileo.ai/blog/multi-agent-ai-failures-prevention)
- [Why Multi-Agent LLM Systems Fail — Galileo (second article)](https://galileo.ai/blog/multi-agent-llm-systems-fail)
- [Agent Drift: Quantifying Behavioral Degradation in Multi-Agent LLM Systems — arXiv 2601.04170](https://arxiv.org/html/2601.04170)
- [Multi-Agent AI Gone Wrong: How Coordination Failure Creates Hallucinations — Galileo](https://galileo.ai/blog/multi-agent-coordination-failure-mitigation)
- [7 Ways Multi-Agent AI Fails in Production — TechAhead](https://www.techaheadcorp.com/blog/ways-multi-agent-ai-fails-in-production/)
- [Why 40% of Multi-Agent AI Projects Fail — SoftwareSeni](https://www.softwareseni.com/why-forty-percent-of-multi-agent-ai-projects-fail-and-how-to-avoid-the-same-mistakes/)
- [How AI Agents Handle Stalled Tasks and Timeouts — DEV Community](https://dev.to/bobrenze/how-ai-agents-handle-stalled-tasks-and-timeouts-lessons-from-my-production-failure-1jj9)
- [Why Multi-Agent Orchestration Collapses: Deadlocks, Infinite Loops, Memory Overwrites — DEV Community](https://dev.to/onestardao/-ep-6-why-multi-agent-orchestration-collapses-deadlocks-infinite-loops-and-memory-overwrites-1e52)
- [Solving Parallel Workflow Conflicts Between AI Agents in Shared Codebases — Medium](https://medium.com/@raminmammadzada/solving-parallel-workflow-conflicts-between-ai-agents-and-developers-in-shared-codebases-286504422125)
- [Git Worktrees for Parallel AI Coding Agents — Upsun Developer Center](https://devcenter.upsun.com/posts/git-worktrees-for-parallel-ai-coding-agents/)
- [Anthropic API Errors — Official Documentation](https://platform.claude.com/docs/en/api/errors)
- [Control Costs in LLM Agents Effectively — ProsperaSoft](https://prosperasoft.com/blog/artificial-intelligence/ai-agent/llm-agent-api-costs/)
- [LLM Economics: How to Avoid Costly Pitfalls — AI Accelerator Institute](https://www.aiacceleratorinstitute.com/llm-economics-how-to-avoid-costly-pitfalls/)
- [Tackling Rate Limiting for LLM Apps — Portkey](https://portkey.ai/blog/tackling-rate-limiting-for-llm-apps/)
- [AI Agent Security Cheat Sheet — OWASP](https://cheatsheetseries.owasp.org/cheatsheets/AI_Agent_Security_Cheat_Sheet.html)
- [Prompt Injection Attacks: The Most Common AI Exploit in 2025 — Obsidian Security](https://www.obsidiansecurity.com/blog/prompt-injection)
- [Rust Libraries for LLM Orchestration 2026 — dasroot.net](https://dasroot.net/posts/2026/02/rust-libraries-llm-orchestration-2026/)

---

*Pitfalls research for: Athena — multi-agent AI orchestration CLI*
*Researched: 2026-03-12*
