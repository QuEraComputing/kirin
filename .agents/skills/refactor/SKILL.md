---
name: refactor
description: Use when refactoring Rust code across crates, extracting traits, splitting modules, or renaming across 3+ files. Also auto-suggested when detecting cross-crate moves or trait extractions.
---

# Refactor

## Overview

Rust-specific refactoring orchestration with architectural guardrails and a configurable agent team. Six phases: scope & pre-flight, review panel, implementation planning, guarded execution, final validation, template capture.

**Announce at start:** "I'm using the refactor skill to orchestrate this refactoring."

## When to Use

- Explicit: user invokes `/refactor`
- Auto-suggest when detecting: cross-crate moves, trait extractions, module splits, renames across 3+ files

**Don't use for:**
- Single-file changes
- Adding new code that doesn't touch existing abstractions
- Bug fixes that don't change public APIs

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

## Phase 2: Review Panel

**Before any implementation**, run the review panel to evaluate the proposed changes. This catches architectural issues early.

### Panel Staffing

Read persona files from `../../team/` directory. Staff reviewers based on scope:

| Role | File | Staff When |
|------|------|-----------|
| PL Theorist | `../../team/pl-theorist.md` | API/trait redesigns, new abstractions |
| Compiler Engineer | `../../team/compiler-engineer.md` | Performance-sensitive changes, derive macro work |
| Physicist | `../../team/physicist.md` | Public API changes, prelude changes |

### Review Process

1. Spawn reviewers (use `triad-design-review` agent when all three are needed, or individual agents for focused reviews)
2. Each reviewer evaluates the pre-flight summary and proposed changes independently
3. If panel is active: reviewers debate and converge on recommendations
4. When PL Theorist and Physicist disagree, surface the disagreement to the user
5. **Present consolidated review findings to the user**

### User Decision Point

After presenting review findings, ask the user:

```
How would you like to proceed with the implementation?

1. Parallel agent team — multiple implementers in isolated worktrees (faster, recommended for large refactors)
2. Solo agent — single agent executes sequentially (simpler, recommended for small refactors)
```

The user's choice determines how Phase 3 designs the plan.

## Phase 3: Implementation Planning

Design the implementation plan based on the user's choice from Phase 2. Use existing skills:
- `writing-plans` — to create a detailed step-by-step plan if one doesn't exist
- `brainstorming` — for upstream design work if the approach is unclear

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

Use `subagent-driven-development` or `executing-plans` for step-by-step execution with review checkpoints.

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
2. If not, use the `using-git-worktrees` skill to set it up (creates directory, verifies gitignore, commits)
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

Read `../../team/guardian.md`. The Guardian runs as the lead agent's advisor:
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
3. **Documenter** (read `../../team/documenter.md`) updates CLAUDE.md/AGENTS.md/memory if conventions changed
4. Hand off to `finishing-a-development-branch`

## Phase 6: Template Capture

**Automatically prompt** after successful refactor: "Save this team configuration as a template?"

If yes, save to `../../team/templates/<name>.md`:
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

## Integration

**Required workflow skills:**
- `using-git-worktrees` — worktree setup (only if `.worktrees/` doesn't exist yet)
- `finishing-a-development-branch` — completion after Phase 5

**Execution skills (choose based on user's Phase 2 decision):**
- `subagent-driven-development` — solo agent, task-by-task with review
- `executing-plans` — parallel or sequential execution with batch checkpoints

**Optional:**
- `brainstorming` — upstream design work before /refactor
- `writing-plans` — creates detailed plan if one doesn't exist
- `simplify` — post-refactor code cleanup
