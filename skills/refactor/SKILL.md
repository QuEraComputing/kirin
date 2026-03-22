---
name: refactor
description: Use when refactoring Rust code across crates, extracting traits, splitting modules, or renaming across 3+ files. Also auto-suggested when detecting cross-crate moves or trait extractions.
effort: high
argument-hint: "[what to refactor]"
---

# Refactor

## Overview

**Announce at start:** State which skill is being used so the user knows what process is driving behavior.

Rust-specific refactoring orchestration with architectural guardrails and a configurable agent team. Two entry paths depending on refactor size, then implementation planning, guarded execution, final validation, template capture.


## When to Use

- Explicit: user invokes `/refactor`
- Auto-suggest when detecting: cross-crate moves, trait extractions, module splits, renames across 3+ files

**Don't use for:**
- Single-file changes
- Adding new code that doesn't touch existing abstractions
- Bug fixes that don't change public APIs

## Target

The refactoring target is: **$ARGUMENTS**

If no target was provided, ask the user what to refactor.

## Path Selection

Before starting, classify the refactor and choose the appropriate path:

| Signal | Path | Why |
|--------|------|-----|
| User specifies concrete changes ("extract trait X", "move Y to crate Z") | **Focused** — pre-flight first | You already know what moves; pre-flight scopes it, review validates it |
| User gives a broad target ("refactor kirin-interpreter", "clean up the derive subsystem") | **Discovery** — triage-review first | You don't know what's wrong yet; review discovers the issues, pre-flight scopes the changes from findings |
| Ambiguous | Ask the user: "Do you have specific changes in mind, or should I review the target first to identify what needs refactoring?" | |

**Announce the chosen path** so the user knows which flow is driving behavior.

---

## Focused Path (pre-flight → review)

Use when the user already knows what needs to change.

### Phase 1F: Scope & Pre-flight

**Exploration budget (hard cap):**
- Small refactors (1-3 crates): 10 file reads + 5 grep searches
- Large refactors (4+ crates): 20 file reads + 15 grep searches

After budget is spent, you MUST have a concrete understanding and proceed to the review.

**Pre-flight checklist:**

1. For each moved/created type: identify target crate, feature flag, visibility level
2. Read dependent crates' Cargo.toml to map the dependency graph
3. Check for visibility-bridging wrappers in affected code (one-liner methods that bridge pub(crate) to pub)
4. List all `pub` items that will change
5. Classify the refactor pattern:
   - **In-place**: modify existing crates (rename, move, split)
   - **Additive-then-switchover**: build new crates first, then swap consumers and delete old crates (zero-risk build phase, atomic switchover)

Output format:
```
## Pre-flight Summary

**Refactor scope:** [small/large] ([N] crates affected)
**Pattern:** [in-place / additive-then-switchover]

**Changes:**
| Item | From | To | Crate | Visibility | Feature Flag |
|------|------|----|-------|------------|-------------|
| TraitName | crate-a | crate-b | kirin-X | pub | interpret |

**Dependent crates:** [list]
**Visibility bridges to preserve:** [list or "none found"]
**Public API changes:** [list]
```

Present to user for approval before proceeding.

### Phase 2F: Review (validation)

Load the `triage-review` skill scoped to the affected crates. Pass the pre-flight summary as context so reviewers evaluate the proposed refactor, not just the current state.

When loading triage-review, suggest including:
- **Soundness Adversary** if the refactor touches builder APIs, arena/ID code, or interpreter internals
- **Dialect Author** if the refactor affects dialect crates or dialect author-facing APIs

The triage-review walkthrough serves as the user decision point — confirms or rejects findings before implementation.

→ After walkthrough completes, proceed to **Phase 3: Implementation Planning**.

---

## Discovery Path (triage-review → pre-flight)

Use when the user provides a broad target without specific changes. The review discovers what needs refactoring; the pre-flight then scopes those discoveries into concrete changes.

### Phase 1D: Triage Review (discovery)

Load the `triage-review` skill scoped to the target crate(s)/subsystem. Do NOT pass a pre-flight summary — there is none yet. The reviewers evaluate the current state to identify architectural issues, code smells, and refactoring opportunities.

When loading triage-review for discovery, suggest including:
- **Soundness Adversary** if the target includes builder APIs, arena/ID code, or interpreter internals
- **Dialect Author** if the target includes dialect crates or dialect author-facing APIs

The triage-review walkthrough presents findings to the user. The user selects which findings to act on — this becomes the refactoring scope.

### Phase 2D: Scope & Pre-flight (from findings)

Using the accepted triage-review findings as input, run the pre-flight checklist (same as Phase 1F above). The difference: instead of scoping from user-specified changes, you scope from review-discovered issues.

**Exploration budget**: same caps as Phase 1F.

Output the same pre-flight summary format. Present to user for approval before proceeding.

→ After approval, proceed to **Phase 3: Implementation Planning**.

---

### User Decision Point

After both paths converge (review + pre-flight both complete), present ALL implementation options. Include a recommendation based on the refactor's characteristics, but let the user choose.

```
How would you like to proceed with the implementation?

1. Direct — I implement the changes myself in this session (best for small, straightforward refactors with < 3 files)
2. Sequential agent — a single background agent executes step-by-step with review checkpoints (best for medium refactors or highly sequential changes where each step depends on the previous)
3. Agent team — multiple implementers working in parallel, each in isolated worktrees (best for large refactors with independent work streams across crates)
```

**Recommendation heuristics** (suggest, don't force):
- 1-3 files, no cross-crate moves → recommend Direct
- Single crate or strictly sequential dependency chain → recommend Sequential agent
- 2+ independent work streams across crates → recommend Agent team

The user's choice determines how Phase 3 designs the plan.

## Phase 3: Implementation Planning

Design the implementation plan based on the user's choice. Use existing skills:
- Load the `writing-plans` skill to create a detailed step-by-step plan if one doesn't exist
- Load the `brainstorming` skill for upstream design work if the approach is unclear

### For Direct Implementation

No agent dispatch needed. The lead agent (you) implements changes directly in the current session.
- Follow the invariants from Phase 4
- Run `cargo check -p <crate>` after every file modification
- Present changes to user for review at natural checkpoints

### For Sequential Agent

Load the `subagent-driven-development` or `executing-plans` skill for step-by-step execution with review checkpoints.

Dispatch a single agent through the pre-dispatch gate (see Phase 4) — `run_in_background: true` and `isolation: "worktree"` are both required. The user can continue interacting while the agent works sequentially through the plan.

### For Agent Team

Identify independent work streams that can run concurrently. Group by:
- **Dependency order**: foundation crates first, then dependents
- **File disjointness**: agents MUST NOT edit the same files concurrently

Map work streams to agent roles:

| Role | Purpose | When to Use |
|------|---------|-------------|
| Builder | Create new crates from scratch | Additive-then-switchover pattern |
| Implementer | Modify existing crates | In-place refactors |
| Migrator | Update downstream consumers (imports, Cargo.toml, feature flags) | When downstream crates are affected |
| Verifier | Run checks, tests, and review agent output; report findings to lead | Always (dedicated agent) |
| Integrator | Merge worktree branches, resolve conflicts, polish the merged result | Always when using agent teams (dedicated agent) |

**Critical dependency sequencing**: If work stream B depends on A's output, A must complete before B starts. Use task dependencies to enforce this.

#### Coordination: Agent Teams (preferred) or Background Agents

**Option A: Agent Teams (preferred when TeamCreate is available)**

Use `TeamCreate` to create a refactor team, then spawn implementer teammates. This gives structured coordination via shared task lists and message passing.

1. Create team: `TeamCreate(team_name: "refactor-<scope>", description: "Refactoring <scope>")`
2. Create tasks for each work stream using `TaskCreate` — one task per agent assignment, with dependencies reflecting the ordering
3. Spawn each code-editing teammate through the pre-dispatch gate (see Phase 4). Every call MUST include both `isolation: "worktree"` and `run_in_background: true`:
   ```
   Agent(team_name: "refactor-<scope>", name: "<role>-<crate>",
         isolation: "worktree", run_in_background: true, ...)
   ```
4. Spawn the Integrator (works on the dev branch, not a worktree — it is the sole writer to the dev branch)
5. Spawn the Verifier (read-only — no `isolation: "worktree"` needed)
6. Teammates pick up tasks from the shared list, work in isolated worktrees, mark tasks complete, and go idle
7. As implementers complete, the lead creates merge tasks for the Integrator specifying branch and merge order
8. After each merge, the Integrator notifies the Verifier; the Verifier checks and reports to lead
9. After all work is done, send shutdown messages to all teammates and call `TeamDelete`

**Option B: Background Agents (fallback)**

If `TeamCreate` is not available, dispatch agents through the pre-dispatch gate with `run_in_background: true` and `isolation: "worktree"`. Track progress through agent completion notifications.

**Non-blocking requirement (both options):** All implementer agents MUST run in background (`run_in_background: true`). The user must be able to interact with the main agent at all times during execution. Never block on agent completion.

### Plan Approval

Present the implementation plan to the user. Include:
- Work streams and their ordering
- Which agents handle which tasks
- Expected merge sequence
- Verification checkpoints

**User approves before any code changes.**

## Phase 4: Guarded Execution

### Worktree Isolation

Every agent that edits code works in its own git worktree. The pre-dispatch gate (below) enforces this — `isolation: "worktree"` on the Agent call is the mechanism that creates the worktree automatically. This applies to all execution modes except Direct implementation.

Each agent works in its isolated worktree. The lead agent (or a merge orchestrator) merges branches back in dependency order.

### Pre-Dispatch Gate (orchestrator checklist)

Before dispatching ANY code-editing agent (whether via Agent Teams or background agents), verify:

1. `isolation: "worktree"` is set on the Agent call — this is the enforcement mechanism, not a suggestion
   - **Exception:** The Integrator works on the dev branch directly (no worktree) since it is the sole writer to it
2. `run_in_background: true` is set
3. The agent's prompt includes the invariants block below
4. The agent's file assignments don't overlap with any other active agent

If any check fails, do NOT dispatch. Fix the call first.

### Invariants (inject into ALL agent prompts)

```
REFACTOR INVARIANTS — these override any conflicting instructions:
0. WORKTREE CHECK: You MUST be running in a git worktree, not the main working directory.
   Run `git rev-parse --show-toplevel` — if the result is the project root (not a worktree path),
   STOP immediately and report to the lead. Do not edit files in the main working directory.
1. NEVER use `#[allow(...)]` or ignore comments as fixes for real errors.
2. NEVER remove one-liner wrapper methods without verifying they are not visibility bridges
   (methods that expose pub(crate) internals through a pub interface).
3. NEVER place new types/traits without checking CLAUDE.md crate ownership conventions.
4. If `cargo check` fails 3 times on the same error, STOP and report the issue.
5. For proc-macro refactors: expand generated code before and after changes to catch
   regressions (use `cargo expand` or the project's debug dump mechanism like KIRIN_EXPAND_DEBUG=1).
6. When snapshot tests exist: run `cargo insta test` to detect changes and report them
   (do NOT auto-accept without lead approval).
```

### Cargo Check Strategy

The strategy depends on execution mode:

- **Direct**: Run `cargo check -p <crate>` after every file modification. Do not batch.
- **Sequential agent**: Run `cargo check -p <crate>` after every file modification within the worktree.
- **Agent team**: Each agent runs `cargo check` within its own worktree. Cross-crate checks happen AFTER merging, led by the Verifier agent.

### The Verifier Agent

**Always staff a Verifier** when using parallel agents. The Verifier:

1. **After each implementer completes a task**: reviews the changes in that agent's worktree, runs `cargo check` and relevant tests, reports findings to the lead
2. **After merging**: runs full workspace verification (`cargo build --workspace`, `cargo nextest run --workspace`, `cargo test --doc --workspace`)
3. **Reports issues to lead**: the lead decides whether to fix now or defer, and assigns fixes to an idle implementer

The Verifier does NOT fix code — it only checks and reports. This separation prevents the verifier from introducing its own bugs.

### The Integrator Agent

**Always staff an Integrator** when using agent teams. The Integrator is a dedicated teammate responsible for merging worktree branches into the dev branch and producing a clean, cohesive result. Spawn with `run_in_background: true` but WITHOUT `isolation: "worktree"` — the Integrator is the sole writer to the dev branch and needs direct access to it.

The Integrator:

1. **Merges worktree branches** in dependency order as implementers complete their tasks. Does not wait for all implementers to finish — merges incrementally as work streams complete.
2. **Resolves merge conflicts** — understands the refactor plan and makes informed decisions about which side to keep. Reports ambiguous conflicts to the lead.
3. **Polishes the merged result:**
   - Fixes import ordering and dead imports left by the merge
   - Ensures consistent naming conventions across merged code from different agents
   - Removes leftover TODO/FIXME markers that agents resolved in their own branches but didn't clean up in others'
   - Runs `cargo fmt --all` after each merge
4. **Cleans up worktree artifacts** — removes merged worktree branches, prunes stale references
5. **Hands off to Verifier** — after each merge, notifies the Verifier to run checks on the merged state

**The Integrator works on the dev branch directly** (not in a worktree) since it is the only agent writing to it. No other agent edits the dev branch.

**Relationship to other roles:**
- Implementers/Builders/Migrators → produce branches in worktrees
- Integrator → merges those branches, resolves conflicts, polishes
- Verifier → checks the merged result, reports issues back to lead
- Lead → assigns merge order, triages Verifier findings

### Lead Agent Responsibilities

The lead agent (you) orchestrates:

1. **Task assignment**: create tasks, assign to agents, track dependencies
2. **Merge ordering**: tell the Integrator which branches to merge and in what order
3. **Issue triage**: when the Verifier reports problems, decide:
   - Which agent should fix it (prefer an idle agent with context, or the Integrator for polish issues)
   - Whether to fix now or batch fixes after all agents complete
4. **Unblocking**: if the Integrator hits an ambiguous conflict, make the call

### Guardian Role

Read the Guardian persona (from the team directory, see AGENTS.md). The Guardian runs as the lead agent's advisor:
- Pre-flight analysis (Phase 1)
- Migration checklist production for Migrators
- Post-validation pub-item diffing (Phase 5)

For small refactors, the lead agent can absorb Guardian duties directly.

## Phase 5: Final Validation + Documentation

1. **Integrator does final polish** on the fully merged dev branch:
   - `cargo fmt --all`
   - Remove any remaining worktree branches and stale references
   - Final pass for consistency: naming, imports, dead code from the merge process
2. **Verifier runs final checks:**
   - `cargo build --workspace`
   - `cargo nextest run --workspace`
   - `cargo test --doc --workspace`
   - `cargo insta test --workspace` (if snapshot tests exist)
3. **Diff pub items** in changed files against pre-flight list — flag unintended changes
4. Read the **Documenter** persona (from the team directory) to update CLAUDE.md/AGENTS.md/memory if conventions changed
5. Load the `finishing-a-development-branch` skill to complete

## Phase 6: Template Capture

**Automatically prompt** after successful refactor: "Save this team configuration as a template?"

If yes, save to the team templates directory (see AGENTS.md Project structure) as `<name>.md`:
```markdown
# Template: [name]

**Refactor type:** [description]
**Pattern:** [in-place / additive-then-switchover]
**Scope:** [N] crates
**Staffed roles:** [list with rationale]
**What worked:** [notes]
**What to adjust:** [notes]
**Date:** [YYYY-MM-DD]
```

## Red Flags — STOP Immediately

- Planning for more than 5 minutes without having started Phase 1 pre-flight
- Rewriting the pre-flight summary more than once
- Making code changes before user approves pre-flight summary AND implementation plan
- Two agents editing the same file (even in different worktrees — merge will conflict)
- Dispatching a code-editing agent without going through the pre-dispatch gate
- A code-editing agent NOT running in a worktree (any execution mode except Direct)
- `cargo check` failing 3+ times on the same error (escalate to user)
- Any agent placing types in a crate not listed in the pre-flight summary
- Verifier auto-fixing code instead of reporting to lead
- Dispatching implementer agents in foreground (blocking) instead of background — user must remain able to interact

## Rationalization Table

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Skip pre-flight | "This refactor is simple, I know what needs to move" | 'Simple' refactors have hidden consumers. Pre-flight takes 5 minutes; debugging a missed re-export takes 30. |
| Skip triage-review | "I already know the code well enough" | You know the code. The Formalism reviewer catches abstraction issues. The Soundness Adversary catches invariant violations. Fresh eyes find what familiarity hides. |
| Pre-flight before review on a broad target | "I'll figure out the scope first, then review" | Without review, pre-flight is guessing. For broad targets, you don't know what's wrong yet — the review discovers the refactoring scope. Pre-flight-first only works when the user already specifies concrete changes. |
| Start coding before plan approval | "I'll adjust the plan based on what I find" | Code-first planning produces sunk-cost pressure to keep bad decisions. Plan approval costs 2 minutes; reworking a wrong approach costs hours. |
| Edit the same file from two agents | "The changes are in different functions" | Git merges on function granularity, not line granularity. Two agents touching the same file creates merge conflicts that require manual resolution. |
| Let the verifier fix issues | "It's faster than dispatching back to the implementer" | The verifier lacks the implementer's context. Verifier fixes introduce new bugs at a higher rate. Report to lead, let the right agent fix it. |
| Skip exploration budget | "I need to read one more file to understand" | The budget exists because unbounded exploration delays the actual work. If 20 reads aren't enough, the scope is wrong — simplify it. |
| Dispatch agents in foreground | "I need to wait for them anyway before merging" | Foreground dispatch blocks the user. Refactors take minutes per agent — the user should be free to ask questions or provide context while agents work in background. |

## Integration

**Required workflow skills (load when needed):**
- The `using-git-worktrees` skill — worktree setup (only if `.worktrees/` doesn't exist yet)
- The `finishing-a-development-branch` skill — completion after Phase 5

**Execution skills (choose based on user's Phase 2 decision):**
- `TeamCreate` / `TeamDelete` — preferred for parallel agent teams (structured coordination with shared task lists)
- `Agent` with `run_in_background: true` + `isolation: "worktree"` — fallback for parallel execution when teams are unavailable
- The `subagent-driven-development` skill — solo agent, task-by-task with review
- The `executing-plans` skill — parallel or sequential execution with batch checkpoints

**Optional:**
- The `brainstorming` skill — upstream design work before refactoring
- The `writing-plans` skill — creates detailed plan if one doesn't exist
- The `simplify` skill — post-refactor code cleanup
