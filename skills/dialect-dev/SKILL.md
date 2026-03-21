---
name: dialect-dev
description: Use when building a new Kirin dialect from scratch or adding a significant extension to an existing dialect. Triggers on requests to create a dialect, design dialect operations, or implement a full dialect stack (text format, IR types, parser, printer, interpreter, tests). Kirin-specific — not for general feature work.
effort: high
argument-hint: "[dialect name or domain]"
---

# Dialect Development

## Overview

Kirin-specific orchestrator for building a new dialect from scratch. Unlike general feature development, dialect development follows a fixed progression that starts with **text format design and operational semantics** before any code is written. Each phase produces independently testable output.

**Announce at start:** "I'm using the dialect-dev skill to orchestrate this dialect development."

**Key principle:** Text format and semantics first, data structures second. The text format is the user-facing contract; the IR types exist to support it.

## When to Use

- Building a new dialect (e.g., a quantum gate dialect, a memory management dialect)
- Adding a significant extension to an existing dialect (new operation categories)

**Don't use for:**
- Adding 1-2 operations to an existing dialect (just add them directly)
- Refactoring an existing dialect (load the `refactor` skill)
- General feature development not dialect-specific (load the `feature-dev` skill)

## Target

The dialect to build: **$ARGUMENTS**

If no target was provided, ask the user what dialect they want to build.

## Pre-requisites

Before starting, the user should be able to answer:
1. What domain does this dialect target?
2. What are the key operations? (even rough names help)
3. What type system does the dialect need? (single type? type lattice? generic over types?)

If the user can't answer these, load the `brainstorming` skill first to explore the design space.

## Phase 1: Text Format & Semantics Design

The most important phase. Everything else derives from this.

### Step 1: Text Format Design

Load the `ir-spec-writing` skill. Design the text syntax for every operation in the dialect.

For each operation, define:
- **Text format**: How it appears in `.kirin` text (the `#[chumsky(format = "...")]` string)
- **Operands**: SSA values, block arguments, regions, successors
- **Results**: What the operation produces
- **Attributes**: Compile-time properties (terminator, pure, speculatable, constant)

Use existing dialects as reference patterns. Refer to AGENTS.md Chumsky Parser Conventions for format string syntax.

### Step 2: Operational Semantics Design

For each operation, define **what it means** — not how it's implemented, but what it computes:

- **Denotational**: What mathematical function does this operation represent?
- **Operational**: What steps does evaluation take? What are the observable effects?
- **Type rules**: What types are the inputs/outputs? What constraints must hold?

For example, for a quantum gate dialect:
```
# X gate (Pauli-X)
Semantics: Applies the Pauli-X matrix [[0,1],[1,0]] to a single qubit.
Type rule: qubit -> qubit
Text: %result = quantum.x %input -> qubit
Terminator: no
Pure: yes (no side effects on classical state)
```

**Document these in the spec.** The semantics drive the interpreter implementation later. Getting them wrong here means fixing them in 4 places (spec, IR, parser, interpreter).

### Step 3: Type System Design

If the dialect needs its own type system (not just reusing an existing one):

- Define the type lattice (what types exist, how they relate)
- Determine if you need `TypeLattice` (for subtype dispatch) or just `CompileTimeValue`
- Define `Placeholder::placeholder()` — what's the "unknown type" value?

**Output:** Spec document with text format examples, operational semantics for every operation, and type system definition.

### GATE: Spec Review

Load the `triage-review` skill scoped to the spec document and any related existing code.

**Required reviewers:**
- **Formalism** (PL Theorist) — are the semantics well-defined? Do operations compose? Are type rules consistent?
- **Dialect Author** — inject the domain background for this dialect (see AGENTS.md Dialect Domain Context). Does the text format feel natural for the domain?
- **Ergonomics/DX** (Physicist) — will users understand the syntax? Is the concept budget reasonable?

**Gate condition:** Spec is approved. Semantics are unambiguous. Text format is clear.

## Phase 2: IR Types

Implement the Rust types that represent the dialect.

1. **Dialect enum or struct** — `#[derive(Dialect)]` with `#[kirin(type = T)]`
2. **Operation variants** — one per operation from the spec, with correct field types (`SSAValue`, `ResultValue`, `Block`, `Region`, `Successor`)
3. **Type lattice** (if needed) — implement `CompileTimeValue`, `Placeholder`, and lattice traits
4. **Derive annotations** — `#[kirin(terminator)]`, `#[kirin(pure)]`, `#[kirin(constant)]` as defined in the spec

Refer to AGENTS.md Derive Infrastructure Conventions for attribute patterns.

**Verification:** `cargo check` passes. Derive expansion generates correct trait impls.

## Phase 3: Parser

Implement parsing from text format to IR.

1. **Format strings** — `#[chumsky(format = "...")]` on each operation, matching the spec's text format exactly
2. **`#[derive(HasParser)]`** — generates parser from format strings
3. **Type parsing** (if custom types) — implement `HasParser` for the type enum, or use `#[chumsky(format = ...)]` on type variants

**Verification:** Parse the example programs from the spec. Every example should parse without errors.

## Phase 4: Printer

Implement pretty-printing from IR back to text.

1. **`#[derive(PrettyPrint)]`** — generates printer from format strings (same source as parser)
2. **Roundtrip test** — parse → emit → print → compare against original text

**Verification:** Roundtrip tests pass for every example from the spec. Place roundtrip tests following AGENTS.md Test Conventions.

## Phase 5: Interpreter

Implement the operational semantics from Phase 1 as Rust code.

1. **`#[derive(Interpretable)]`** — generates dispatch boilerplate
2. **Manual `interpret` impls** — implement the actual semantics for each operation
3. **`CallSemantics`** (if the dialect has callable operations) — implement function-call evaluation
4. **`SSACFGRegion`** (if the dialect has region-containing operations) — mark for standard CFG evaluation

The interpreter implementation should match the operational semantics from the spec **exactly**. If an implementation decision diverges from the spec, update the spec first.

**Verification:** Evaluate the example programs from the spec. Check that results match the expected semantics.

### GATE: Integration Review

After all phases are complete:

1. Load the `test-coverage-review` skill to verify coverage and discover edge cases
2. Load the `triage-review` skill for multi-perspective review — include:
   - **Formalism** — does the implementation match the spec semantics?
   - **Code Quality** — are there unnecessary `#[allow]` annotations or duplication?
   - **Dialect Author** (with domain context) — does it feel right for the domain?
   - **Soundness Adversary** — can invalid IR be constructed through the public API?

**Gate condition:** No P0/P1 issues. Spec and implementation are aligned.

## Phase 6: Complete

1. Load the `verification-before-completion` skill
2. Load the `finishing-a-development-branch` skill

## Iteration

Dialect development is inherently iterative. Common backtrack patterns:

| Discovery | Go Back To |
|-----------|-----------|
| Interpreter reveals ambiguity in semantics | Phase 1 Step 2 (operational semantics) |
| Parser can't express the text format | Phase 1 Step 1 (text format design) |
| Type lattice doesn't support needed dispatch | Phase 1 Step 3 (type system design) |
| Roundtrip fails because print doesn't match parse | Phase 3 or Phase 4 (usually a format string issue) |
| Triage-review finds formalism issue | Phase 1 (spec) |

Backtracking is expected and healthy. Update the spec document when you backtrack — the spec is the source of truth.

## Red Flags — STOP

- Writing IR types before the text format is designed (Phase 2 before Phase 1)
- Implementing interpreter before spec semantics are reviewed (Phase 5 before Gate)
- Spec and implementation diverging without updating the spec
- Skipping the spec review gate
- Adding operations not in the spec without updating the spec first
- Using `#[allow(...)]` to suppress derive errors instead of fixing the type definitions

## Rationalization Table

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Write IR types first | "I need to see the types to design the text format" | The text format is the user-facing contract. Types serve the format, not the reverse. Types written first constrain the format to what's easy to represent, not what's natural for the domain. |
| Skip operational semantics | "The semantics are obvious from the operation names" | 'Obvious' semantics have edge cases: what does division by zero do? What happens when a loop body yields early? Ambiguity here becomes 4 bugs later (spec, IR, parser, interpreter). |
| Skip spec review gate | "The spec is simple, let's just implement" | Simple specs still have composability issues. The Formalism reviewer catches interactions you don't see when looking at operations in isolation. |
| Implement all phases at once | "Faster to write everything together" | Phases are ordered so each validates the previous. A parser bug found in Phase 3 is cheap. The same bug found after writing the interpreter in Phase 5 requires fixing 3 layers. |
| Add operations during implementation | "I realized we need one more operation" | Add it to the spec first, get it reviewed, then implement. Unreviewed operations accumulate semantic debt. |
| Suppress derive errors with `#[allow]` | "I'll fix the type definition later" | Derive errors signal misaligned types. Suppressing them hides the misalignment until it surfaces as a runtime bug in the interpreter. Fix the type definition now. |

## Integration

**Skills this orchestrator composes (load when needed):**

| Layer | Skill | When |
|-------|-------|------|
| 3 | `triage-review` | Spec review gate, integration review gate |
| 3 | `test-coverage-review` | Integration review gate |
| 2 | `brainstorming` | Pre-requisite exploration if domain is unclear |
| 2 | `writing-plans` | Phase 2-5 if the dialect is large enough to need a plan |
| 2 | `finishing-a-development-branch` | Phase 6 completion |
| 1 | `verification-before-completion` | Phase 6 checks |
| Domain | `ir-spec-writing` | Phase 1 Step 1 text format design |
| Domain | `kirin-derive-macros` | Phase 2 if custom derive behavior is needed |
