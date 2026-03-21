---
name: dialect-dev
description: Use when building a new Kirin dialect from scratch or adding a significant extension to an existing dialect. Triggers on requests to create a dialect, design dialect operations, or implement a full dialect stack (text format, IR types, parser, printer, interpreter, tests). Kirin-specific — not for general feature work.
effort: high
argument-hint: "[dialect name or domain]"
---

# Dialect Development

**Announce at start:** State which skill is being used so the user knows what process is driving behavior.

**Key principle:** Text format and semantics first, data structures second. The text format is the user-facing contract; the IR types exist to support it.

Fixed progression: Spec (Phase 1) → GATE → IR Types (Phase 2) → Parser+Printer (Phase 3) → Interpreter (Phase 4) → GATE → Complete

## When to Use

- Building a new dialect or significant extension to an existing one
- **Also when types already exist but lack parser/printer/interpreter** — Phase 1 still applies. Existing types encode implicit assumptions about format and semantics. Phase 1 means documenting what the types imply as a spec, not skipping it.

**Don't use for:** 1-2 operations added to an existing dialect (just add them), refactoring (load `refactor`), non-dialect features (load `feature-dev`).

## Target

The dialect to build: **$ARGUMENTS**

If not provided, ask what dialect to build and what domain it targets.

## Phase 1: Spec — Text Format, Semantics, Types

The most important phase. This is where edge cases are discovered cheaply — before they're baked into 4 layers of code.

### Step 1: Text Format

For each operation, write the concrete syntax. Use the spec template in `references/spec-template.md`.

```
# <operation name>
Format:  %result = <namespace>.<op> %operand1, %operand2 -> <result-type>
Operands: %operand1: <type>, %operand2: <type>
Results:  %result: <type>
Attrs:    terminator: no | pure: yes | speculatable: yes | constant: no
```

Write 2-3 **example programs** showing operations composed together. These become test cases later.

### Step 2: Operational Semantics

For each operation, define what it computes. Focus on **edge cases** — this is where the value is:

```
# <operation name>
Semantics: <mathematical definition>
Type rule: <input types> -> <output types>
Edge cases:
  - <what happens at boundary? overflow? NaN? empty input?>
  - <preconditions? what if violated?>
  - <interaction with other operations?>
```

**The edge cases section is mandatory.** Testing showed that without it, "obvious" operations hide real design decisions:
- Negation: what about integer minimum (i32::MIN overflows)?
- Min/Max: NaN propagation semantics?
- Clamp: precondition `lo <= hi` — what if violated?
- Measure: does it consume the qubit (linearity)?

If you can't identify edge cases, you don't understand the operation well enough to implement it.

### Step 3: Type System

If the dialect needs its own types: define the type lattice, `CompileTimeValue` impl, and `Placeholder::placeholder()`. If reusing an existing type, confirm compatibility and document it.

### Output

A spec document containing: text format for every operation, operational semantics with edge cases, type system, and 2-3 example programs. Save to the design directory (see AGENTS.md Project structure).

### GATE: Spec Review

Load the `triage-review` skill scoped to the spec. Suggest reviewers:
- **Formalism** — semantics well-defined? Operations compose? Type rules consistent?
- **Dialect Author** (with domain context) — text format natural for the domain?
- **Ergonomics/DX** — syntax understandable? Concept budget reasonable?

**Gate condition:** Spec approved. Semantics unambiguous. Edge cases resolved.

## Phase 2: IR Types

Translate the spec into Rust types. Refer to AGENTS.md Derive Infrastructure Conventions.

1. Dialect enum/struct with `#[derive(Dialect)]` and `#[kirin(type = T)]`
2. Operation variants matching the spec's operand/result structure exactly
3. Type lattice impls if needed
4. Derive annotations (`#[kirin(terminator)]`, `#[kirin(pure)]`, etc.) as the spec defines

**Verification:** `cargo check` passes.

## Phase 3: Parser + Printer

Parser and printer derive from the same format strings and are always tested together.

1. Add `#[derive(HasParser, PrettyPrint)]` to operations and dialect enum
2. Format strings must match the spec's text format exactly
3. Write **roundtrip tests** (parse → emit → print → compare) using the example programs from Phase 1

**Verification:** All spec examples roundtrip correctly. Place tests per AGENTS.md Test Conventions.

## Phase 4: Interpreter

Translate the operational semantics from Phase 1 into Rust.

1. `#[derive(Interpretable)]` for dispatch boilerplate
2. Manual `interpret` impls for actual computation logic
3. `CallSemantics` / `SSACFGRegion` if the dialect has callable or region-containing operations

**The implementation must match the spec semantics exactly** — including edge case behavior. If an implementation decision diverges, update the spec first.

**Verification:** Evaluate the example programs from Phase 1. Results match spec'd semantics. Test the edge cases explicitly.

### GATE: Integration Review

1. Load `test-coverage-review` — verify coverage, discover missed edge cases
2. Load `triage-review` — include Formalism, Code Quality, Dialect Author, Soundness Adversary

**Gate condition:** No P0/P1. Spec and implementation aligned.

## Phase 5: Complete

Load `verification-before-completion`, then `finishing-a-development-branch`.

## Iteration

Backtracking is expected. The spec is the source of truth — update it when you backtrack.

| Discovery | Go Back To |
|-----------|-----------|
| Interpreter reveals semantic ambiguity | Phase 1 Step 2 |
| Parser can't express the text format | Phase 1 Step 1 |
| Type lattice doesn't support dispatch | Phase 1 Step 3 |
| Roundtrip mismatch | Phase 3 (usually format string) |
| Review finds formalism issue | Phase 1 |

## Red Flags — STOP

- Writing IR types before text format is designed (Phase 2 before Phase 1)
- Implementing interpreter before spec review gate passes
- Spec and implementation diverging without updating spec
- Skipping the spec review gate
- Adding operations not in the spec without updating spec first
- Using `#[allow(...)]` to suppress derive errors instead of fixing types

## Rationalization Table

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Write IR types first | "I need to see the types to design the format" | Types written first constrain the format to what's easy to represent, not what's natural for the domain. Format is the user-facing contract — it leads. |
| Skip semantics | "The semantics are obvious from the names" | Testing proved this wrong: "obvious" operations like Neg, Min, Clamp all have non-obvious edge cases (i32::MIN overflow, NaN propagation, precondition violations). Ambiguity in semantics becomes 4 bugs (spec, IR, parser, interpreter). |
| Skip spec review | "The spec is simple, let's just implement" | Simple specs still have composability issues invisible from a single-operation view. |
| Implement all phases at once | "Faster to do it together" | Each phase validates the previous. A format bug in Phase 3 is cheap. The same bug found after Phase 4 requires fixing 3 layers. |
| Build on existing types without spec | "Types already exist, just add parser/printer" | Existing types encode implicit decisions. Phase 1 means making those decisions explicit as a spec — not skipping the spec. The types are a foundation, not a finished design. |
| Add operations during implementation | "I realized we need one more" | Add to spec first, review, then implement. Unreviewed operations accumulate semantic debt. |
| Suppress derive errors | "I'll fix the type definition later" | Derive errors signal misaligned types. Suppressing hides the problem until it's a runtime bug. |

## Integration

**Skills this orchestrator composes (load when needed):**

| Layer | Skill | When |
|-------|-------|------|
| 3 | `triage-review` | Spec review gate, integration review gate |
| 3 | `test-coverage-review` | Integration review gate |
| 2 | `brainstorming` | Pre-requisite exploration if domain unclear |
| 2 | `writing-plans` | Phase 2-4 if large enough to need a plan |
| 2 | `finishing-a-development-branch` | Phase 5 completion |
| 1 | `verification-before-completion` | Phase 5 checks |
| Domain | `ir-spec-writing` | Phase 1 Step 1 |
| Domain | `kirin-derive-macros` | Phase 2 if custom derive needed |
