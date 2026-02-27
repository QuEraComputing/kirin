# Kirin Triad Design Review -- 2026-02-24

Comprehensive design review of all 20 crates using three-persona triad analysis (Dr. Lambda -- PL Theorist, Casey -- Compiler Engineer, Alex -- DSL User/Physicist).

---

## Executive Summary

The Kirin framework has **strong architectural bones**: the MLIR-inspired arena-based IR, derive macro infrastructure, and bidirectional format-string-driven parser/printer are well-designed. However, several cross-cutting issues emerged consistently across all reviewers:

1. **Incomplete dialect ecosystem** -- ~~no comparison operations~~, unusable SCF dialect
2. **Ergonomic friction** -- excessive IR builder ceremony, `GetInfo<L>` pass-the-context pattern
3. **Performance low-hanging fruit** -- `std::HashMap` where `FxHashMap` should be used, unnecessary allocations
4. **Correctness hazards** -- arena GC with no ID remapping, division-by-zero panics

---

## Severity Matrix (Cross-Crate)

### RED -- Blocking Issues

| # | Issue | Crate(s) | Reviewer |
|---|-------|----------|----------|
| R1 | ~~**No comparison operations**~~ -- **Fixed.** New `kirin-cmp` crate with `Cmp<T>` dialect (eq/ne/lt/le/gt/ge) and `CompareValue` trait | ~~kirin-cf, kirin-arith~~ | Dialects |
| R2 | **SCF uses Block instead of Region** -- **Reclassified as correct.** MLIR's SCF dialect uses `SingleBlock` regions; Kirin's `Block` is the direct equivalent of a single-block region. Nesting works via SCF ops inside block statements. | kirin-scf | Dialects |
| R3 | **Arith Div/Rem panics on division by zero** -- runtime crash in interpreter | kirin-arith | Dialects |
| R5 | **IR builder boilerplate** -- 50-100 lines per multi-block test, duplicated builders across files | kirin-interpreter tests | Tests |
| R6 | **`tokens.to_vec()` per declaration** -- O(n*k) allocation in function parser | kirin-chumsky | Parser |
| R7 | **Generated AST type names leak into errors** -- `FooAST<'tokens, 'src, ...>` confuses users | kirin-chumsky-derive | Parser |
| R8 | **Arena GC returns stale IDs with no remapping** -- calling `gc()` corrupts the IR | kirin-ir | IR |
| R10 | ~~**kirin-bitwise and kirin-scf have no interpreter impls**~~ -- **Fixed.** Bitwise: full impl using `std::ops` traits. SCF: `If` uses conditional jump/fork, `Yield` returns value to parent op, `For` deferred (needs loop-back infrastructure). | kirin-bitwise, kirin-scf | Dialects |

### YELLOW -- Significant Issues

| # | Issue | Crate(s) | Reviewer |
|---|-------|----------|----------|
| Y1 | `std::HashMap` in `InternTable` and `EmitContext` instead of `FxHashMap` | kirin-ir, kirin-chumsky | IR, Parser |
| Y2 | `FxHashSet<Use>` per SSA value -- allocation-heavy for 1-3 uses | kirin-ir | IR |
| Y3 | `Successor`/`Block` free conversion eliminates newtype safety | kirin-ir | IR |
| Y4 | `GetInfo<L>` pass-the-context-everywhere pattern is ergonomically heavy | kirin-ir | IR |
| Y5 | Three-level function hierarchy has high ceremony for simple cases | kirin-ir | IR |
| Y6 | `HasDialectParser` exposed in public docs without "internal" marking | kirin-chumsky | Parser |
| Y7 | No systematic roundtrip test coverage | kirin-chumsky, kirin-prettyless | Parser |
| Y8 | String allocations in `EmitIR` impls | kirin-chumsky | Parser |
| Y9 | `worklist.contains()` is O(n) in abstract interpreter inner loop | kirin-interpreter | Interpreter |
| Y10 | Duplicate argument binding code in 3 locations | kirin-interpreter | Interpreter |
| Y11 | `WideningStrategy::AllJoins` is a misnomer (widens at every revisit) | kirin-interpreter | Interpreter |
| Y12 | Stringly-typed field classification in derive macros | kirin-derive-core | Derive |
| Y13 | Property lattice only partially validated (constant->pure missing) | kirin-derive-dialect | Derive |
| Y14 | `#[callable]` attribute is undiscoverable | kirin-derive-interpreter | Derive |
| Y15 | PhantomData `#[kirin(default)]` is mandatory incantation | kirin-derive-core | Derive |
| Y16 | `FunctionBody` can't be used via `#[wraps]` due to E0275 | kirin-function | Dialects |
| Y17 | Duplicate `Return` type across kirin-cf and kirin-function | kirin-cf, kirin-function | Dialects |
| Y18 | Interval Div/Rem over-approximate to top | kirin-interval | Dialects |
| Y19 | `dump_function` hardcoded to `CompositeLanguage` | kirin-test-utils | Tests |
| Y21 | No documentation of two-crate-versions problem | kirin-test-utils | Tests |
| Y22 | Missing module-level docs in 4 of 7 dialect crates | kirin-cf, kirin-scf, kirin-constant, kirin-function | Dialects |
| Y23 | `call_handler` freezes dialect parameter L | kirin-interpreter | Interpreter |
| Y24 | Abstract interpreter trait bounds extremely verbose | kirin-interpreter | Interpreter |

### GREEN -- Minor Issues

| # | Issue | Crate(s) |
|---|-------|----------|
| G1 | Identity `.map()` in `Arena::iter()` | kirin-ir |
| G2 | `DenseHint` `Clone` bound overly restrictive | kirin-ir |
| G3 | `TestSSAValue`/`SSAKind::Test` in public API | kirin-ir |
| G4 | No `Region::statements()` convenience iterator | kirin-ir |
| G5 | `FormatOption` enum not extensible | kirin-chumsky-format |
| G6 | `BlockLabel::emit` panics on undefined block | kirin-chumsky |
| G7 | No `Display` for `Continuation` | kirin-interpreter |
| G8 | `recursion_depth()` is O(n) scan | kirin-interpreter |
| G9 | Phantom lifetime on `FieldAccess` is dead code | kirin-derive-dialect |
| G10 | Manual Clone impls could be derived | kirin-derive-core |
| G11 | Three identical `build_pattern` functions | kirin-derive-interpreter, kirin-derive-dialect |
| G12 | `parse_tokens!` macro exported but unused | kirin-test-utils |
| G13 | Missing `Eq` bound on lattice assertion functions | kirin-test-utils |
| G14 | `CompileTimeValue` blanket impl maximally permissive | kirin-ir |

---

## Per-Crate Detailed Reviews

### 1. kirin-ir (Core IR)

**Strengths:**
- Clean arena-based design with `Vec<Item<T>>` + index IDs
- Well-documented lattice traits with algebraic laws
- `SignatureSemantics` trait with `ExactSemantics`/`LatticeSemantics` is extensible
- Builder pattern (`stage.block().argument(ty).stmt(s).terminator(t).new()`) is ergonomic
- Error-as-recovery pattern preserves construction args

**Key Recommendations:**
1. Switch `InternTable` to `FxHashMap` (free perf win, already a dependency)
2. Add block arguments to `Successor` or document why they're intentionally absent
3. Gate `TestSSAValue`/`SSAKind::Test` behind `#[cfg(test)]` or feature flag
4. Restrict `Arena::gc()` to `pub(crate)` until remapping infrastructure exists
5. Switch `SSAInfo::uses` to `SmallVec<[Use; 2]>`
6. Add `WithStage<'a, L>` wrapper for ergonomic method chaining
7. Add convenience `Pipeline::define_function()` for the common case
8. Replace `IndexMap` in `FunctionInfo` with `Vec<(CompileStage, StagedFunction)>`

**Questions:**
- Is `Successor` intended to eventually carry block arguments (MLIR-style)?
- Is `Arena::gc()` called anywhere in the codebase?
- Is `IndexMap`'s insertion-order preservation used for `staged_functions`?

---

### 2. Derive Infrastructure (kirin-derive-core, kirin-derive-dialect, kirin-derive, kirin-derive-interpreter)

**Strengths:**
- `Layout` trait is an elegant extensibility mechanism (row polymorphism for attributes)
- Single-parse path eliminates 15x redundant parsing
- Scan/Emit visitor pair provides consistent code generation
- Thorough snapshot test coverage
- `#[wraps]` + `VariantRef` enables clean dialect composition

**Key Recommendations:**
1. Extract duplicated `build_pattern` into `kirin-derive-core`
2. Add `#[callable]` error redirect in `error_unknown_attribute`
3. Auto-default PhantomData fields (detect type, set `DefaultValue::Default`)
4. Remove phantom lifetime on `FieldAccess`
5. Add `#[kirin(kind = argument|result|...)]` as explicit field classification override
6. Add `constant -> pure` validation in property lattice

**Questions:**
- Should `constant` always imply `pure`?
- Are more bare attributes planned beyond `#[callable]`?

---

### 3. Parser/Printer (kirin-chumsky, kirin-chumsky-derive, kirin-chumsky-format, kirin-lexer, kirin-prettyless, kirin-prettyless-derive)

**Strengths:**
- Two-phase parsing (AST then IR emission) is categorically correct
- Format string syntax (`{field:option}`) is intuitive and drives both parsing and printing
- `ParseStatementTextExt` Ctx pattern is elegant
- StageDialects HList-based type-level dispatch is clean
- Levenshtein distance suggestions for typos in stage names
- Pretty printer arena allocation with `Deref` to `DocAllocator` is clean

**Key Recommendations:**
1. Eliminate `tokens.to_vec()` in `parse_one_declaration` -- use iterator-based stream
2. Replace `std::HashMap` with `FxHashMap` in `EmitContext`
3. Add `#[doc(hidden)]` to `HasDialectParser`
4. Rename generated types (`FooAST` -> `__FooOutput`) with `#[doc(hidden)]`
5. Document the ASTSelf coinductive pattern
6. Add Levenshtein suggestions for unknown field names in format strings
7. Add roundtrip test macro for systematic coverage

**Questions:**
- Is `HasDialectParser` ever called directly by downstream users?
- Has the `tokens.to_vec()` allocation been profiled?
- Is `line_numbers` config option actually used anywhere?

---

### 4. kirin-interpreter

**Strengths:**
- `Continuation<V, Ext>` open sum with `Infallible` default is well-designed
- `AbstractValue` algebraic contracts are documented
- Clean separation of `Interpretable` and `CallSemantics`
- `Args<V> = SmallVec<[V; 2]>` is good inline size choice
- `InterpreterError` variants are clear and actionable

**Key Recommendations:**
1. Add `FxHashSet<Block>` worklist side-set for O(1) membership
2. Extract argument binding helper (deduplicate 3 copies)
3. Rename `WideningStrategy::AllJoins` to `Always`
4. Switch breakpoints to `FxHashSet`
5. Consider `Vec<Option<V>>` for frame values (dense SSA indices)

**Questions:**
- Would you accept `Vec<Option<V>>` for frame values?
- Where should IR well-formedness checking live?

---

### 5. Dialect Crates (kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-function, kirin-interval)

**Strengths:**
- Consistent derive pattern across all dialects
- Feature-gated interpreter separation is clean
- `ArithType`/`ArithValue` with custom float bit-equality is solid
- `BranchCondition::is_truthy` three-valued logic is the right abstraction
- Interval lattice laws are tested
- kirin-arith and kirin-bitwise have excellent module docs

**Key Recommendations:**
1. ~~**Add comparison operations**~~ -- **Fixed.** New `kirin-cmp` crate with `Cmp<T>` dialect and `CompareValue` trait (feature-gated behind `interpret`). Interval domain support in kirin-interval behind `cmp` feature.
2. ~~Fix SCF -- change Block to Region for If/For bodies~~ -- **Reclassified as correct** (MLIR SCF uses SingleBlock regions)
3. **Add interpreter impls** for kirin-bitwise and kirin-scf
4. **Fix Div/Rem** -- pre-check `IsZero` before delegating to `std::ops::Div`
5. Add module-level docs to kirin-cf, kirin-scf, kirin-constant, kirin-function
6. Document the `Function { body: Region }` boilerplate pattern

**Questions:**
- ~~Should comparison ops go in kirin-arith (adding Bool) or a new kirin-cmp?~~ **Resolved:** Separate `kirin-cmp` crate so interpreter only requires `CompareValue`, not arithmetic trait bounds.
- Is SCF intended to be first-class or experimental?
- Is `ControlFlow::Return` intended to diverge from `function::Return`?

---

### 6. Test Infrastructure (kirin-test-utils, kirin-test-languages)

**Strengths:**
- Lattice law verifier with batch violation reporting is excellent
- Roundtrip utilities (`assert_statement_roundtrip`, `assert_pipeline_roundtrip`) are clean and composable
- `rustfmt` helper is resilient with proper fallback
- `CompositeLanguage` composes real production dialects for realistic testing
- Snapshot testing with insta catches regressions

**Key Recommendations:**
1. **Add lattice law tests for `SimpleType`** in kirin-test-languages
2. Move duplicated IR builder functions to kirin-test-utils
3. Make `dump_function` generic over `L: Dialect`
4. Document the two-crate-versions problem
5. Improve `emit_statement` panic messages to include input and errors

**Questions:**
- Is a fluent IR builder DSL for multi-block test programs desirable?

---

## Cross-Cutting Themes

### Theme 1: `std::HashMap` -> `FxHashMap`
Multiple crates use `std::HashMap` for compiler-internal lookups where keys are short identifiers. The crate already depends on `rustc-hash`. Affected locations:
- `kirin-ir/src/intern.rs:12` (InternTable)
- `kirin-chumsky/src/traits.rs:261-262` (EmitContext)
- `kirin-interpreter/src/stack.rs:46` (breakpoints)

### Theme 2: Missing Dialect Completeness
The dialect ecosystem has structural gaps that prevent standalone use:
- ~~No comparison ops~~ -> fixed via `kirin-cmp`
- SCF with Block not Region -> no nesting
- Missing interpreter impls (bitwise, SCF)

### Theme 3: Ergonomic Friction
The arena-based design requires passing `&StageInfo<L>` to every operation. A `WithStage<'a, L>` wrapper pattern would significantly improve the day-to-day authoring experience. Similarly, the three-level function hierarchy (Function -> StagedFunction -> SpecializedFunction) needs a convenience shorthand.

### Theme 4: Test Infrastructure Quality
The test infrastructure has strong utilities (lattice verifier, roundtrip helpers) but the actual test code suffers from massive boilerplate duplication. A shared IR builder library would dramatically improve test authoring velocity.

---

## Developer Decisions (2026-02-24 Discussion)

- **R3 (Div/Rem panic)**: **Accepted as intentional.** The Arith interpreter delegates to Rust's native division semantics. If the value type panics on zero, that aligns with Rust's behavior for Rust-building type systems. Reclassified from RED to GREEN (by design).
- **R1 (No comparison operations)**: **Fixed.** New `kirin-cmp` crate with `Cmp<T>` dialect (eq/ne/lt/le/gt/ge). `CompareValue` trait and `i64` impl are feature-gated behind `interpret`. Interval domain `CompareValue` impl in kirin-interval behind `cmp` feature. Comparison ops are separate from arithmetic so the interpreter only requires `CompareValue`, not `Add+Sub+Mul+...`.
- **R4 (SimpleIRType lattice)**: **Fixed.** Deleted `SimpleIRType` from tests/simple.rs, refactored to use `kirin-test-languages::SimpleType` with correct flat lattice. Also added PrettyPrint derive to `SimpleLanguage` behind `pretty` feature flag.
- **R8 (Arena GC remapping)**: **Deferred.** No rewriting infrastructure exists yet, so remap_ids is premature. Will revisit when rewriting passes are implemented.
- **R9 (Convergence check)**: **Fixed.** Added `AnalysisResult::is_subseteq` for pointwise comparison of return values and block argument abstract values. Convergence check now compares full analysis results, not just return values.
- **R2 (SCF uses Block instead of Region)**: **Reclassified as correct (GREEN).** MLIR's `scf.if` and `scf.for` use `SingleBlock` + `SingleBlockImplicitTerminator<scf::YieldOp>` regions — each body region contains exactly one block. Kirin's `Block` type directly models this single-block semantics. Nesting is achieved by placing SCF ops inside block statement lists, not by having multi-block regions.

## Recommended Priority Order

**High (completeness):**
1. ~~Add comparison operations (R1)~~ -- **Done**
2. ~~Fix SCF to use Region (R2)~~ -- **Reclassified as correct**
3. Add interpreter impls for bitwise/SCF (R10)

**Medium (performance/ergonomics):**
4. Switch to FxHashMap everywhere (Y1)
5. Eliminate `tokens.to_vec()` allocation (R6)
6. Add `WithStage<'a, L>` wrapper (Y4)
7. Move duplicated builders to test-utils (R5)
8. Improve generated type names (R7)

**Lower (polish):**
9. Auto-default PhantomData in derives (Y15)
10. Add missing dialect docs (Y22)
11. Extract duplicated `build_pattern` (G11)
12. SmallVec optimizations (Y2)
