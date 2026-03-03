---
name: refactor
description: Use when refactoring Rust code across crates, extracting traits, splitting modules, or renaming across 3+ files. Also auto-suggested when detecting cross-crate moves or trait extractions.
---

# Refactor

## Overview

Rust-specific refactoring orchestration that wraps executing-plans and subagent-driven-development with architectural guardrails and a configurable agent team. Four phases: scope & pre-flight, team staffing, guarded execution, review + documentation.

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

After budget is spent, you MUST have a concrete understanding and proceed to staffing.

**Pre-flight checklist:**

1. For each moved/created type: identify target crate, feature flag, visibility level
2. Read dependent crates' Cargo.toml to map the dependency graph
3. Check for visibility-bridging wrappers in affected code (one-liner methods that bridge pub(crate) to pub)
4. List all `pub` items that will change
5. Present architectural summary to user → **user approves before any code changes**

Output format:
    ```
    ## Pre-flight Summary

    **Refactor scope:** [small/large] ([N] crates affected)

    **Changes:**
    | Item | From | To | Crate | Visibility | Feature Flag |
    |------|------|----|-------|------------|-------------|
    | TraitName | crate-a | crate-b | kirin-X | pub | interpret |

    **Dependent crates:** [list]
    **Visibility bridges to preserve:** [list or "none found"]
    **Public API changes:** [list]
    ```

## Phase 2: Team Staffing

### Available Roles

Read persona files from `../../team/` directory (shared across skills). Each file defines a role's background, perspective, and responsibility.

| Role | File | Staff When |
|------|------|-----------|
| Guardian | `../../team/guardian.md` | Any cross-crate refactor or visibility change |
| Implementer | `../../team/implementer.md` | Always |
| Migrator | `../../team/migrator.md` | When downstream crates are affected |
| PL Theorist | `../../team/pl-theorist.md` | API/trait redesigns, new abstractions |
| Compiler Engineer | `../../team/compiler-engineer.md` | Performance-sensitive changes, derive macro work |
| Physicist | `../../team/physicist.md` | Public API changes, prelude changes |
| Documenter | `../../team/documenter.md` | When conventions or public API surface change |

### Staffing Process

1. Check `../../team/templates/` for similar past refactors
2. Ask user about refactor scope using AskUserQuestion:
   - What's changing? (traits, modules, types, renames)
   - Which crates are affected?
   - Mechanical (rename/move) vs semantic (API redesign)?
   - Downstream API impact?
3. Propose a roster with rationale for each role
4. User confirms or adjusts

### Review Panel Configuration

The three reviewers (PL Theorist, Compiler Engineer, Physicist) can be staffed:
- **Individually**: for focused feedback on one dimension
- **As a debate panel**: for API/trait redesigns where they debate and converge

When PL Theorist and Physicist disagree, surface the disagreement to the user — they do not resolve it themselves.

### Staffing Heuristics

- Simple rename across crates → Implementer + Migrator + Compiler Engineer
- Trait extraction to new crate → Guardian + Implementer + Migrator + Compiler Engineer
- Public API redesign → Guardian + Implementer + Migrator + full Review Panel + Documenter
- Module split within one crate → Implementer + Compiler Engineer
- Convention change → all roles

## Phase 3: Guarded Execution

### Invariants (inject into ALL staffed agent prompts)

```
REFACTOR INVARIANTS — these override any conflicting instructions:
1. Run `cargo check -p <crate>` after EVERY file modification. Do not batch.
2. NEVER use `#[allow(...)]` or ignore comments as fixes for real errors.
3. NEVER remove one-liner wrapper methods without verifying they are not visibility bridges
   (methods that expose pub(crate) internals through a pub interface).
4. NEVER place new types/traits without checking CLAUDE.md crate ownership conventions.
5. Run `cargo nextest run --workspace` before ANY commit.
6. If `cargo check` fails 3 times on the same error, STOP and report the issue.
```

### Execution Delegation

Delegate to one of:
- **subagent-driven-development** — for same-session, task-by-task execution with review
- **executing-plans** — for parallel session execution with batch checkpoints

Map staffed roles to subagent prompts by reading the persona file and prepending the invariants.

**Guardian** runs as lead agent (Phase 1 pre-flight + Phase 4 validation).
**Implementer** maps to the implementer subagent prompt (with invariants prepended).
**Migrator** runs after Implementer, executing the Guardian's migration checklist.
**Reviewers** run after implementation, using their persona as the review lens.
**Documenter** runs last, before final validation.

## Phase 4: Review + Documentation

1. Staffed reviewers run (individual or panel, as determined in Phase 2)
2. If review panel is active: each reviewer produces findings independently, then they debate
3. Documenter updates CLAUDE.md/AGENTS.md/memory if conventions changed
4. Guardian runs final validation:
   - `cargo build --workspace`
   - `cargo nextest run --workspace`
   - `cargo test --doc --workspace`
   - Diff `pub` items in changed files against pre-flight list — flag unintended changes
5. Hand off to finishing-a-development-branch

## Phase 5: Template Capture

After successful refactor, ask: "Save this team configuration as a template?"

If yes, save to `../../team/templates/<name>.md`:
```markdown
# Template: [name]

**Refactor type:** [description]
**Scope:** [N] crates
**Staffed roles:** [list with rationale]
**What worked:** [notes]
**What to adjust:** [notes]
**Date:** [YYYY-MM-DD]
```

## Red Flags — STOP Immediately

- Planning for more than 5 minutes without having started Phase 1 pre-flight
- Rewriting the pre-flight summary more than once
- Making code changes before user approves pre-flight summary
- Implementer and Migrator editing the same file simultaneously
- `cargo check` failing 3+ times on the same error (escalate to user)
- Any agent placing types in a crate not listed in the pre-flight summary

## Integration

**Required workflow skills:**
- `subagent-driven-development` or `executing-plans` — execution delegation
- `finishing-a-development-branch` — completion after Phase 4

**Optional:**
- `brainstorming` — upstream design work before /refactor
- `writing-plans` — creates detailed plan if one doesn't exist
- `simplify` — post-refactor code cleanup
