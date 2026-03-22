# Graph & Parsing Refactor — Triage Review Plan

**Date:** 2026-03-21
**Scope:** Full triage of recent graph-IR-node, signature, and function-parsing changes
**Focus:** Readability, DX, abstraction opportunities, soundness

---

## Scope Summary

| Crate | Lines | Key Areas |
|-------|-------|-----------|
| kirin-ir | 7,587 | Graph nodes (DiGraph, UnGraph, Port), Signature, builders, SSA extensions |
| kirin-derive-toolkit | 7,176 | Field classification (FieldCategory, FieldData), template system, codegen |
| kirin-derive-ir | 950 | Dialect trait codegen, HasSignature generation, property traits |
| kirin-derive-chumsky | 6,898 | Parser codegen, format string DSL, field projections, EmitIR, PrettyPrint |
| kirin-derive-prettyless | 129 | RenderDispatch derive |
| kirin-chumsky | 6,277 | Parser runtime, AST nodes, function text parsing, traits, builtins |
| kirin-prettyless | 3,468 | Pretty printer, graph rendering, document builder, pipeline printing |
| kirin-lexer | 995 | Token definitions for graph syntax |
| kirin-function | 657 | Function dialect (Lexical/Lifted), Call, Return, Lambda, Bind |

**Total:** ~34,137 lines of source code under review

---

## Review Units

Crates grouped by coupling to enable focused per-unit reviews:

| Unit | Crates | Lines | Rationale |
|------|--------|-------|-----------|
| **U1: Core IR** | kirin-ir | 7,587 | Foundation — graph nodes, signature, builders, SSA. All reviewers needed. |
| **U2: Parser Runtime** | kirin-chumsky, kirin-lexer | 7,272 | Parser infrastructure + tokens. User-facing parsing APIs. |
| **U3: Derive Infra** | kirin-derive-toolkit, kirin-derive-ir | 8,126 | Shared field classification + IR trait codegen. Template system. |
| **U4: Parser/Printer Codegen** | kirin-derive-chumsky, kirin-derive-prettyless | 7,027 | Parser + printer code generation from derives. Format string DSL. |
| **U5: Output & Dialects** | kirin-prettyless, kirin-function | 4,125 | Printer runtime + function dialect. Dialect author experience. |

---

## Reviewer Roster

### Core Reviewers (all units)

| Role | Persona | Focus |
|------|---------|-------|
| **Formalism** | PL Theorist | Abstraction composability, trait boundary design, naming, type safety |
| **Code Quality** | Implementer (review mode) | Lint suppressions, duplication, Rust idioms, best practices |
| **Ergonomics/DX** | Physicist | API clarity, concept budget, lifetime complexity, user repetition |

### Domain Reviewers (selective)

| Role | Persona | Assigned Units | Domain Context |
|------|---------|----------------|----------------|
| **Dialect Author** | Dialect Author | U1, U2, U5 | PL / Lambda Calculus (kirin-function), Compiler IR Design (kirin-ir), Parser DX |
| **Soundness Adversary** | Soundness Adversary | U1, U2, U4 | Arena/ID safety, graph builder invariants, parser error paths |

### Per-Unit Assignment Matrix

| Unit | Formalism | Code Quality | Ergonomics | Dialect Author | Soundness |
|------|-----------|-------------|------------|----------------|-----------|
| U1: Core IR | X | X | X | X | X |
| U2: Parser Runtime | X | X | X | X | X |
| U3: Derive Infra | X | X | X | | |
| U4: Parser/Printer Codegen | X | X | X | | X |
| U5: Output & Dialects | X | X | X | X | |

**Total dispatches:** 22 reviewer-unit pairs (Step 1) + 22 cross-reviews (Step 2) + 5 per-unit aggregations (Step 3)

---

## Focus Areas Per Unit

### U1: Core IR (kirin-ir)

**Formalism:**
- Graph node type hierarchy (DiGraph vs UnGraph vs Block vs Region) — is the taxonomy principled?
- Port/SSAKind extension — does SSAKind::Port compose cleanly with existing SSA infrastructure?
- Signature<T, C> design — is the constraint parameter justified?
- HasSignature trait — does it compose with HasBlocks/HasRegions pattern?

**Code Quality:**
- Builder code duplication between DiGraphBuilder and UnGraphBuilder
- Error type design (PipelineError, SpecializeError, FinalizeError) — overlap?
- Lint suppressions in builder code
- Missing Debug/Display on new public types

**Ergonomics:**
- Concept budget for graph operations (DiGraph + UnGraph + Port + PortParent + SSAKind extension)
- Builder API ergonomics — how many steps to construct a graph?
- Signature API — intuitive for dialect authors?

**Dialect Author:**
- Workflow: "add a graph-containing operation to my dialect" step by step
- Domain alignment: do graph abstractions map to real compiler/quantum domains?
- Incremental development: can I add graph support after initial dialect implementation?

**Soundness:**
- Graph builder invariants (node/edge validation, port indexing)
- debug_assert vs assert in graph builders (release-mode safety)
- Port SSA value lifecycle — stale port references after graph mutation?
- StatementParent generalization — cross-container parent assignment?

**Files:**
- `src/node/digraph.rs`, `src/node/ungraph.rs`, `src/node/port.rs`, `src/node/ssa.rs`
- `src/builder/digraph.rs`, `src/builder/ungraph.rs`, `src/builder/stage_info.rs`
- `src/signature/` (all files)
- `src/language.rs` (trait extensions: HasDigraphs, HasUngraphs, IsEdge)

### U2: Parser Runtime (kirin-chumsky + kirin-lexer)

**Formalism:**
- AST node hierarchy (blocks, graphs, values, symbols) — clean taxonomy?
- Parser trait design (HasParser, HasDialectParser, ParseEmit) — composability?
- Function text parsing architecture — separation of concerns?

**Code Quality:**
- `function_text/parse_text.rs` at 978 lines — module decomposition needed?
- `tests.rs` at 1,479 lines — test organization
- Duplication between block and graph parsers
- Lexer token variants — completeness, organization

**Ergonomics:**
- ParseStatementText / ParsePipelineText API — intuitive for users?
- Error reporting quality from function_text parser
- Concept budget: how many traits/types to parse a single statement?

**Dialect Author:**
- "Parse a custom operation" workflow — how much boilerplate?
- Built-in type extension: adding a new primitive type
- Error messages when parser derive generates incorrect code

**Soundness:**
- Parser error recovery — can malformed input cause panics?
- AST-to-IR emission (EmitIR) — are builder calls validated?
- Graph parser port/capture handling — can invalid port references parse successfully?

**Files:**
- `kirin-chumsky/src/ast/` (all), `src/traits/` (all), `src/parsers/` (all)
- `kirin-chumsky/src/function_text/` (all), `src/builtins/` (all)
- `kirin-lexer/src/lib.rs`

### U3: Derive Infrastructure (kirin-derive-toolkit + kirin-derive-ir)

**Formalism:**
- FieldCategory enum — is it exhaustive? Extensible?
- Template system (TraitImplTemplate, MethodPattern) — composability of codegen patterns
- Layout trait design — right abstraction for derive-specific attributes?

**Code Quality:**
- `builder_template/helpers.rs` at 588 lines — decomposition
- `ir/fields/info.rs` at 385 lines — complexity
- Duplication in field_iter_set.rs (312 lines of similar patterns)
- `misc.rs` at 310 lines — grab-bag module

**Ergonomics:**
- Template API for derive authors — concept budget
- Adding a new field category — how many files change?
- Error messages from darling when attributes are misconfigured

**Files:**
- `kirin-derive-toolkit/src/ir/` (all), `src/template/` (all)
- `kirin-derive-toolkit/src/codegen/`, `src/context/`, `src/misc.rs`
- `kirin-derive-ir/src/generate.rs`, `src/has_signature.rs`

### U4: Parser/Printer Codegen (kirin-derive-chumsky + kirin-derive-prettyless)

**Formalism:**
- Format string DSL design — formal grammar? Ambiguity?
- Field projection system (`:ports`, `:captures`, `:body`, `:signature`) — principled taxonomy?
- AST/parser/EmitIR codegen split — clean separation?

**Code Quality:**
- `codegen/parser/chain.rs` at 615 lines — decomposition
- `codegen/pretty_print/statement.rs` at 518 lines — decomposition
- `field_kind.rs` at 509 lines — complexity
- `validation.rs` at 676 lines — can validation rules be declarative?

**Ergonomics:**
- Format string syntax — intuitive for dialect authors?
- Error messages when format string is invalid
- PrettyPrint derive — concept budget for printer customization

**Soundness:**
- Format string validation completeness — can invalid formats pass validation?
- EmitIR codegen correctness — do generated builder calls match format parse order?
- Parser/printer symmetry — can codegen guarantee roundtrip correctness?

**Files:**
- `kirin-derive-chumsky/src/codegen/` (all), `src/field_kind.rs`, `src/format.rs`
- `kirin-derive-chumsky/src/validation.rs`, `src/visitor.rs`, `src/input.rs`
- `kirin-derive-prettyless/src/generate.rs`

### U5: Output & Dialects (kirin-prettyless + kirin-function)

**Formalism:**
- RenderDispatch vs PrettyPrint trait hierarchy — clean separation?
- Graph rendering algorithm — principled ordering?
- Function dialect: Lexical vs Lifted distinction — formal semantics?

**Code Quality:**
- `ir_render.rs` at 604 lines — decomposition
- `traits.rs` at 263 lines — trait count and responsibilities
- Function dialect interpret_impl at 301 lines — complexity

**Ergonomics:**
- PrettyPrint API — how much work to print a custom type?
- Pipeline printing configuration — intuitive?
- Function dialect composability — combining with other dialects

**Dialect Author:**
- "Add graph printing to my dialect" workflow
- Custom render formatting beyond defaults
- Function dialect as a reference implementation — clear enough to learn from?

**Files:**
- `kirin-prettyless/src/document/` (all), `src/traits.rs`, `src/pipeline.rs`, `src/impls.rs`
- `kirin-function/src/` (all files)

---

## Design Context Sections (from AGENTS.md)

Include these sections verbatim in all reviewer prompts to prevent false positives:

1. **Derive Infrastructure Conventions** — darling re-export, helper attributes, #[wraps] semantics, auto-placeholder
2. **IR Design Conventions** — Block vs Region, BlockInfo::terminator caching
3. **Chumsky Parser Conventions** — single lifetime, ParseEmit, ParseDispatch, #[wraps] with Region/Block
4. **Interpreter Conventions** — trait decomposition, 'ir lifetime, L on method
5. **Test Conventions** — where tests go

---

## Execution Plan

**Step 1:** Dispatch all 22 reviewer-unit pairs in parallel (maximize throughput)
**Step 2:** After Step 1 completes per-unit, dispatch cross-reviews in parallel
**Step 3:** Formalism reviewer aggregates per-unit final reports
**Step 4:** Main lead aggregates full workspace report

**Output directory:** `docs/review/2026-03-21-graph-parsing-refactor/`
