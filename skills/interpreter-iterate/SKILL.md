---
name: interpreter-iterate
description: Use to autonomously iterate on the kirin interpreter framework design in a self-directed research loop. Triggers when the user wants to improve, redesign, or extend the interpreter framework (kirin-interpreter-*), or wants to run the autoresearch loop for convergence. Also triggers on phrases like "next interpreter iteration", "critique interpreter design", "iterate interpreter", "new interpreter crate", "run the interpreter loop". The skill runs multiple iterations autonomously — critiquing, designing, implementing, testing, committing — and stops only when convergence criteria are met or iteration budget is exhausted. The user can sleep; the skill keeps going.
effort: high
argument-hint: "[iteration number or 'next' or specific goal]"
---

# Interpreter Iterate

**Announce at start:** State which skill is being used so the user knows what process is driving behavior.

## Overview

This skill is an **autonomous research loop** for the kirin interpreter framework. It runs without waiting for user approval between iterations. The user can initiate it and step away — the skill determines when convergence is reached.

Each iteration:
1. Critiques the current design (spawns a critic subagent)
2. Scores the critique to decide whether another iteration is warranted
3. Designs the next iteration based on findings
4. Implements the new `kirin-interpreter-<N>` crate + dialect submodules + toy-lang
5. Runs tests — fixes failures autonomously (may loop back to design if failures reveal flaws)
6. Logs findings to `docs/log.md` (gitignored)
7. Commits implementation (never docs/)
8. Loops back to step 1 unless convergence criteria are met

## Target

Iteration goal: **$ARGUMENTS**

If no target is given, determine the next iteration number from `crates/kirin-interpreter-*` and proceed.

---

## Non-Negotiable Requirements

Every iteration must preserve these. **Never reduce them.** Adding new features or test coverage is encouraged; removing existing capabilities is a regression.

### Required Feature Set (all must be present and tested)

1. **Concrete interpretation** — single-stage (source HighLevel) and multi-stage (HighLevel + LowLevel)
2. **Abstract interpretation** — single-stage (source and lowered) and multi-stage; interval domain and type-lattice domain both tested
3. **SCF support in abstract mode** — `scf.if` and `scf.for` must work under the abstract interpreter (not just concrete)
4. **Cross-stage calls** — source-stage functions calling lowered-stage functions (and vice-versa where appropriate)
5. **Lift/Project algebra** — zero-cost, enum-based, no heap allocation; covers cursor coproducts, and any other total/dialect objects

### Required Test Coverage (these tests must exist and pass every iteration)

In `example/toy-lang/src/interpreter<N>.rs`:
- `test_add_highlevel`, `test_factorial`, `test_abs_positive`, `test_abs_negative` (concrete, single-stage)
- `interval_add_known_range`, `interval_branch_joins_both_paths`, `interval_factorial_converges` (abstract, lowered, interval)
- `toytype_add_highlevel_abstract`, `toytype_abs_highlevel_abstract`, `toytype_factorial_highlevel_abstract` (abstract, source, type lattice)
- `multi_cross_stage_source_calls_lowered`, `multi_cross_stage_double_five`, `multi_same_stage_call_through_dispatch` (concrete, multi-stage)
- `abstract_multi_same_stage_type_propagates`, `abstract_multi_cross_stage_type_propagates`, `interval_cross_stage_doubles_range` (abstract, multi-stage)

New iterations may **add** tests beyond this baseline (for new features, extensibility probes, etc.) but must never remove or weaken existing ones.

### Design Constraints (hard)
- No unsafe code, no `mem::transmute`, no raw pointers
- No `Box<dyn Trait>` in core framework APIs — use generics and enums
- No `Arc`/`Rc` in core framework APIs
- Dialect-local: `Interpretable<E>` implemented in the dialect crate or toy-lang example, never in the interpreter crate
- Cursor types defined by the user (in toy-lang), not imposed by the interpreter crate

---

## Convergence Criteria

The loop terminates autonomously when **all** of the following hold:

1. **Rubric weighted score ≤ 8** (see Phase 2 scoring)
2. **R1 (completeness) ≥ 4** — all non-negotiable features present and tested
3. **R6 (type correctness) ≥ 4** — no unsafe, no unsound lifetime casts
4. **Extensibility probe passed** (R8 = 5): at least one new analysis implemented without touching any interpreter crate
5. **Iteration budget**: at most 5 iterations per session unless the user specified more

If the budget is reached without convergence, stop, commit what's done, log the open issues and current rubric scores, and tell the user what remains.

### Extensibility Probe

Once the weighted score falls below 20 for the first time (baseline is stable enough), trigger the extensibility probe. Implement a new analysis entirely within `example/toy-lang/src/` — no changes to any interpreter crate or dialect crate. Good candidates:

- **Liveness analysis**: abstract domain over `HashSet<SSAValue>` tracking live values at each program point
- **Constant propagation**: abstract domain `ConstProp { Concrete(i64), Top }` — verifies the framework handles non-lattice-join semantics cleanly
- **Type inference**: verify ToyType propagates correctly through SCF and cross-stage calls with a richer lattice

The probe **passes** (R8 = 5) if:
- Implemented entirely in `example/toy-lang/src/`
- Has at least one passing test
- Requires no `unsafe`, `'static`, or `Box<dyn Trait>` in the framework

Log: "Extensibility probe: PASS/FAIL — <what was attempted, what friction was encountered>".

---

## Autonomous Loop Protocol

This skill runs in auto mode. Do not pause for user confirmation between iterations. Make decisions using the scoring and criteria above.

**When to stop and ask the user:**
- A design choice has two roughly equal options and the wrong choice would require a full rewrite (e.g., a fundamental algebraic question)
- A test failure cannot be fixed without understanding user intent (e.g., ambiguous expected behavior)
- The iteration budget is reached

**When to proceed autonomously:**
- All other decisions, including which pain points to address, what features to add, and how to structure implementations

At the **start** of the session, send one message: "Running interpreter-iterate autonomously. Will notify you when done or when I need a decision."

At the **end** of the session, send one message summarizing: iterations run, final convergence score, which requirements are satisfied, and what remains open.

---

## Phase 1: Baseline Critique (runs once at session start)

Determine the current iteration number:

```bash
ls crates/ | grep 'kirin-interpreter-[0-9]' | sort -V
```

The current (most recent) iteration is `max(existing)`. The next iteration N = current + 1.

Run the critic subagent against the current codebase. This is the only critic run that happens *before* an iteration — every subsequent critique happens *after* a commit (see Phase 7). Do not prompt the user.

Spawn a read-only critic subagent (`dontAsk` mode — never `bypassPermissions`). Brief it with:
- All source files in `crates/kirin-interpreter-<current>/src/`
- `example/toy-lang/src/interpreter<current>.rs`
- The full rubric below (copy it verbatim into the critic brief)
- `docs/log.md` history (so the critic doesn't re-report already-addressed issues)

### Critic Brief

The critic must produce a **structured report** with two parts: (1) a rubric scorecard, (2) per-finding review notes. The critic reads code, does not write any.

#### Part 1 — Rubric Scorecard

Score each dimension 1–5 using the rubric table below. A score of 5 means fully satisfied; 1 means critically broken. Record each score and a one-sentence justification.

| # | Dimension | 5 (Excellent) | 3 (Acceptable) | 1 (Critical gap) |
|---|-----------|--------------|----------------|-----------------|
| R1 | **Requirement completeness** | All non-negotiable features present and tested (concrete + abstract, single + multi-stage, SCF, cross-stage calls) | Most features present, minor gaps | A required feature is missing or broken |
| R2 | **Lift/Project algebra** | Zero-cost enum-based lift/project with no heap allocation; consistent across cursor, effect, and value types | Works for cursors but not consistently applied elsewhere | Missing, unsound, or requires heap allocation |
| R3 | **Dialect locality** | Dialect authors implement only `Interpretable<E>`; cursor types and dispatch live in user code; zero interpreter-crate changes needed for new dialects | Minor leakage — one or two interpreter-internal concepts exposed | Dialect authors must edit the interpreter crate |
| R4 | **Mode uniformity** | `Interpretable<E>` works identically for concrete and abstract modes; pure ops have a single generic impl; mode-specific ops use `E::Mode` discriminant only where necessary | Mostly uniform; a few ops duplicated unnecessarily | Separate traits or duplicate impls for concrete vs. abstract |
| R5 | **Boilerplate burden** | Dialect authors write ≤ 1 impl per op type; composition is mechanical enough to be derived; no repeated type bounds copy-paste | Moderate repetition but contained to well-marked `// TODO: derive` sites | Extensive manual impl repetition with no clear derive path |
| R6 | **Type-system correctness** | No `'static` bounds, no `unsafe`, no `Box<dyn Trait>` in framework APIs; `'ir` lifetime threads correctly through all borrows | `'static` used only in abstract interp pipeline borrow (known limitation, tracked) | `unsafe`, `transmute`, or unsound lifetime casts present |
| R7 | **Algebraic elegance** | Lift/Project, Mode, Cursor, and Env form a coherent algebra; naming is consistent; a new developer can predict the pattern from one example | Mostly coherent; some naming inconsistencies or ad-hoc special cases | Ad-hoc design; each new case requires a novel pattern |
| R8 | **Extensibility** | A new analysis or interpreter type can be added by implementing traits in user code only; demonstrated by at least one extensibility probe test | Framework is extensible in theory but probe not yet written | Framework requires core changes to add new interpreter types |

**Overall iteration grade** = average of R1–R8, rounded to one decimal. Report it prominently.

#### Part 2 — Per-Finding Review Notes

For each issue found (severity ≥ Medium), write a structured finding:

```
Finding #<K>
Dimension: R<N> — <dimension name>
Severity: Critical | High | Medium | Low
Location: <file>:<line> (or range)
Observation: <what the code does — factual, no editorializing>
Problem: <why this violates the rubric or requirements — cite the specific rubric cell>
Suggestion: <concrete, actionable change — specific enough that an implementer can act without asking follow-up questions; propose the new trait signature, the new type, or the new abstraction if applicable>
Effort: <estimate: trivial | small | medium | large>
```

Findings must be grounded in the code — cite lines, not vibes. The suggestion field is required (unlike before, the critic *does* propose solutions here — specific ones). Do not suggest vague "refactor this" or "improve this"; propose the actual trait, type, or pattern change.

### Scoring and Convergence Decision

After the critic returns, compute the **weighted convergence score**:

```
score = Σ (6 - dimension_score) * weight
```

| Dimension | Weight |
|-----------|--------|
| R1 (completeness) | 5 |
| R2 (lift/project) | 3 |
| R3 (dialect locality) | 4 |
| R4 (mode uniformity) | 3 |
| R5 (boilerplate) | 2 |
| R6 (type correctness) | 4 |
| R7 (elegance) | 2 |
| R8 (extensibility) | 3 |

**Convergence** when weighted score ≤ 8 AND R1 ≥ 4 AND R6 ≥ 4 (completeness and type correctness are never negotiable).

Record the rubric scores, overall grade, and weighted score in `docs/log.md`. This score is the baseline that all subsequent iterations are compared against.

If convergence criteria are already met (weighted score ≤ 8, R1 ≥ 4, R6 ≥ 4, extensibility probe passed), stop immediately — nothing to do.

---

## Phase 2: Design the Next Iteration

Based on the highest-scoring findings, design changes. Prefer targeted fixes over full redesigns — only redesign what's broken. Write down (in your own reasoning, not to disk):

1. What changes, and which finding it addresses
2. API surface delta (new/changed traits, types, associated types)
3. Algebra delta (Lift/Project changes, Mode discriminant changes, new cursor types)
4. What stays the same — name it explicitly to avoid accidental breakage

Then update `docs/design_principles.md` to reflect the current intended design. This file is forward-looking (what the design *should* be now), not a history — remove stale principles.

**Do not wait for user approval.** Proceed to implementation.

---

## Phase 3: Implement

Each iteration starts from a blank slate. Do **not** copy the previous crate — that would carry forward its structural biases and prevent fundamental redesign. Read the previous iteration's code for reference and understanding, but write the new one from scratch based on the design from Phase 2.

### 3a: New crate scaffold

Create a fresh crate:

```bash
cargo new --lib crates/kirin-interpreter-<N>
```

Add to workspace `Cargo.toml` members list. Write a new `Cargo.toml` with only the dependencies the new design actually needs — do not inherit the previous iteration's dependency list. Dependencies that were needed for a discarded abstraction should not carry over.

Write all source files from scratch based on Phase 2's design. Reading the previous iteration's source for orientation is fine; copying it is not. If a module from the previous iteration is unchanged by the new design, rewrite it anyway — this surfaces hidden coupling and keeps the new crate self-consistent.

Project conventions (from AGENTS.md):
- `mod.rs` for multi-file modules, kept lean (only `mod` + `pub use`)
- No unsafe code
- Mark manual impls with `// TODO: replace this with derive macro`

### 3b: Dialect submodules

For each affected dialect (`kirin-scf`, `kirin-function`, etc.), write a new submodule `interpreter<N>` from scratch under `crates/<dialect>/src/`. Do not copy from `interpreter<prev>` — the new design may require fundamentally different trait impls. Declare with `pub mod interpreter<N>;` in the dialect's `lib.rs`/`mod.rs`.

### 3c: Toy-lang example

Write `example/toy-lang/src/interpreter<N>.rs` from scratch. The **test cases** are fixed (see Required Test Coverage), but the implementation structure — cursor types, trait impls, dispatch types — must reflect the new iteration's design, not the previous one's.

The required semantic surface (what the tests exercise) is fixed:
- Single-stage concrete interpretation of HighLevel programs (SCF, recursion)
- Single-stage abstract interpretation of HighLevel and LowLevel programs
- Multi-stage concrete interpretation (source calls lowered)
- Multi-stage abstract interpretation (type and interval domains)

The implementation structure (how those semantics are achieved) is unconstrained. A new iteration may:
- Rename or restructure cursor types entirely
- Replace the `Mode` discriminant pattern with a different mechanism
- Unify concrete and abstract interpreters under a single type
- Change how dispatch works across stages
- Introduce new algebraic structures not present in previous iterations

Register the module in `example/toy-lang/src/lib.rs`: `pub mod interpreter<N>;`

---

## Phase 4: Run Tests and Fix

```bash
cargo nextest run -p toy-lang -E 'test(interpreter<N>)'
cargo nextest run -p kirin-interpreter-<N>
cargo nextest run -p kirin-scf
cargo nextest run -p kirin-function
```

If tests fail:
- Fix compilation errors first (most common after API changes)
- Fix logic failures by tracing through the interpreter execution
- If a failure reveals a design flaw, revise the relevant part of Phase 4 and re-run — do NOT loop all the way back to Phase 2 for a logic fix
- If a failure reveals a fundamental design problem (critique score would jump by ≥5), abort this iteration's implementation, log what was attempted, and loop to Phase 2 with updated critique

All tests must pass before committing.

---

## Phase 5: Log Findings

Append to `docs/log.md` (create if missing — this file must be gitignored):

```markdown
## Iteration <N> — <YYYY-MM-DD>

**Status: KEEP | DISCARD**
**Weighted score: <N> (previous: <prev>) — improved: YES | NO**
**Reason (if DISCARD):** <what the design change failed to address, or what new friction it introduced>

### Rubric Scorecard
| Dim | Score | Δ vs prev | Justification |
|-----|-------|-----------|--------------|
| R1 Completeness     | <1–5> | <+/-N>    | |
| R2 Lift/Project     | <1–5> | <+/-N>    | |
| R3 Dialect locality | <1–5> | <+/-N>    | |
| R4 Mode uniformity  | <1–5> | <+/-N>    | |
| R5 Boilerplate      | <1–5> | <+/-N>    | |
| R6 Type correctness | <1–5> | <+/-N>    | |
| R7 Elegance         | <1–5> | <+/-N>    | |
| R8 Extensibility    | <1–5> | <+/-N>    | |

**Overall grade:** <avg>/5
**Weighted convergence score:** <N> (threshold: ≤ 8)

### Findings addressed this iteration
- Finding #<K> [<severity>]: <what was changed and why>

### Design decisions
- **<change>**: <rationale — the "why", not the "what">

### Implementation notes
- <surprises, non-obvious constraints, Rust type system friction encountered>

### Test results
- <K> baseline tests: PASS
- Extensibility probe: PASS/FAIL/SKIPPED — <what was attempted, any friction>

### Open findings (carried to next iteration)
- Finding #<K> [High/Medium]: <brief — the suggestion from the critic>

### Convergence: YES / NO — <reason>
```

---

## Phase 6: Commit and Evaluate

Always commit — whether the iteration improves things or not. The commit creates a recoverable record. Then decide whether to keep it.

### 6a: Commit

```bash
git add crates/kirin-interpreter-<N>/
git add Cargo.toml Cargo.lock
git add crates/kirin-scf/src/interpreter<N>.rs   # and other dialect submodules
git add example/toy-lang/src/interpreter<N>.rs
git add example/toy-lang/src/lib.rs

git commit -m "feat(interpreter-<N>): <one-line summary of main change>"
```

Never stage or commit anything under `docs/`. Never use `git add .` or `git add -A` — always stage specific files.

### 6b: Score Comparison

Compare the weighted convergence score from this iteration against the previous iteration's score (read from `docs/log.md`):

- **Improved**: this iteration's score < previous iteration's score, OR this is the first iteration
- **No improvement**: this iteration's score ≥ previous iteration's score

### 6c: Discard if No Improvement

If the score did **not** improve, revert the commit immediately:

```bash
git revert HEAD --no-edit
```

This creates a revert commit that removes all implementation changes (`crates/kirin-interpreter-<N>/`, dialect submodules, and toy-lang additions) while keeping the history intact.

Then log the discarded iteration in `docs/log.md` (see Phase 6 template — use `status: DISCARD`). The log entry must include the reason the score did not improve (what the design change failed to address, or what new friction it introduced).

If the score **did** improve, log with `status: KEEP` and proceed.

### 6d: Consecutive Failure Check

If two consecutive iterations are discarded (neither improved the score), stop the loop:
- Log: "Stopped after N consecutive non-improving iterations — design may have reached a local optimum. Review open findings manually."
- Do not attempt a third iteration with the same finding set — a different approach is needed that requires human input.

---

## Phase 7: Critique the Committed Iteration

Run the critic subagent against the code that was just committed (or the previous KEEP iteration if this one was discarded). This is the critic run for iteration N — it produces the findings that Phase 2 of iteration N+1 will design against.

Same critic brief as Phase 1:
- All source files in `crates/kirin-interpreter-<N>/src/` (or `<prev>` if N was discarded)
- `example/toy-lang/src/interpreter<N>.rs` (or `<prev>` if N was discarded)
- The full rubric (copy verbatim)
- Updated `docs/log.md` (so the critic sees the discarded iteration notes too)

The critic produces the same structured report (Part 1 scorecard + Part 2 findings). Append the scores to the current iteration's log entry.

This is the **only** critic run per iteration. Do not re-run the critic mid-iteration or before committing.

---

## Phase 8: Loop or Stop

After Phase 7, check:

**Stop if** convergence criteria are met (weighted score ≤ 8, R1 ≥ 4, R6 ≥ 4, extensibility probe passed).

**Stop if** the iteration budget is exhausted.

**Stop if** two consecutive iterations were discarded (Phase 6d).

**Otherwise** increment N and loop to Phase 2 immediately — no user prompt needed.

---

## gitignore Check

At session start, verify `docs/log.md` is gitignored:

```bash
grep -q 'docs/log.md' .gitignore || echo 'docs/log.md' >> .gitignore
```

`docs/design/`, `docs/plans/`, and `docs/review/` are committed — do not gitignore them. Only `docs/log.md` and `docs/design_principles.md` should be gitignored (they are working notes, not checked-in artifacts).

```bash
grep -q 'docs/design_principles.md' .gitignore || echo 'docs/design_principles.md' >> .gitignore
```

---

## Subagent Notes

- Critic subagent: `dontAsk` mode, read-only, must return a structured report with severity tags
- Implementation subagents (if parallelizing crate vs. dialect work): use git worktrees, merge before commit
- Never use `bypassPermissions` — it exhausts session auth
