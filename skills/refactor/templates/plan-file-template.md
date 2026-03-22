# Plan File Template

Use this template when generating per-finding plan files in `wave-N/<slug>-plan.md`.

Each plan file MUST be self-contained: an implementer agent can execute it without
reading the review report, other plan files, or the plan index. Paste finding text
verbatim, quote AGENTS.md conventions inline, include exact file paths and commands.

---

## Template

```markdown
# <Finding Title>

**Finding(s):** <IDs from review report, e.g., P0-1, P1-2>
**Wave:** <N>
**Agent role:** <Builder | Implementer | Migrator>
**Estimated effort:** <quick-win | moderate | design-work>

---

## Issue

<What finding to solve. Include the original review finding text verbatim — or
summarized if multiple findings are grouped. Include enough context that an
implementer who has not read the review report can understand the problem.

For grouped findings, explain why they are coupled and must be fixed together.>

**Crate(s):** <affected crates>
**File(s):** <affected files with line references where available>
**Confidence:** <confirmed | likely | uncertain — from the review>

## Guiding Principles

<Design principles to follow for this specific change. Select and quote the
subset of AGENTS.md conventions that apply to this change. Do not reference
by section name — include the actual text so the agent has it in context.>

- <Quoted convention or design principle>
- <Another principle relevant to this change>

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crate/src/path/file.rs` | modify | <what changes> |
| `crate/src/path/new.rs` | create | <what it contains> |
| `tests/path/test.rs` | modify | <new test or updated test> |

**Files explicitly out of scope:**
- `<file or area>` — <why it is excluded, e.g., "touched by a different plan file">

## Verify Before Implementing

<When the plan depends on assumptions about how existing code works, add
explicit verification steps BEFORE the implementation steps. The implementer
runs these first — if any fails, they STOP and report instead of silently
working around the issue.>

- [ ] **Verify: <assumption to check>**
  Run: `<command>`
  Expected: <what success looks like>
  If this fails, STOP and report — the implementation approach needs to change.

<Common assumptions that need verification in refactoring:>
- "This function exists at this path" — the review may reference stale locations
- "This type implements trait X" — check with cargo check before depending on it
- "This API accepts this pattern" — write a minimal test before building on it
- "Changing X does not break downstream crate Y" — cargo check -p <Y> first
- "Derive macros handle this field type" — add the field and `cargo check` before
  designing the rest of the implementation around it. Derive macros may reject
  certain field types (e.g., `Vec<ResultValue>`) that look valid from reading
  the struct definition alone. Discover this EARLY.
- "Changing a trait bound from `From<T>` to `TryFrom<T>` is non-breaking" —
  verify blanket impls cover existing callers via `cargo build --workspace`

## Regression Test

<The implementation MUST start by writing a test that triggers the bug BEFORE
applying the fix. This applies to ALL severity levels, not just P0/P1. The
test-first sequence is: write test → confirm it fails → apply fix → confirm
it passes. This approach:
1. Proves the bug is real (not a false positive from the review)
2. Confirms the fix actually resolves the issue (test goes from fail to pass)
3. Prevents regressions in future changes

Only skip if writing the test requires significant implementation work that
would be wasted if the fix approach changes — and the implementer must get
lead approval before skipping. If a reproducing test is truly infeasible
(e.g., UB that only manifests under specific runtime conditions), explain why
and describe how the fix will be validated instead.>

- [ ] **Write regression test for <issue description>**
  <Describe the test: what it sets up, what behavior it exercises, and what
  assertion captures the bug. Reference the test file and function name.>
  Test file: `<crate>/tests/<file>.rs` or inline `#[cfg(test)]`

- [ ] **Run the test — confirm it fails (or demonstrates the issue)**
  Run: `<command>`
  Expected: <FAIL with specific error, or demonstrates the problematic behavior>

## Design Decisions (design-work plans only)

<Include this section when estimated effort is "design-work". Omit for
"quick-win" and "moderate" plans where the implementation is straightforward.

Document the design space and decision points the implementer will face.
Each decision should have a primary approach, a fallback, and a verification
step that determines which to use.>

**Decision 1: <title>**
- **Primary approach:** <what to try first>
- **Fallback:** <what to do if primary doesn't work>
- **How to decide:** <concrete verification — e.g., "run cargo check after
  adding the field; if it fails with a derive error, use the fallback">

<Example from a real refactor:
Decision: scf.for result value representation
- Primary: Vec<ResultValue> for multiple loop-carried values (MLIR parity)
- Fallback: single ResultValue (less expressive but derive-compatible)
- How to decide: add Vec<ResultValue> field and cargo check — if derive
  macros reject it, use single ResultValue>

## Implementation Steps

<Ordered steps. Use checkbox syntax for tracking. Each step is one action —
small enough to verify independently before moving on.

Granularity guide:
- "Write the failing test" — one step
- "Run the test to confirm it fails" — one step
- "Implement the change" — one step
- "Run tests to confirm they pass" — one step
- "Run cargo clippy and fix warnings" — one step

Include exact code patterns where they clarify intent, but do not write the
full implementation — the implementer adapts to the current code state.

For P0/P1 findings with a regression test above, the first implementation
step should be the fix, and a subsequent step should re-run the regression
test to confirm it passes.>

- [ ] **Step 1: <title>**
  <What to do. Reference specific functions, types, or patterns.>

- [ ] **Step 2: <title>**
  <What to do.>
  Run: `<command>`
  Expected: <expected output or behavior>

- [ ] ...

## Must Not Do

<Anti-patterns, constraints, things to avoid. Be specific and reference
concrete patterns from the codebase. Include the rationale so the implementer
can judge edge cases.>

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the
  underlying cause. If a suppression seems genuinely necessary, stop and report
  to the lead with the root cause explanation.
- Do NOT leave clippy warnings. Run `cargo clippy -p <crate>` before reporting
  completion and fix all warnings. Workarounds (renaming to `_var`, dead code
  annotations, `#[allow]`) are not acceptable — address the root cause.
- Do NOT <specific anti-pattern> — <rationale>
- Do NOT <another constraint> — <rationale>
- <Project convention, e.g., "No unsafe code (AGENTS.md: all implementations
  MUST use safe Rust)">

## Validation

**Per-step checks** (include expected output so the implementer knows what
success looks like — "Expected: PASS" is better than no expectation):
- After step 1: `<command>` — Expected: <output or behavior>
- After step N: `<command>` — Expected: <output or behavior>

**Final checks:**
```bash
cargo clippy -p <crate>                   # Expected: no warnings
cargo nextest run -p <crate>              # Expected: all tests pass
cargo nextest run -p <downstream-crate>   # Expected: no regressions
cargo test --doc -p <crate>               # Expected: all doctests pass
```

**Snapshot tests:** <yes/no — if yes, run `cargo insta test -p <crate>` and
report changes, do NOT auto-accept>

## Success Criteria

<Higher-level assessment criteria. The spec reviewer (from
subagent-driven-development) uses this section to verify the implementation
meets the original intent, not just compiles.>

1. <Criterion that validates the fix addresses the root cause, not a symptom>
2. <Criterion that validates no regressions in related functionality>
3. <Criterion that validates the change follows the guiding principles>

**Is this a workaround or a real fix?**
<Explicit statement. If it is a workaround, explain what the real fix would be
and why it is deferred. If it is the real fix, state what makes it definitive.>
```

---

## Filling guidance

**Issue section:** Copy the finding's full text from `report.md`. For coupled
findings (e.g., P0-1 + P1-2), include both and explain the coupling. The
review report's "Cross-Cutting Themes" section identifies these groupings.

**Guiding Principles:** Match crate to AGENTS.md section:
- kirin-ir changes → "IR Design Conventions", "No unsafe code"
- derive changes → "Derive Infrastructure Conventions"
- parser changes → "Chumsky Parser Conventions"
- interpreter changes → "Interpreter Conventions"
- test changes → "Test Conventions"

**Verify Before Implementing:** Add verification steps when the plan depends on
assumptions. Common cases in refactoring:
- Review finding references a function/type at a specific line → verify it still exists
- Plan assumes a trait is implemented for a type → `cargo check` a minimal test
- Plan assumes changing X won't break crate Y → `cargo check -p Y` first
- Plan assumes an API accepts a certain pattern → write a compile test first
If the planner verified the assumption during exploration, still include the step —
the code may change between planning and execution.

**Regression Test (all severities):** The implementation MUST start by writing a
test that triggers the bug before applying the fix. This applies to ALL findings,
not just P0/P1. Think about:
- Can you construct an input that triggers the bug? (e.g., nested blocks with
  duplicate SSA names for P0-2 scope shadowing)
- Can you assert on the wrong behavior? (e.g., zeroed type field for P1-2)
- If the issue is UB or a panic, can you write a test that catches it under
  debug/test builds? (e.g., `#[should_panic]`, miri, debug_assert)
Only skip if writing the test requires significant implementation work — get
lead approval first. If truly infeasible, explain why and describe alternative
validation.

**Implementation Steps:** Use `- [ ]` checkbox syntax. One action per step. Include
`Run:` and `Expected:` lines for steps that produce verifiable output. The cycle
is always: write regression test → run (expect fail) → implement fix → run
(expect pass) → clippy → commit.

**Must Not Do:** Always include these two mandatory items plus any finding-specific constraints:
- Never `#[allow(...)]` to suppress warnings — fix the root cause
- All clippy warnings must be resolved before completion — no workarounds
- Never remove visibility bridges without verification
- Never place types in wrong crates
- `cargo check` failure 3x → stop and report

**Design Decisions section:** Include only for "design-work" effort plans. The
key signal: if the implementer will need to make a choice between approaches
that affects the rest of the implementation, document it here. Common triggers:
- Adding new fields to derived structs (derive macros may not support the type)
- Changing trait bounds (cascading effects on downstream impls)
- Adding features to IR types that touch parser + printer + interpreter together
- Any change where "try X first, fall back to Y" is the right strategy

**Slug naming:** Use the agent name from the plan index, or derive from
the finding: `<short-description>-plan.md`. Examples:
`arena-fix-plan.md`, `emit-context-scoping-plan.md`, `graph-unification-plan.md`.
