---
name: test-coverage-review
description: Use when wanting to improve test coverage while simultaneously discovering design issues, bugs, and API ergonomic problems. Writes new tests targeting uncovered or edge-case code paths; when a test reveals excessive boilerplate, unexpected behavior, or a bug, it stops and reports the finding. Produces a lightweight findings report, then iterates with the user on confirmation and solutions. Use this skill whenever the user mentions improving tests, adding edge case coverage, test-driven review, or discovering issues through testing.
---

# Test Coverage Review

## Overview

Test-driven codebase review. Discovers design issues, bugs, and ergonomic problems by actually writing tests — not by reading code and speculating. Three phases: write tests and collect findings, present findings to the user for confirmation, then iterate on solutions together.

The insight: writing a test is the fastest way to discover whether an API is awkward, a behavior is wrong, or a design forces unnecessary boilerplate. If you can't write a clean test for something, that's a signal worth reporting.

**Announce at start:** "I'm using the test-coverage-review skill to discover issues through test writing."

## When to Use

- User wants to improve test coverage for a crate or subsystem
- User wants to find bugs or design issues through testing
- After a refactor, to verify behavior and catch regressions
- When edge cases haven't been exercised yet

**Don't use for:**
- Comprehensive multi-reviewer codebase review (use `/triage-review`)
- Implementing fixes from an existing review (just fix them directly or use `/refactor`)
- Writing tests for a specific known bug (just write the test)

## Phase 1: Test Writing and Discovery

### Scoping

Ask the user what to cover. Accept a crate name, subsystem, or specific module. If unspecified, look at recent git changes to pick a focus area.

Read the target code thoroughly before writing any tests. Understand the public API, internal invariants, and existing test coverage.

### Test writing strategy

Write tests in priority order:

1. **Uncovered public API paths** — functions/methods with no tests or only happy-path tests
2. **Edge cases** — empty inputs, boundary values, error conditions, type boundaries
3. **Interaction tests** — how components compose (e.g., parse → emit → interpret roundtrip)
4. **Regression seeds** — patterns that historically cause bugs in similar codebases

Follow the project's test conventions from AGENTS.md:
- Roundtrip tests → `tests/roundtrip/`
- Unit tests → inline `#[cfg(test)]`
- Codegen snapshots → inline in derive crates
- New test types → `kirin-test-types`
- New test helpers → `kirin-test-utils`

### Discovery signals — when to stop and report

While writing each test, watch for these signals. When you hit one, **stop writing that test** and record it as a finding:

| Signal | Finding type | Example |
|--------|-------------|---------|
| Test requires >15 lines of setup boilerplate | **Design issue** | "Testing `eval_block` requires manually constructing 6 IR nodes, a pipeline, stage info, and a stack frame" |
| Test reveals behavior that contradicts docs or intuition | **Bug or spec gap** | "Passing an empty block to `eval_block` panics instead of returning `Continue`" |
| Test cannot be written without reaching into private internals | **Encapsulation issue** | "No public way to test successor resolution without constructing a full `StackInterpreter`" |
| Test exposes inconsistent behavior across similar APIs | **Consistency issue** | "`Block::last_statement` returns `Option` but `Region::entry_block` panics on empty" |
| Same test pattern repeated 3+ times with only type changes | **Missing abstraction** | "Every dialect's parse test repeats the same 8-line parser setup; could be a test helper" |
| Test passes but the API it exercises is unnecessarily hard to use | **Ergonomic issue** | "`ParsePipelineText` requires a `CompileStage` even when there's only one stage" |

When a test compiles and passes cleanly with minimal setup, that's a healthy signal — keep it and move on to the next test.

### What to produce

For each test area, either:
- **A passing test** — commit-ready, following project conventions
- **A finding** — with the incomplete test as evidence

Aim for a natural ratio. A good session might produce 5-8 passing tests and 2-4 findings. Don't force findings — if the code is well-designed, report that too.

## Phase 2: Findings Report

### Report format

Summarize all findings. Keep it lightweight — no roleplay panels, no multi-reviewer synthesis. Just the facts from what you observed while testing.

For each finding:

```
### [SEVERITY] [CONFIDENCE] Short title — file:line

**Found while:** writing test for [what you were testing]
**Signal:** [which discovery signal from the table above]

**What happened:**
[2-3 sentences describing what you observed]

**Evidence:**
[The incomplete/problematic test code, or a minimal reproduction]

**Potential direction:**
[1-2 sentences on how this could be addressed — not a full solution, just a direction]
```

#### Severity levels

- **P1 — Bug**: Wrong behavior, panic, unsoundness. The code does something incorrect.
- **P2 — Design issue**: Excessive boilerplate, missing abstraction, encapsulation violation. The code works but the design makes it harder than it should be.
- **P3 — Ergonomic nit**: Inconsistent naming, minor API awkwardness, documentation gap. Low impact but worth noting.

#### Confidence levels

- **certain**: You have a failing test or a clear reproduction demonstrating the issue.
- **likely**: The test evidence strongly suggests an issue, but there may be context you're missing.
- **uncertain**: Something feels off but you're not sure if it's intentional. Frame as a question.

Do not assign P1 to findings with "uncertain" confidence.

### Cross-reference with AGENTS.md

Before finalizing findings, check each one against documented design conventions in AGENTS.md. Drop findings that flag intentional patterns. Note dropped findings briefly at the end so the user can audit.

## Phase 3: User Walkthrough and Solution Iteration

Present findings to the user using `AskUserQuestion`. The goal is not just confirmation — it's a conversation to understand the right solution together.

### Walkthrough procedure

Present findings one at a time (or up to 4 per AskUserQuestion call for lower severity), ordered by severity.

For each finding, include a `markdown` preview showing:
- The actual code at the cited location (or the test that exposed the issue)
- Keep previews to 15 lines or fewer — show only relevant lines with `...` for elided context

#### Question format

```
question: "[P1] [certain] <finding title> — <file:line>"
options:
  - label: "Confirm — let's fix this"
    markdown: |
      ```rust
      // The problematic code or test evidence
      // ... (15 lines max)
      ```
      **Direction:** <potential solution approach>
  - label: "Won't Fix — intentional"
    description: "This behavior is by design"
  - label: "Needs Discussion"
    description: "I want to explore this more"
```

### Solution iteration

When the user selects "Confirm" or "Needs Discussion", dig deeper:

1. **Explain the finding with a concrete example** — show a before/after of what a clean test *would* look like if the issue were fixed
2. **Propose 1-2 solution directions** — not full implementations, just approaches. For example: "We could add a `TestPipelineBuilder` helper in `kirin-test-utils`" or "We could make the `stage` parameter optional with a default"
3. **Ask for the user's preference** — they may have context about why the current design exists
4. **If the user provides new context**, revise the finding accordingly — maybe it's actually intentional, or maybe there's a better solution direction than what you proposed

Continue iterating until the user is satisfied with the direction for each confirmed finding.

### After walkthrough

1. **Commit passing tests** that were written during Phase 1 (with user approval)
2. **Update the findings report** with the agreed solution direction for each confirmed finding
3. Proceed to Phase 4

## Phase 4: Solution Planning and Implementation

After the user has confirmed findings and iterated on solutions, this skill orchestrates the next steps by delegating to the appropriate skills. The key principle: this skill owns the discovery and confirmation loop, then hands off to specialized skills for design exploration, planning, and implementation.

### Step 1: Delegate complex design questions to `/brainstorming`

If any confirmed finding involves a non-obvious design choice — e.g., a new trait boundary, a module restructure, or a change that affects multiple crates — delegate to `/brainstorming` before planning. Signs that brainstorming is needed:

- The user selected "Needs Discussion" and the solution direction is still open
- Multiple findings interact (fixing one affects how another should be fixed)
- The solution involves introducing a new abstraction or changing a public API

Provide `/brainstorming` with the finding, the test evidence, and the solution directions discussed so far. The brainstorming output feeds back into the findings report as a refined solution direction.

### Step 2: Write a fix plan with `/writing-plans`

Once all confirmed findings have clear solution directions (either from the walkthrough or from brainstorming), delegate to `/writing-plans` to produce an implementation plan.

Provide the plan with:
- The updated findings report (with solution directions)
- Which findings can be fixed independently vs. which have dependencies
- Any ordering constraints (e.g., trait changes before call-site updates)

The plan should group findings into implementation units — sets of changes that can be applied and verified together.

### Step 3: Implement fixes with `/subagent-driven-development`

Once the user approves the plan, delegate to `/subagent-driven-development` to execute it. The plan from Step 2 provides the task breakdown; subagent-driven-development handles parallel dispatch, conflict avoidance, and verification.

### When to skip steps

- **Skip brainstorming** if all findings have straightforward fixes (e.g., adding a test helper, fixing a panic on empty input)
- **Skip writing-plans** if there's only 1-2 simple findings — just fix them directly
- **Skip subagent-driven-development** if there's only one finding — fix it inline

Use judgment. The full pipeline (brainstorm → plan → parallel implement) is for sessions that produce 3+ non-trivial findings. For simpler sessions, collapse the steps.

## Red Flags — STOP

- Implementing fixes during Phase 1 (discovery phase writes tests, not fixes)
- Reporting a finding without test evidence or a concrete example
- Assigning P1 severity with "uncertain" confidence
- Skipping the user walkthrough and producing only a report
- Flagging documented AGENTS.md conventions as issues
- Forcing findings when the code is actually well-designed
- Writing tests that only pass because they test trivial/tautological things
- Jumping to implementation without updating the findings report with agreed solutions
- Starting `/subagent-driven-development` without a plan from `/writing-plans` (unless trivially simple)
- Skipping `/brainstorming` for design-heavy findings where the solution direction is unclear

## Integration

**Skills this skill delegates to (Phase 4):**
- `/brainstorming` — explore complex design questions before committing to a solution
- `/writing-plans` — produce an implementation plan from confirmed findings
- `/subagent-driven-development` — execute the plan with parallel agents

**Skills this skill uses (Phases 1-3):**
- `insta-snapshot-testing` — for snapshot-based test discoveries
- `test-driven-development` — follows TDD conventions for test structure
- `verification-before-completion` — verify passing tests before committing

**Related:**
- `/triage-review` — comprehensive multi-reviewer review (heavier weight, read-only)
- `/refactor` — for larger structural changes beyond point fixes
- `/requesting-code-review` — PR-level review (not discovery-oriented)
