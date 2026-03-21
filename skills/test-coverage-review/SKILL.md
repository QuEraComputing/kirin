---
name: test-coverage-review
description: Use when test coverage needs improvement, edge cases are unexercised, or design issues and bugs should be discovered through test writing rather than code reading. Triggers on requests to improve tests, add edge case coverage, find bugs through testing, verify behavior after a refactor, or do test-driven review of a crate or subsystem.
effort: high
argument-hint: "[crate, subsystem, or module to cover]"
---

# Test Coverage Review

## Overview

Test-driven codebase review. Discovers design issues, bugs, and ergonomic problems by actually writing tests — not by reading code and speculating. Three phases: dispatch agents to write tests and collect findings autonomously, merge and verify findings, then walk through findings with the user for confirmation.

The insight: writing a test is the fastest way to discover whether an API is awkward, a behavior is wrong, or a design forces unnecessary boilerplate. If you can't write a clean test for something, that's a signal worth reporting.


## When to Use

- User wants to improve test coverage for a crate or subsystem
- User wants to find bugs or design issues through testing
- After a refactor, to verify behavior and catch regressions
- When edge cases haven't been exercised yet

**Don't use for:**
- Comprehensive multi-reviewer codebase review (load the `triage-review` skill)
- Implementing fixes from an existing review (just fix them directly or load the `refactor` skill)
- Writing tests for a specific known bug (just write the test)

## Phase 1: Test Writing and Discovery (Agent-Driven)

### Scoping

The review scope is: **$ARGUMENTS**

If no scope was provided, ask the user what to cover. Accept a crate name, subsystem, or specific module. If still unspecified, look at recent git changes to pick a focus area.

### Dispatching test agents

Partition the scope into independent work areas (e.g., by crate, module, or subsystem). Launch agents in parallel — each agent owns a slice of the codebase and works autonomously.

The orchestrator picks a `<title>` slug summarizing the review scope (e.g., `test-coverage`, `interpreter-edge-cases`). Agent findings go into a shared directory under the review output directory (see AGENTS.md Project structure) as `review-<date>-<title>/`. Each agent writes to its own file within that directory.

Each agent's prompt must include:
1. Which files/modules to cover
2. The test writing strategy and discovery signals (below)
3. **A unique findings document path** — `<review-dir>/review-<date>-<title>/review-<date>-<area-slug>.md`
4. Instructions to write findings to that document as they're discovered (not at the end)
5. The finding format template (below)
6. **Persona guidance** (see below)

### Agent persona guidance

Match agent persona to the scope being tested:

| Scope | Persona Lens | What to Emphasize |
|-------|-------------|-------------------|
| Core IR | Code Quality (Implementer persona, review mode) | Invariant violations, API misuse paths, missing `#[must_use]` |
| Parser/printer | Code Quality + Ergonomics (Physicist persona) | Roundtrip correctness, error message quality, parse failure edge cases |
| Interpreter | Code Quality + Formalism (PL Theorist persona) | Semantic preservation, lattice law violations, control flow edge cases |
| Dialect crates | Dialect Author persona (with domain context from AGENTS.md) | Domain-framework alignment, derive experience, boilerplate ratio |
| Derive macros | Code Quality + Compiler Engineer persona | Generated code quality, error message quality, attribute edge cases |
| Builder/arena/interpreter | Soundness Adversary persona | Invariant violations, stale ID attacks, bypass paths, unsafe audit |

For **dialect crates**, include domain context from the triage-review Domain Context Resolution table (e.g., ZX calculus for quantum dialects, compiler engineering for SCF). This lets the test agent write domain-meaningful tests, not just structural ones.

Read the relevant persona file from the team directory (see AGENTS.md Project structure) and include its content in the agent prompt alongside the test writing strategy. The persona informs *what kind of issues to look for* — the test writing strategy informs *how to find them*.

The orchestrator does NOT read or merge findings during Phase 1. Let agents run to completion.

### Agent instructions (include in each agent prompt)

#### Test writing strategy

Read the target code thoroughly before writing any tests. Understand the public API, internal invariants, and existing test coverage.

Write tests in priority order:

1. **Uncovered public API paths** — functions/methods with no tests or only happy-path tests
2. **Edge cases** — empty inputs, boundary values, error conditions, type boundaries
3. **Interaction tests** — how components compose (e.g., parse → emit → interpret roundtrip)
4. **Regression seeds** — patterns that historically cause bugs in similar codebases

Follow the project's test conventions from AGENTS.md (see the Test Conventions section for where each test type belongs).

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
| Dialect operation's IR encoding doesn't naturally express a domain concept | **Domain alignment** | "ZX spider fusion requires manually building two statements + a rewrite; should be one operation" |
| Adding a new operation requires touching 4+ files or 3+ derives | **Framework friction** | "Adding `TweezerPulse::Ramp` required edits in lib.rs, interpret_impl.rs, Cargo.toml, and test fixtures" |
| Test can construct invalid state through the public API without unsafe | **Soundness hole** | "Can create a `Block` with terminator pointing to a statement in a different block — no validation" |
| `debug_assert!` guards an invariant that isn't checked in release | **Release-mode gap** | "Graph node existence only validated by `debug_assert!` — release build silently accepts invalid nodes" |

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
3. **Read all agent findings documents** — collect findings from each agent's file in the review directory
4. **Merge into a single report** — create `<review-dir>/review-<date>-<title>.md` (at the same level as the agent directory) with a combined tests table and all findings, ordered by severity (P1 first, then P2, then P3)
5. **Deduplicate** — if multiple agents found the same issue, keep the one with stronger evidence
6. **Clean up** — delete the per-agent directory and its contents, since the merged report now contains everything

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
3. This skill ends after the interview. See Next Steps below.

## Next Steps (After Discovery)

This skill is **read-only** — it writes tests and produces a findings report with the user's decisions. It does not implement fixes. See AGENTS.md Skill Architecture for the composition model.

To act on confirmed findings:
- **Design-heavy findings**: Load the `brainstorming` skill, then the `writing-plans` skill
- **Multiple findings with dependencies**: Load the `writing-plans` skill to sequence them, then the `subagent-driven-development` skill to execute
- **Simple fixes (1-2 findings)**: Fix them directly — no orchestration skill needed
- **Structural changes**: Load the `refactor` skill

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

## Integration

**Skills this skill uses (load when needed):**
- The `insta-snapshot-testing` skill — for snapshot-based test discoveries
- The `test-driven-development` skill — follows TDD conventions for test structure
- The `verification-before-completion` skill — verify passing tests before committing

**Related:**
- The `triage-review` skill — comprehensive multi-reviewer review (heavier weight, read-only)
- The `refactor` skill — for larger structural changes beyond point fixes
- The `requesting-code-review` skill — PR-level review (not discovery-oriented)
