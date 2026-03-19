# Kirin Workspace Review — 2026-03-18

**Scope:** Full workspace — 21 crates (~46K lines)
**Reviewers:** PL Theorist (formalism), Implementer (code quality), Physicist (ergonomics/DX), Compiler Engineer (cross-cutting)
**Per-crate reports:** `docs/review/2026-03-18/<crate>/final-report.md`
**Status:** Phase 1 (core crates) + Phase 2 (tier 2 groups) complete. User walkthrough complete.

---

## Executive Summary

The codebase is architecturally sound with strong formalism foundations. The trait decomposition, MLIR alignment, and derive-based DX are well-executed. The review found **0 P0** and **8 P1** issues across 6 crates, with the most impactful findings clustering around three themes: **dependency hygiene**, **user-facing convenience APIs**, and **code duplication**.

| Severity | Accepted | Won't Fix | Total |
|----------|----------|-----------|-------|
| P1 | 8 | 0 | 8 |
| P2 | 16 | 0 | 16 |
| P3 | 16 | 4 | 20 |

---

## P1 Findings (All Accepted)

### P1-1. Pipeline lacks convenience methods for stage/function resolution
**Crate:** kirin-ir | **File:** `example/toy-lang/src/main.rs:76-110`
Stage lookup requires 13 lines of iterator chaining. Function resolution requires 6+ nested lookups.
**Action:** Add `Pipeline::stage_by_name()` and `Pipeline::resolve_function()`.

### P1-2. `kirin-chumsky` hard-depends on `kirin-prettyless`
**Crate:** kirin-chumsky | **File:** `crates/kirin-chumsky/Cargo.toml:10`
Parser-only users must compile the entire printer stack.
**Action:** Feature-gate behind `pretty` (default-enabled).

### P1-3. `kirin-prettyless` defaults `bat` feature on
**Crate:** kirin-prettyless | **File:** `crates/kirin-prettyless/Cargo.toml:16`
Forces `syntect` + transitive deps on all downstream library crates.
**Action:** Change to `default = ["serde"]`. Update binaries to explicitly enable `bat`.

### P1-4. `kirin-derive-chumsky` has direct `darling` dependency
**Crate:** kirin-derive-chumsky | **Files:** `Cargo.toml:13`, `src/attrs.rs:3`
Violates workspace re-export convention.
**Action:** Switch to `kirin_derive_toolkit::prelude::darling` imports.

### P1-5. Unused `_has_dialect_parser_bounds` parameter
**Crate:** kirin-derive-chumsky | **File:** `src/codegen/ast/trait_impls.rs:168-177`
Dead parameter forcing `#[allow(clippy::too_many_arguments)]`.
**Action:** Remove parameter and allow.

### P1-6. Custom type lattices require ~60-115 lines of manual boilerplate
**Crate:** kirin-chumsky | **File:** `kirin-arith/src/types/arith_type.rs`
6 manual trait impls for a simple keyword-to-enum mapping.
**Action:** Support `chumsky(format = ...)` when deriving type enums (user preference over simple keyword approach, to support generic types).

### P1-7. SSA name resolution duplicated 7 times in kirin-prettyless
**Crate:** kirin-prettyless | **Files:** `src/impls.rs`, `src/document/ir_render.rs`
Same lookup-name-or-fallback-to-ID pattern copy-pasted.
**Action:** Extract `ssa_name()` helper on `Document` (user preferred name over `resolve_ssa_name`).

### P1-8. Inner dialect enums lack derive support for Interpretable
**Crate:** kirin-interpreter | **Files:** `crates/kirin-function/src/interpret_impl.rs:81-116, 267-301`
~69 lines of manual delegation that `#[derive(Interpretable)]` should handle.
**Action:** Extend derive to support enum-level `#[wraps]` + `#[callable]`.

---

## P2 Findings (All Accepted)

### Cross-Crate Themes

**A. Missing `#[must_use]` annotations (all 6 crates)**
Zero `#[must_use]` across the entire workspace. Key candidates: `Pipeline` constructors, `Continuation` enum, `AnalysisResult` accessors, `EmitContext::new()`, `RenderBuilder::to_string()`, `TemplateBuilder::build()`.

**B. `bon` dependency overhead**
- `kirin-prettyless`: **dead dependency** — not imported or used anywhere. Immediate removal.
- `kirin-ir`: Pulls duplicate darling 0.20 for 4 builder methods. Replace with hand-written builders.

**C. DiGraph/UnGraph code duplication**
- `kirin-ir/src/builder/digraph.rs` + `ungraph.rs`: ~100 lines identical
- `kirin-chumsky/src/ast/graphs.rs`: ~60 lines identical

### Per-Crate P2 Findings

| # | Crate | Finding | File | User Notes |
|---|-------|---------|------|------------|
| D | kirin-ir | Remove `Default` from `TypeLattice`, investigate callers | `lattice.rs:59` | User: remove Default, report callers |
| E | kirin-ir | Add `#[diagnostic::on_unimplemented]` to `HasStageInfo`, `Dialect`, `StageMeta` | `stage/meta.rs:28,68`, `language.rs:103` | |
| F | kirin-interpreter | Improve `active_stage_info` panic with `type_name::<L>()` | `stage_access.rs:36` | |
| G | kirin-interpreter | `bind_block_args`: replace `Vec` with `SmallVec<[SSAValue; 4]>` | `block_eval.rs:44-48` | |
| H | kirin-interpreter | Simplify Interpretable trait bounds (explore trait simplification, NOT macros) | Various dialect crates | User: avoid macros, simplify the trait itself |
| I | kirin-interpreter | Convenience API for function resolution | `kirin-function/src/interpret_impl.rs:149-234` | User: needs more investigation first |
| J | kirin-chumsky | `parse_and_emit` conflates `EmitError` with `ParseError` (zero span) | `traits/parse_emit.rs:39-49` | |
| K | kirin-chumsky | Replace glob re-exports with explicit re-exports | `lib.rs:60-63` | |
| L | kirin-derive-chumsky | `is_missing_type_error` string matching — use Option instead | `input.rs:37-39` | |
| M | kirin-derive-chumsky | Format string DSL standalone documentation | — | |
| N | kirin-prettyless | `RenderDispatch` return `RenderError` instead of `fmt::Error` | `pipeline.rs:46-51` | |
| O | kirin-prettyless | Introduce `PrettyPrintViaDisplay` marker trait | `impls.rs` | |
| P | kirin-prettyless | Unify f32/f64 `PrettyPrint` with macro | `impls.rs:236-270` | |
| Q | kirin-ir | PhantomData: use `#[non_exhaustive]` approach | Various dialect crates | User: keep PhantomData explicit, use non_exhaustive |
| R | kirin-derive-toolkit | Duplicate `#[stage(name)]` detection | `stage_info.rs` | |
| S | kirin-derive-toolkit | "Which derives do I need?" cheat sheet | Documentation | |

---

## P3 Findings

| # | Finding | Decision | User Notes |
|---|---------|----------|------------|
| 1 | `from_stage_name` blanket impl ignores arg — add doc comment | Accepted | |
| 2 | `from_stage_name` returns `String` error — use structured `StageNameError` | Accepted | |
| 3 | `Signature` fields are `pub` — make private with accessors | Accepted | |
| 4 | `module_inception`: signature/signature.rs — rename | Accepted | |
| 5 | Builder APIs use panics where Results could work | **Won't Fix** | Programmer errors, panics correct |
| 6 | Long derive lists (8-10 per type) — `KirinDialect` shorthand | **Won't Fix** | f64 fields break Eq/Hash, user may need manual impls |
| 7 | HRTB supertrait pressure on `Dialect` (19 `for<'a>`) | Accepted | Monitor as project scales |
| 8 | `AnalysisResult` informal partial order — formalize with lattice traits | Accepted | |
| 9 | Manual Debug/Clone on `AnalysisResult` — replace with derives | Accepted | |
| 10 | `Continuation` could derive Clone | Accepted | |
| 11 | `Staged` lifetime docs — add doc comment | Accepted | |
| 12 | `sprint()` panic message — include actual error | Accepted | |
| 13 | `RenderBuilder::to_string` naming — rename to `into_string()` | Accepted | User preference: `into_string()` |
| 14 | `EmitContext::resolve_ssa` forward ref Result(0) convention | **Won't Fix** | ResolutionInfo is dedicated for this |
| 15 | Three error types without unified hierarchy | Accepted | |
| 16 | `__`-prefix convention undocumented — add docs + optional `#[kirin(hidden)]` | Accepted | |
| 17 | Monomorphization pressure from `interpret<L>` | Accepted | Monitor only |
| 18 | String clones in `register_ssa` — accept `impl Into<String>` | Accepted | |
| 19 | `pretty_print_name`/`pretty_print_type` defaults — document contract | Accepted | |
| 20 | Two-pass pipeline parsing doubles cost | Accepted | Note for future optimization |

---

## Cross-Cutting Themes

### 1. Dependency hygiene (3 reviewers, 4 crates)
`bon` dead in prettyless, `bon` pulling duplicate darling in kirin-ir, `bat` defaulting to on, chumsky hard-depending on prettyless, direct darling import in derive-chumsky. Collectively these add significant compile time.

### 2. Missing convenience APIs (Physicist + Implementer, 3 crates)
Pipeline stage/function resolution, callee resolution, and type lattice definition all require excessive ceremony. The framework excels at the derive-heavy path but drops off sharply when users need to interact with the runtime API.

### 3. Code duplication (Implementer, 3 crates)
DiGraph/UnGraph duplication in both kirin-ir (builders) and kirin-chumsky (emit), plus SSA name resolution in prettyless. ~250+ lines of near-identical code to extract.

### 4. Error diagnostics gap (Compiler Engineer + Physicist, 3 crates)
`AsBuildStage` sets a high bar for diagnostic quality, but key traits lack `#[diagnostic::on_unimplemented]`. Panic messages miss type names. Parser derive errors surface from deep in generated code.

---

## Architectural Strengths

1. **Principled trait encoding** — L-on-method technique, witness methods for GAT projection, coinductive resolution. (PL Theorist)
2. **Strong MLIR alignment** — Block/Region/Statement hierarchy, namespace-based dispatch. (PL Theorist)
3. **Clean trait decomposition** — ValueStore / StageAccess / BlockEvaluator / Interpreter. (PL Theorist, Implementer)
4. **Format string DSL** — Single source of truth for parse and print. (All reviewers)
5. **Derive-heavy DX** — Zero lifetime annotations for dialect authors. (Physicist)
6. **Correct abstract interpretation** — Cousot-style widening/narrowing. (PL Theorist)
7. **Low clippy suppression count** — 16 total, all justified except 1. (Implementer)

---

## Suggested Follow-Up Actions (Priority Order)

### Quick Wins (< 30 min each)
1. Remove `bon` from `kirin-prettyless/Cargo.toml`
2. Remove unused `_has_dialect_parser_bounds` parameter + `#[allow]`
3. Move `bat` out of default features
4. Add `#[must_use]` annotations across workspace
5. Improve `active_stage_info` panic with `type_name::<L>()`
6. Unify f32/f64 `PrettyPrint` with macro
7. Improve `sprint()` panic messages to include error
8. Replace manual Debug/Clone on `AnalysisResult` with derives
9. Add Clone derive to `Continuation`
10. Rename `RenderBuilder::to_string()` to `into_string()`

### Moderate Effort (1-3 hours each)
11. Add `Pipeline::stage_by_name()` and `Pipeline::resolve_function()`
12. Extract `ssa_name()` helper in kirin-prettyless
13. Extract shared DiGraph/UnGraph port allocation helpers
14. Feature-gate `kirin-prettyless` in `kirin-chumsky`
15. Resolve direct darling dependency in kirin-derive-chumsky
16. Add `#[diagnostic::on_unimplemented]` to key traits
17. Replace glob re-exports with explicit re-exports in kirin-chumsky
18. Remove `Default` from `TypeLattice` (investigate callers first)
19. Add duplicate `#[stage(name)]` detection
20. Replace `Vec` with `SmallVec` in `bind_block_args`
21. Change `register_ssa` to accept `impl Into<String>`
22. Replace `bon` in kirin-ir with hand-written builders
23. Structured `StageNameError` for `from_stage_name`
24. Make `Signature` fields private, add accessors
25. Rename signature/signature.rs (module_inception)

### Design Work (half-day+)
26. Support `chumsky(format = ...)` for type enum derives (supports generic types)
27. Extend `#[derive(Interpretable)]` for enum-level `#[wraps]` on inner dialect enums
28. Introduce `PrettyPrintViaDisplay` marker trait
29. Change `RenderDispatch` return type to `RenderError`
30. Simplify Interpretable trait bounds (explore trait simplification, not macros)
31. Investigate function resolution convenience API
32. `parse_and_emit` error type: preserve parse-vs-emit distinction
33. Unify error type hierarchy in kirin-chumsky
34. Formalize `AnalysisResult` with lattice traits
35. `is_missing_type_error`: use Option instead of string matching

### Documentation
36. Format string DSL reference documentation
37. "Which derives do I need?" cheat sheet
38. Document `__`-prefix variant convention + optional `#[kirin(hidden)]`
39. Document `Staged` lifetime semantics
40. Document `from_stage_name` single-dialect base case
41. Document `pretty_print_name`/`pretty_print_type` override contract
42. PhantomData: document `#[non_exhaustive]` approach

---

---

# Phase 2: Tier 2 Crate Groups

**Groups:** Dialects (7 crates), Derive-small (3 crates), Testing (3 crates), Utilities (2 crates)
**Per-group reports:** `docs/review/2026-03-18/{dialects,derive-small,testing,utilities}/final-report.md`

## Phase 2 P1 Findings (All Accepted)

### P1-9. Dialect crates depend on top-level `kirin`, pulling full parser+printer unconditionally
**Group:** Dialects | **File:** All 7 dialect `Cargo.toml` files
All dialects have `kirin.workspace = true` which unconditionally pulls `kirin-chumsky` + `kirin-prettyless`. Interpreter-only users compile the entire parser and printer stack.
**Action:** Dialect crates depend on `kirin-ir` directly; feature-gate parser/pretty derives.

### P1-10. Inner dialect enums (StructuredControlFlow) also lack derive support
**Group:** Dialects | **File:** `kirin-scf/src/interpret_impl.rs:229-247`
Extends Phase 1 P1-8 scope. `StructuredControlFlow` in kirin-scf is a third inner enum with manual delegation alongside `Lexical` and `Lifted`.

## Phase 2 P2 Findings (Accepted unless noted)

| # | Group | Finding | File | Status |
|---|-------|---------|------|--------|
| P2-U | Dialects | Binary-op interpret boilerplate ~18 times across arith/bitwise/cmp | Multiple `interpret_impl.rs` | Accepted |
| P2-V | Utilities | Interval Div/Rem return top() unconditionally | `interval/lattice_impl.rs:82-96` | Accepted |
| P2-W | Utilities | Interval fields pub, bypass new() normalization | `interval/domain.rs:5-8` | Accepted |
| P2-X | Derive-small | `#[callable]` undocumented at derive level | `kirin-derive-interpreter/src/lib.rs:19-35` | Accepted |
| P2-Y | Testing | `dump_function` hardcoded to CompositeLanguage | `kirin-test-utils/src/lib.rs:20-28` | Accepted |
| P2-Z | Dialects | FunctionBody/Lambda identical SSACFGRegion+Interpretable (~35 lines) | `kirin-function/src/interpret_impl.rs:9-79` | Accepted |
| P2-AA | Dialects | CompareValue returns Self instead of boolean domain | `kirin-cmp/src/interpret_impl.rs:10-17` | Accepted |
| — | Testing | test-languages unconditional parser/pretty features | `kirin-test-languages/Cargo.toml:8` | **Won't Fix** |

## Phase 2 P3 Findings

| # | Finding | Decision | Notes |
|---|---------|----------|-------|
| 1 | `_ => unreachable!()` should match `__Phantom` explicitly | Accepted | 4 dialect crates |
| 2 | `ForLoopValue::loop_condition` None semantics undocumented | Accepted | |
| 3 | Lexer `lex()` error lacks source context | Accepted | |
| 4 | Lattice test suite only exercises UnitType | Accepted | Add SimpleType |
| 5 | CheckedDiv float naming misleading | **Won't Fix** | |
| 6 | Token::ToTokens mechanical match arms | Accepted | Use macro (user preference) |
| 7 | `bottom_interval()` remove from public API | Accepted | Use HasBottom::bottom() only |
| 8 | CallSemantics Result homogeneity implicit | **Won't Fix** | |
| 9 | Bound::negate i64::MIN → PosInf documentation | Accepted | |
| 10 | LocalFieldIterConfig duplicates toolkit type (needs Copy) | Accepted | ~60 lines removable |
| 11 | Interpreter where-clause repetition | Accepted | Tracked by Phase 1 P2-N |
| 12 | StringLit allocates on every lex | Accepted | Note for future |

---

## Full Summary Counts (Phase 1 + Phase 2)

| Metric | Phase 1 | Phase 2 | Total |
|--------|---------|---------|-------|
| P0 | 0 | 0 | **0** |
| P1 accepted | 8 | 2 | **10** |
| P2 accepted | 16 | 7 | **23** |
| P3 accepted | 16 | 10 | **26** |
| Won't fix | 4 | 3 | **7** |
| **Total accepted** | **40** | **19** | **59** |
| Filtered | ~30 | ~15 | **~45** |

## Complete Follow-Up Actions (Priority Order)

### Quick Wins (< 30 min each)
1. Remove `bon` from `kirin-prettyless/Cargo.toml` (dead dep)
2. Remove unused `_has_dialect_parser_bounds` + `#[allow]` in derive-chumsky
3. Move `bat` out of default features in kirin-prettyless
4. Add `#[must_use]` annotations across workspace
5. Improve `active_stage_info` panic with `type_name::<L>()`
6. Unify f32/f64 `PrettyPrint` with macro
7. Improve `sprint()` panic messages to include error
8. Replace manual Debug/Clone on `AnalysisResult` with derives
9. Add Clone derive to `Continuation`
10. Rename `RenderBuilder::to_string()` to `into_string()`
11. Replace `_ => unreachable!()` with `Self::__Phantom(..) => unreachable!()`
12. Document `ForLoopValue::loop_condition` None semantics
13. Add source text to `lex()` error messages
14. Make `bottom_interval()` `pub(crate)`, use `HasBottom::bottom()`
15. Document `Bound::negate` i64::MIN behavior
16. Add Copy to FieldIterConfig/BoolPropertyConfig, remove local wrappers
17. Use macro for Token::ToTokens match arms
18. Make `dump_function` generic over language type

### Moderate Effort (1-3 hours each)
19. Add `Pipeline::stage_by_name()` and `Pipeline::resolve_function()`
20. Extract `ssa_name()` helper in kirin-prettyless
21. Extract shared DiGraph/UnGraph port allocation helpers
22. Feature-gate `kirin-prettyless` in `kirin-chumsky`
23. Resolve direct darling dependency in kirin-derive-chumsky
24. Add `#[diagnostic::on_unimplemented]` to key traits
25. Replace glob re-exports with explicit re-exports in kirin-chumsky
26. Remove `Default` from `TypeLattice` (investigate callers)
27. Add duplicate `#[stage(name)]` detection
28. Replace `Vec` with `SmallVec` in `bind_block_args`
29. Change `register_ssa` to accept `impl Into<String>`
30. Replace `bon` in kirin-ir with hand-written builders
31. Structured `StageNameError` for `from_stage_name`
32. Make `Signature` fields private, add accessors
33. Rename signature/signature.rs
34. Extract binary-op helper for arith/bitwise/cmp interpret impls
35. Make `Interval` fields `pub(crate)` with accessors
36. Extract shared FunctionBody/Lambda SSACFGRegion+Interpretable impl
37. Add SimpleType to lattice law test suite
38. Document `#[callable]` attribute on derive proc-macro entry points

### Design Work (half-day+)
39. **Decouple dialect crates from top-level `kirin`** — depend on `kirin-ir` directly
40. Support `chumsky(format = ...)` for type enum derives
41. Extend `#[derive(Interpretable)]` for inner dialect enums (Lexical, Lifted, SCF)
42. Introduce `PrettyPrintViaDisplay` marker trait
43. Change `RenderDispatch` return type to `RenderError`
44. Simplify Interpretable trait bounds (explore trait simplification)
45. Investigate function resolution convenience API
46. `parse_and_emit` error type: preserve parse-vs-emit distinction
47. Unify error type hierarchy in kirin-chumsky
48. Formalize `AnalysisResult` with lattice traits
49. `is_missing_type_error`: use Option instead of string matching
50. Implement tighter interval division/remainder bounds
51. CompareValue boolean domain separation

### Documentation
52. Format string DSL reference documentation
53. "Which derives do I need?" cheat sheet
54. Document `__`-prefix variant convention + optional `#[kirin(hidden)]`
55. Document `Staged` lifetime semantics
56. Document `from_stage_name` single-dialect base case
57. Document `pretty_print_name`/`pretty_print_type` override contract
58. PhantomData: document `#[non_exhaustive]` approach

<details>
<summary>All Filtered Findings (~45 total)</summary>

**Phase 1 — Intentional design (AGENTS.md):**
- BlockInfo::terminator cache design
- Closed Dialect supertrait set
- Statement naming (vs MLIR's "Operation")
- `#[wraps]` per-variant vs enum-level dual pattern
- Tuple-based StageDispatch O(N)
- Three ParseEmit paths
- `HasDialectParser` 4 required items
- `'ir` lifetime threading complexity
- L-on-method technique
- SSACFGRegion marker trait
- `Custom<L>::separate()` closure pattern
- DeriveContext pre-computation
- Template method pattern differentiation
- Attribute namespace fragmentation
- Format string constrained to Kirin lexer tokens
- `BoxedParser` type-erasure overhead
- `PrettyPrint` monomorphization O(M*N)
- `kirin-chumsky` depends on `kirin-prettyless` for roundtrip
- `expect_info()` panics guard IR invariants

**Phase 1 — False positives:**
- `unit_cmp` allows in signature (generic code where `C = ()`)
- `chumsky` direct dep "removable" (needed for format string parsing)
- `choice()` arity limit concern (uses `.or()` chains)
- ValueStore/StageAccess impl "duplication" (different type params)
- Constructor `new`/`new_with_global` "duplication" (standard pattern)
- `InterpreterError` not Clone (intentional for extensibility)
- `Continuation::Fork` constructibility (runtime check sufficient)
- `petgraph` dependency (required)
- Heavy dev-dependencies (test-only)
- `syn` extra-traits (required by darling)

**Phase 1 — Won't Fix (user decision):**
- Builder panics (programmer errors)
- Long derive lists / `KirinDialect` shorthand (f64 breaks Eq/Hash)
- `EmitContext::resolve_ssa` forward-ref Result(0) convention

**Phase 2 — Intentional design / false positives:**
- PhantomData on generic dialect types
- `saturating_add(NegInf, PosInf)` asymmetry (unreachable after empty guards)
- Feature-gating in kirin-interval (intentional)
- `i64`-only intervals (scoped design)
- Dual free-function/operator API (standard Rust)
- RenderDispatch validation (compile errors sufficient)
- `#[wraps]`/`#[callable]` separation (intentional)
- All-`#[wraps]` restriction (tracked by P1-8)
- Feature flag explosion in test-languages (manageable at current scale)

**Phase 2 — Won't Fix (user decision):**
- CheckedDiv float naming (standard semantics)
- CallSemantics Result homogeneity documentation
- Test-languages unconditional parser/pretty features

</details>
