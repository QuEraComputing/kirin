---
name: test-coverage-review
description: Use when wanting to improve test coverage while simultaneously discovering design issues, bugs, and API ergonomic problems. Writes new tests targeting uncovered or edge-case code paths; when a test reveals excessive boilerplate, unexpected behavior, or a bug, it stops and reports the finding. Produces a lightweight findings report, then iterates with the user on confirmation and solutions. Use this skill whenever the user mentions improving tests, adding edge case coverage, test-driven review, or discovering issues through testing.
---

# Test Coverage Review

## Overview

Test-driven codebase review. Discovers design issues, bugs, and ergonomic problems by actually writing tests — not by reading code and speculating. Three phases: dispatch agents to write tests and collect findings autonomously, merge findings and present them to the user, then iterate on solutions together.

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

## Phase 1: Test Writing and Discovery (Agent-Driven)

### Scoping

Ask the user what to cover. Accept a crate name, subsystem, or specific module. If unspecified, look at recent git changes to pick a focus area.

### Dispatching test agents

Partition the scope into independent work areas (e.g., by crate, module, or subsystem). Launch agents in parallel — each agent owns a slice of the codebase and works autonomously.

The orchestrator picks a `<title>` slug summarizing the review scope (e.g., `test-coverage`, `interpreter-edge-cases`). Agent findings go into a shared directory: `design/reviews/review-<date>-<title>/`. Each agent writes to its own file within that directory.

Each agent's prompt must include:
1. Which files/modules to cover
2. The test writing strategy and discovery signals (below)
3. **A unique findings document path** — `design/reviews/review-<date>-<title>/review-<date>-<area-slug>.md`
4. Instructions to write findings to that document as they're discovered (not at the end)
5. The finding format template (below)

The orchestrator does NOT read or merge findings during Phase 1. Let agents run to completion.

### Agent instructions (include in each agent prompt)

#### Test writing strategy

Read the target code thoroughly before writing any tests. Understand the public API, internal invariants, and existing test coverage.

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

#### Discovery signals — when to stop and report

While writing each test, watch for these signals. When you hit one, **stop writing that test**, record it as a finding in your findings document, then continue to the next test:

| Signal | Finding type | Example |
|--------|-------------|---------|
| Test requires >15 lines of setup boilerplate | **Design issue** | "Testing `eval_block` requires manually constructing 6 IR nodes, a pipeline, stage info, and a stack frame" |
| Test reveals behavior that contradicts docs or intuition | **Bug or spec gap** | "Passing an empty block to `eval_block` panics instead of returning `Continue`" |
| Test cannot be written without reaching into private internals | **Encapsulation issue** | "No public way to test successor resolution without constructing a full `StackInterpreter`" |
| Test exposes inconsistent behavior across similar APIs | **Consistency issue** | "`Block::last_statement` returns `Option` but `Region::entry_block` panics on empty" |
| Same test pattern repeated 3+ times with only type changes | **Missing abstraction** | "Every dialect's parse test repeats the same 8-line parser setup; could be a test helper" |
| Test passes but the API it exercises is unnecessarily hard to use | **Ergonomic issue** | "`ParsePipelineText` requires a `CompileStage` even when there's only one stage" |

When a test compiles and passes cleanly with minimal setup, that's a healthy signal — keep it and move on to the next test.

#### Finding document format

Each agent writes to its own findings document. The document has two sections:

```markdown
# Test Coverage Review — <area> — <date>

## Tests Written

| File | New Tests | Focus |
|------|-----------|-------|
| ... | ... | ... |

## Findings

### [SEVERITY] [CONFIDENCE] Short title — file:line

**Found while:** writing test for [what you were testing]
**Signal:** [which discovery signal from the table above]

**What happened:**
[2-3 sentences describing what you observed]

**Evidence:**
[The incomplete/problematic test code, or a minimal reproduction]

**Potential direction:**
[1-2 sentences on how this could be addressed — not a full solution, just a direction]

## Dropped Findings

- **<title>** — <reason it was dropped, e.g., intentional per AGENTS.md>
```

Agents update the document incrementally — append each finding as it's discovered, don't batch them. This way if the agent is interrupted, findings aren't lost.

#### Severity and confidence levels

Severity:
- **P1 — Bug**: Wrong behavior, panic, unsoundness. The code does something incorrect.
- **P2 — Design issue**: Excessive boilerplate, missing abstraction, encapsulation violation, compile failure. The code works (or doesn't) but the design makes it harder than it should be.
- **P3 — Ergonomic nit**: Inconsistent naming, minor API awkwardness, documentation gap. Low impact but worth noting.

Confidence:
- **certain**: You have a failing test or a clear reproduction demonstrating the issue.
- **likely**: The test evidence strongly suggests an issue, but there may be context you're missing.
- **uncertain**: Something feels off but you're not sure if it's intentional. Frame as a question.

Do not assign P1 to findings with "uncertain" confidence.

Cross-reference each finding against AGENTS.md design conventions before writing it. Drop findings that flag intentional patterns — note them in "Dropped Findings."

### What agents produce

For each test area, either:
- **A passing test** — commit-ready, following project conventions
- **A finding** — written to the findings document with evidence

Aim for a natural ratio. Don't force findings — if the code is well-designed, report that too.

## Phase 2: Merge and Verify

After all agents complete:

1. **Verify the build** — run `cargo nextest run --workspace` to confirm all new tests pass
2. **Fix compilation errors** — if any agent introduced broken tests, fix them before proceeding
3. **Read all agent findings documents** — collect findings from each file in `design/reviews/review-<date>-<title>/`
4. **Merge into a single report** — create `design/reviews/review-<date>-<title>.md` (at the same level as the agent directory) with a combined tests table and all findings, ordered by severity (P1 first, then P2, then P3)
5. **Deduplicate** — if multiple agents found the same issue, keep the one with stronger evidence
6. **Clean up** — delete the agent directory `design/reviews/review-<date>-<title>/` and its contents, since the merged report now contains everything

The orchestrator should not re-discover or re-investigate findings at this stage. Trust the agents' reports — just merge and organize.

## Phase 3: Findings Interview

Present findings to the user using `AskUserQuestion`. The goal is not just "which do you want to fix?" — it's a conversation where you explain each finding clearly so the user can make an informed decision.

### Interview procedure

Walk through findings one at a time, ordered by severity (P1 first). For each finding, use `AskUserQuestion` with:

1. **A clear question** — the finding title with severity and confidence
2. **Option descriptions** that explain why it matters and what each choice means
3. **Preview panels** on options that show code — the problematic code, the test evidence, and/or a before/after fix example

#### AskUserQuestion format

Use `preview` fields on options to show code in the side-by-side panel. The `description` field explains the option; `preview` shows the code.

```
question: "[P2] [certain] `Bound::Finite(i64::MIN).negate()` panics in debug mode"
options:
  - label: "Fix it"
    description: "Use checked_neg() and map overflow to PosInf"
    preview: |
      // bound.rs:85 — current (panics on i64::MIN):
      Bound::Finite(v) => Bound::Finite(-v),

      // proposed fix:
      Bound::Finite(v) => match v.checked_neg() {
          Some(neg) => Bound::Finite(neg),
          None => Bound::PosInf,
      },
  - label: "Won't fix — intentional"
    description: "i64::MIN is not a valid bound in practice — mark as known limitation"
  - label: "Needs discussion"
    description: "Not sure PosInf is right for the overflow case — let's talk semantics"
    preview: |
      // -i64::MIN would be i64::MAX + 1
      // Options: PosInf, saturate to i64::MAX, or error
```

The key difference from a plain list: each option includes enough context that the user can decide without having to go read the code themselves. The "Fix it" option shows both the problem and the solution. The "Won't fix" option explains when ignoring it is reasonable. The "Needs discussion" option names the specific ambiguity.

**Preview size constraint:** The `preview` field renders in a side-by-side panel with limited vertical space. Keep code snippets to **15 lines or fewer** — show only the relevant lines with `// ...` for elided context. If the finding involves a large function, extract just the problematic lines plus 1-2 lines of surrounding context. Never paste an entire function or file into a preview.

### After each response

- **"Fix it"**: Record the decision. Move to the next finding.
- **"Won't fix"**: Record as intentional. Move to the next finding.
- **"Needs discussion"**: Dig deeper — propose 1-2 alternative approaches, show tradeoffs, ask a more specific follow-up question. Continue until the user is satisfied.

### Batching lower-severity findings

For P3 findings, you can batch up to 3-4 per `AskUserQuestion` call. Still include code snippets and explanations for each, but present them as a group with a single set of options (e.g., "Fix all", "Fix #1 and #3 only", "Skip all").

### After the interview

1. **Update the merged findings report** with the user's decisions (confirmed, won't fix, needs discussion resolution)
2. **Commit passing tests** with user approval
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

Once all confirmed findings have clear solution directions (either from the interview or from brainstorming), delegate to `/writing-plans` to produce an implementation plan.

Provide the plan with:
- The updated findings report (with solution directions and user decisions)
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
- Asking the user mid-Phase-1 about findings (agents work autonomously, findings go to documents)
- Presenting findings as a bare list without code snippets or explanations
- Flagging documented AGENTS.md conventions as issues
- Forcing findings when the code is actually well-designed
- Writing tests that only pass because they test trivial/tautological things
- Jumping to implementation without the findings interview
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
