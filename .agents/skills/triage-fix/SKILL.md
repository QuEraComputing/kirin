---
name: triage-fix
description: Use after /triage-review produces a verified findings report. Walks through each finding with the user to nail down solutions, then dispatches parallel agents to implement all fixes. Handles breaking changes, agent conflicts, and iterative verification.
---

# Triage Fix

## Overview

Implements fixes for verified findings from a triage-review report. Three phases: interview the user on each finding to nail down solutions, dispatch parallel agents to implement, then verify and commit.

**Prerequisite:** A verified `docs/design-issues.md` (or `docs/reviews/*.md`) with actionable findings.

## When to Use

- After `/triage-review` produces a report with accepted findings
- When the user says "fix the findings" or "implement the fixes"
- After a design-issues document has been verified and triaged

**Don't use for:**
- Generating the review itself (use `/triage-review`)
- Single bug fixes (just fix them directly)
- Refactoring without a findings list (use `/refactor`)

## Phase 1: Solution Interview

Walk through each finding with the user using `AskUserQuestion` with markdown previews.

### Ordering

1. **Fix-soon (Medium severity)** — present 1-4 at a time, each with a concrete code preview showing before/after
2. **Fix-when-touched (Low severity)** — present 1-4 at a time, same format

### Preview format

Every option MUST include a `markdown` preview showing:
- The actual source code at the cited location
- A before/after snippet of the proposed fix
- Keep to 15 lines or fewer

### Capture user decisions

For each finding, record:
- **Accept**: which solution variant
- **Won't Fix**: remove from the report
- **Needs Discussion**: dig deeper, propose alternatives, re-ask

### Design discovery

Solutions proposed during review may not work in practice. When the user raises concerns or when implementation reveals constraints:
1. Explain the constraint concretely
2. Propose revised alternatives
3. Re-ask with updated options

Example: "We proposed making `#[wraps]` always forward eval_call, but discovered that not all inner types implement `CallSemantics`. Revised approach: require explicit `#[callable]`."

## Phase 2: Parallel Implementation

### Agent dispatch strategy

**CRITICAL LESSONS from production experience:**

1. **Identify file overlap BEFORE dispatching.** Map each finding to its affected files. If two findings touch the same files, they MUST be sequenced, not parallelized.

2. **Breaking changes need atomic application.** When a trait signature changes (e.g., return type from `T` to `Result<T, E>`), ALL files must be edited before any build. Instruct agents: "Read ALL files first, make ALL edits, then build ONCE at the end."

3. **Group related findings.** Findings affecting the same module (e.g., P-2 and P-3 both in prettyless) should be assigned to a single agent.

### Agent prompt template

For each agent:
```
Fix [FINDING-ID]: [one-line description]

**IMPORTANT: Read ALL files first, make ALL edits, then build ONCE at the end.
Do NOT run cargo build between individual file edits.**

Changes needed:
1. [specific file and change]
2. [specific file and change]
...

After ALL edits:
- cargo build --workspace
- cargo nextest run --workspace (or relevant crate)
```

### Dispatch order

1. **Independent fixes first** — findings that only touch one crate with no downstream dependents
2. **Breaking changes second** — changes that affect trait signatures, error types, or public APIs
3. **Downstream updates last** — test files, snapshot updates, call site propagation

### Handling agent failures

Agents may fail due to:
- **Linter reverting partial changes** — re-launch with "atomic application" instructions
- **Rate limits** — the agent will document what it planned; re-launch with that plan
- **File conflicts with other agents** — wait for conflicting agent to finish, then re-launch
- **Design issues discovered during implementation** — report back to user, revise approach

## Phase 3: Verification and Commit

1. **Full workspace build**: `cargo build --workspace`
2. **Full test suite**: `cargo nextest run --workspace`
3. **Check for partial changes**: `git diff --stat` — verify all expected files are changed
4. **Fix any remaining issues** manually if agents left gaps
5. **Single commit** with conventional commit format listing all findings fixed
6. **Update the design-issues document** to reflect completed fixes

### Commit message format

```
fix: resolve N verified design issues across workspace

Address all findings from design-issues.md triage review:

**Fix soon (Medium severity):**
- FINDING-1: one-line description of fix
- FINDING-2: one-line description of fix

**Fix when touched (Low severity):**
- FINDING-3: one-line description of fix
```

## Red Flags — STOP

- Dispatching agents that touch overlapping files in parallel
- Running cargo build between individual file edits for breaking changes
- Accepting a solution without user confirmation
- Skipping the interview phase and jumping straight to implementation
- Committing with failing tests
- Implementing a fix that the user hasn't approved

## Integration

**Skills this skill uses:**
- `dispatching-parallel-agents` — run fix agents concurrently
- `verification-before-completion` — verify all tests pass before commit
- `insta-snapshot-testing` — update snapshots after codegen changes

**Skills that call this skill:**
- `/triage-review` Phase 3 — after walkthrough, user may say "fix these"

**Related:**
- `/triage-review` — generates the findings this skill fixes
- `/refactor` — for larger structural changes beyond point fixes
