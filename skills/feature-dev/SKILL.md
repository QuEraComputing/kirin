---
name: feature-dev
description: Use when building new capabilities — from a single helper method to an entire framework subsystem. Triggers on new feature requests, new subsystem development, multi-crate additions, or any work that needs design exploration before coding. Not for refactoring existing code or implementing approved RFCs.
effort: high
argument-hint: "[feature description]"
---

# Feature Development

## Overview

**Announce at start:** State which skill is being used so the user knows what process is driving behavior.

Orchestrator for building new capabilities — from a single helper method to an entire framework subsystem. Hybrid approach: strict gates at critical checkpoints (design review before implementation, tests before merge), flexible ordering within phases. The skill scales by activating more phases for larger work.


## When to Use

- Building something new (not restructuring existing code — that's the `refactor` skill)
- Adding a new subsystem, framework, analysis pass, or significant capability
- Implementing a feature that needs design exploration before coding
- Multi-session work that benefits from guided checkpoints

**Don't use for:**
- Restructuring existing code (load the `refactor` skill)
- Implementing an already-approved RFC (load the `kirin-rfc-implementer` skill)
- Adding a new dialect (load the `dialect-dev` skill — it has domain-specific phases)
- Bug fixes (load the `systematic-debugging` skill)

## Target

The feature to build: **$ARGUMENTS**

If no target was provided, ask the user what they want to build.

## Scale Detection

Determine the scale at the start to decide which phases to activate:

| Scale | Signals | Phases Active |
|-------|---------|---------------|
| Small | 1-2 files, clear approach, no API design needed | Implement → Review → Complete |
| Medium | 3+ files, new API surface, needs a plan | Design → Plan → Implement → Review → Complete |
| Large / Framework | New subsystem, multiple crates, needs spec or RFC, multi-session | Research → Design → *Gate* → Plan → Implement (iterative) → *Gate* → Complete |

Present the scale assessment to the user for confirmation before proceeding.

## Phase 1: Research & Explore (Large scale only)

Understand the design space before committing to an approach.

1. **Read existing code** — understand the landscape, conventions, and constraints
2. **Research prior art** — papers, existing frameworks, related implementations. Use parallel agents for independent research threads.
3. **Load the `brainstorming` skill** — explore the design space, identify tradeoffs

**Output:** Design context document summarizing research findings, constraints, and candidate approaches.

## Phase 2: Design (Medium+ scale)

Formalize the approach. Scale the formalization to match the work:

| Artifact | When | Skill to Load |
|----------|------|---------------|
| Inline design notes | Medium scale, internal changes | None — write notes in the plan |
| Text format / API spec | New syntax, new public API | The `ir-spec-writing` skill |
| RFC | Large changes affecting multiple crates or user-facing contracts | The `kirin-rfc-writer` skill |

For framework-scale work, you may produce multiple specs/RFCs. Each spec should be independently reviewable.

**Output:** Design artifacts (spec, RFC, or inline notes) ready for review.

### GATE: Design Review (Required for Medium+ scale)

Load the `triage-review` skill scoped to the design artifacts and affected crates.

Suggest including these reviewers:
- **Formalism** (PL Theorist) — always, for abstraction design
- **Ergonomics/DX** (Physicist) — when adding public APIs
- **Dialect Author** — when changing dialect author-facing APIs
- **Soundness Adversary** — when the design involves invariants, unsafe, or trust boundaries

**Gate condition:** User confirms the design direction after reviewing findings. Iterate on design if the review surfaces fundamental issues.

## Phase 3: Plan (Medium+ scale)

Break the implementation into testable units.

- **Medium:** Load the `writing-plans` skill to produce a step-by-step plan
- **Large:** Load the `writing-plans` skill with explicit ordering constraints and milestone markers

Each unit should be:
- Independently compilable (`cargo check` passes after each unit)
- Independently testable (at least one test verifies the unit's behavior)
- Small enough for a single agent to implement

For large-scale work, group units into **milestones**. Each milestone is a natural review point.

**Output:** Implementation plan with ordered tasks and milestone markers.

Present the plan to the user for approval before implementation.

## Phase 4: Implement (All scales)

Execute the plan. Scale the execution to match the work:

| Scale | Execution Strategy |
|-------|--------------------|
| Small | Implement directly — no subagent needed |
| Medium | Load the `subagent-driven-development` skill — sequential tasks with per-task review |
| Large | Load the `subagent-driven-development` skill with worktree isolation — parallel where tasks are independent |

For large-scale work with milestones:
- After each milestone, pause and verify:
  - Does the implementation match the design spec/RFC?
  - Do all tests pass?
  - Is the approach still sound, or does the design need revision?
- If the design needs revision, go back to Phase 2. This is expected and healthy for framework-level work.

### GATE: Implementation Review (Required)

After implementation is complete (or at each milestone for large work):

- Load the `test-coverage-review` skill to verify coverage and discover edge cases
- Load the `triage-review` skill for multi-perspective review of the implementation
- If a spec/RFC exists, verify implementation matches it

**Gate condition:** Review findings are addressed. No P0/P1 issues remain.

## Phase 5: Complete (All scales, strict)

1. Load the `verification-before-completion` skill — run all checks
2. Load the `finishing-a-development-branch` skill — merge, cleanup

## Red Flags — STOP

- Starting implementation before design review gate passes (Medium+ scale)
- Skipping the plan for Medium+ scale work
- Implementing more than one milestone without pausing to verify
- Framework-scale work without a spec or RFC
- Proceeding after review surfaces P0/P1 issues without addressing them
- Parallel agents without worktree isolation

## Rationalization Table

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Skip design for medium work | "It's only 3 files, I can figure it out as I go" | 3 files with a new API surface means design decisions. Coding without design means the first approach becomes permanent, even if a 5-minute design phase would have found a better one. |
| Skip design review gate | "The design is straightforward, review would be rubber-stamping" | If the design is truly straightforward, the review takes 2 minutes. If it's not (and you didn't realize), the review catches it before you've written 500 lines. |
| Blast through milestones | "I'm in flow, the next milestone is similar" | Each milestone is a checkpoint. Bugs compound across milestones — a wrong assumption in milestone 1 becomes a structural problem by milestone 3. Pause, verify, continue. |
| Treat large work as medium | "It's a big feature but the approach is clear" | If it spans multiple crates or needs a spec, it's large. Calling it medium to skip phases saves 30 minutes of planning and costs hours of rework. |

## Integration

**Skills this orchestrator composes (load when needed):**

| Layer | Skill | When |
|-------|-------|------|
| 3 | `triage-review` | Design review gate, implementation review gate |
| 3 | `test-coverage-review` | Implementation review gate |
| 2 | `brainstorming` | Phase 1 exploration |
| 2 | `writing-plans` | Phase 3 planning |
| 2 | `subagent-driven-development` | Phase 4 execution |
| 2 | `finishing-a-development-branch` | Phase 5 completion |
| 1 | `verification-before-completion` | Phase 5 checks |
| Domain | `ir-spec-writing` | Phase 2 spec writing |
| Domain | `kirin-rfc-writer` | Phase 2 RFC writing |
