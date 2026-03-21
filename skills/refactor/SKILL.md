---
name: refactor
description: Use when refactoring Rust code across crates, extracting traits, splitting modules, or renaming across 3+ files. Also auto-suggested when detecting cross-crate moves or trait extractions.
effort: high
argument-hint: "[what to refactor]"
---

# Refactor

## Overview

Rust-specific refactoring orchestration with architectural guardrails and a configurable agent team. Six phases: scope & pre-flight, review panel, implementation planning, guarded execution, final validation, template capture.


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

## Phase 1: Scope & Pre-flight

**Exploration budget (hard cap):**
- Small refactors (1-3 crates): 10 file reads + 5 grep searches
- Large refactors (4+ crates): 20 file reads + 15 grep searches

After budget is spent, you MUST have a concrete understanding and proceed to the review panel.

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

## Phase 2: Review

**Before any implementation**, review the proposed changes to catch architectural issues early.

Load the `triage-review` skill scoped to the affected crates. The triage-review skill handles reviewer selection, parallel dispatch, cross-review, and aggregation. Pass the pre-flight summary as context so reviewers evaluate the proposed refactor, not just the current state.

When loading triage-review for a refactor, suggest including:
- **Soundness Adversary** if the refactor touches builder APIs, arena/ID code, or interpreter internals
- **Dialect Author** if the refactor affects dialect crates or dialect author-facing APIs

The triage-review walkthrough (Phase 3 of that skill) serves as the user decision point — the user confirms or rejects findings before implementation begins.

### User Decision Point

After the triage-review walkthrough completes, ask the user:

```
How would you like to proceed with the implementation?

1. Parallel agent team — multiple implementers in isolated worktrees (faster, recommended for large refactors)
2. Solo agent — single agent executes sequentially (simpler, recommended for small refactors)
```

The user's choice determines how Phase 3 designs the plan.

## Phase 3: Implementation Planning

Design the implementation plan based on the user's choice from Phase 2. Use existing skills:
- Load the `writing-plans` skill to create a detailed step-by-step plan if one doesn't exist
- Load the `brainstorming` skill for upstream design work if the approach is unclear

### For Parallel Agent Teams

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

**Critical dependency sequencing**: If work stream B depends on A's output, A must complete before B starts. Use task dependencies (`addBlockedBy`) to enforce this.

### For Solo Agent

Load the `subagent-driven-development` or `executing-plans` skill for step-by-step execution with review checkpoints.

### Plan Approval

Present the implementation plan to the user. Include:
- Work streams and their ordering
- Which agents handle which tasks
- Expected merge sequence
- Verification checkpoints

**User approves before any code changes.**

## Phase 4: Guarded Execution

### Worktree Isolation (MANDATORY for parallel agents)

Every implementer/builder/migrator agent MUST work in its own git worktree under `.worktrees/`.

**Setup procedure:**
1. Check if `.worktrees/` exists and is in `.gitignore`
2. If not, load the `using-git-worktrees` skill to set it up (creates directory, verifies gitignore, commits)
3. If `.worktrees/` already exists, skip the skill and create worktrees directly

**Creating per-agent worktrees:**
```bash
git worktree add .worktrees/<agent-name> -b refactor/<agent-name>
```

Each agent works in its isolated worktree. The lead agent (or a merge orchestrator) merges branches back in dependency order.

**When using the Agent tool**, set `isolation: "worktree"` to enforce worktree isolation per agent. This is NON-NEGOTIABLE for parallel execution.

### Invariants (inject into ALL agent prompts)

```
REFACTOR INVARIANTS — these override any conflicting instructions:
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

- **Solo agent**: Run `cargo check -p <crate>` after every file modification. Do not batch.
- **Parallel agents**: Each agent runs `cargo check` within its own worktree. Cross-crate checks happen AFTER merging, led by the Verifier agent.

### The Verifier Agent

**Always staff a Verifier** when using parallel agents. The Verifier:

1. **After each implementer completes a task**: reviews the changes in that agent's worktree, runs `cargo check` and relevant tests, reports findings to the lead
2. **After merging**: runs full workspace verification (`cargo build --workspace`, `cargo nextest run --workspace`, `cargo test --doc --workspace`)
3. **Reports issues to lead**: the lead decides whether to fix now or defer, and assigns fixes to an idle implementer

The Verifier does NOT fix code — it only checks and reports. This separation prevents the verifier from introducing its own bugs.

### Lead Agent Responsibilities

The lead agent (you) orchestrates:

1. **Task assignment**: create tasks, assign to agents, track dependencies
2. **Merge sequencing**: merge worktree branches back in dependency order
3. **Issue triage**: when the Verifier reports problems, decide:
   - Which agent should fix it (prefer an idle agent with context)
   - Whether to fix now or batch fixes after all agents complete
4. **Conflict resolution**: if merges conflict, resolve or delegate

### Guardian Role

Read the Guardian persona (from the team directory, see AGENTS.md). The Guardian runs as the lead agent's advisor:
- Pre-flight analysis (Phase 1)
- Migration checklist production for Migrators
- Post-validation pub-item diffing (Phase 5)

For small refactors, the lead agent can absorb Guardian duties directly.

## Phase 5: Final Validation + Documentation

1. **Verifier runs final checks:**
   - `cargo build --workspace`
   - `cargo nextest run --workspace`
   - `cargo test --doc --workspace`
   - `cargo insta test --workspace` (if snapshot tests exist)
2. **Diff pub items** in changed files against pre-flight list — flag unintended changes
3. Read the **Documenter** persona (from the team directory) to update CLAUDE.md/AGENTS.md/memory if conventions changed
4. Load the `finishing-a-development-branch` skill to complete

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
- An implementer agent NOT running in a worktree during parallel execution
- `cargo check` failing 3+ times on the same error (escalate to user)
- Any agent placing types in a crate not listed in the pre-flight summary
- Verifier auto-fixing code instead of reporting to lead

## Rationalization Table

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Skip pre-flight | "This refactor is simple, I know what needs to move" | 'Simple' refactors have hidden consumers. Pre-flight takes 5 minutes; debugging a missed re-export takes 30. |
| Skip triage-review | "I already know the code well enough" | You know the code. The Formalism reviewer catches abstraction issues. The Soundness Adversary catches invariant violations. Fresh eyes find what familiarity hides. |
| Start coding before plan approval | "I'll adjust the plan based on what I find" | Code-first planning produces sunk-cost pressure to keep bad decisions. Plan approval costs 2 minutes; reworking a wrong approach costs hours. |
| Edit the same file from two agents | "The changes are in different functions" | Git merges on function granularity, not line granularity. Two agents touching the same file creates merge conflicts that require manual resolution. |
| Let the verifier fix issues | "It's faster than dispatching back to the implementer" | The verifier lacks the implementer's context. Verifier fixes introduce new bugs at a higher rate. Report to lead, let the right agent fix it. |
| Skip exploration budget | "I need to read one more file to understand" | The budget exists because unbounded exploration delays the actual work. If 20 reads aren't enough, the scope is wrong — simplify it. |

## Integration

**Required workflow skills (load when needed):**
- The `using-git-worktrees` skill — worktree setup (only if `.worktrees/` doesn't exist yet)
- The `finishing-a-development-branch` skill — completion after Phase 5

**Execution skills (choose based on user's Phase 2 decision):**
- The `subagent-driven-development` skill — solo agent, task-by-task with review
- The `executing-plans` skill — parallel or sequential execution with batch checkpoints

**Optional:**
- The `brainstorming` skill — upstream design work before refactoring
- The `writing-plans` skill — creates detailed plan if one doesn't exist
- The `simplify` skill — post-refactor code cleanup
