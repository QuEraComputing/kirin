# Kirin Codebase Design Review

**Date**: 2026-03-01
**Method**: 12 parallel reviewers (6 triad-design-review critics + 6 brainstorming simplifiers)
**Scope**: All crates in the workspace
**Constraint**: Keep existing features; focus on abstraction quality, simplification, and downstream ergonomics

---

## Table of Contents

- [Cross-Cutting Themes](#cross-cutting-themes)
- [Priority Matrix](#priority-matrix)
- [Architectural Strengths](#architectural-strengths)
- [Per-Crate Reviews](#per-crate-reviews)
  - [kirin-ir](#kirin-ir)
  - [kirin-interpreter + kirin-derive-interpreter](#kirin-interpreter--kirin-derive-interpreter)
  - [kirin-chumsky + kirin-chumsky-derive + kirin-chumsky-format](#kirin-chumsky--kirin-chumsky-derive--kirin-chumsky-format)
  - [kirin-derive-core + kirin-derive-dialect + kirin-derive](#kirin-derive-core--kirin-derive-dialect--kirin-derive)
  - [kirin-prettyless + kirin-lexer](#kirin-prettyless--kirin-lexer)
  - [Dialect Crates](#dialect-crates)
- [Open Questions](#open-questions)

---

## Cross-Cutting Themes

### 1. Stage Resolution Boilerplate (4 reviewers)

The 8-line `active_stage() -> stage() -> try_stage_info()` chain appears in every dialect interpret impl. All four reviewers (interp-simplifier, dialect-simplifier, interp-critic, dialect-critic) recommend a `resolve_stage<L>()` helper on `Interpreter` or as a free function.

### 2. PhantomData Ceremony (3 reviewers)

Every dialect struct carries `#[kirin(default)] marker: PhantomData<T>`. IR-simplifier, dialect-simplifier, and derive-critic all recommend the derive auto-inject it when `#[kirin(type = T)]` is present.

### 3. `std::HashMap` -> `FxHashMap` (3 reviewers)

Found in `InternTable` (ir-critic), `EmitContext` (parser-critic), `StackInterpreter` (interp-critic). Free performance wins throughout.

### 4. Code Duplication in Derive Infrastructure (3 reviewers)

`build_pattern`, `all_fields`/`field_pattern`/`field_name_tokens`, cached/non-cached code paths -- all identified independently.

### 5. Function Call Resolution (2 reviewers)

`Call::interpret` does O(N) linear scan of function arena. Both dialect-critic and dialect-simplifier recommend `Pipeline::function_by_name()`.

---

## Priority Matrix

### P0 -- Bugs / Correctness Hazards

| Issue | Source | Crate |
|-------|--------|-------|
| Block forward-reference ordering -- `br ^bbN` panics if target not yet emitted | parser-critic | kirin-chumsky |
| `Arena::gc()` without reference remapping -- silently corrupts pointers | ir-critic | kirin-ir |
| `call_handler` panic in AbstractInterpreter | interp-critic | kirin-interpreter |
| Arith Div/Rem panic on division by zero | dialect-critic | kirin-arith |
| Incomplete property lattice validation (`constant => pure` unchecked) | derive-critic | kirin-derive |

### P1 -- High-Impact Simplifications

| Issue | Source | Crate | Est. Savings |
|-------|--------|-------|-------------|
| Collapse `HasParser<'tokens, 'src>` to `HasParser<'src>` | parser-simplifier | kirin-chumsky | Pervasive |
| Unify cached/non-cached dispatch paths | interp-simplifier + interp-critic | kirin-interpreter | ~100 lines |
| Merge `kirin-derive-dialect` into `kirin-derive-core` (3->2 layer) | derive-simplifier | derive infra | 1 crate removed |
| Stage resolution helper | 4 reviewers | kirin-interpreter | ~15 lines/dialect |
| `Pipeline::function_by_name()` | dialect-critic + dialect-simplifier | kirin-ir | O(N)->O(1) calls |
| Remove `PhantomData<L>` from `BlockInfo`/`RegionInfo` | ir-simplifier | kirin-ir | Cascade simplification |
| `PrettyPrintExt` builder pattern (12 methods -> 1 + builder) | printer-simplifier | kirin-prettyless | ~24 methods |
| PhantomData auto-injection in derive | 3 reviewers | kirin-derive | ~40 lines/5 dialects |

### P2 -- Ergonomic Improvements

| Issue | Source | Crate |
|-------|--------|-------|
| `ParseDialect<L>` helper trait (bundle HRTB bounds) | parser-critic | kirin-chumsky |
| Shared `RecursiveAST<T>` (eliminate per-dialect `ASTSelf`) | parser-simplifier | kirin-chumsky |
| `WithStage<'a, L>` convenience wrapper for IR walking | ir-critic | kirin-ir |
| `pipeline.simple_function()` one-call shortcut | ir-critic | kirin-ir |
| Rename `#[kirin(type=...)]` to `#[kirin(ssa_type=...)]` on fields | derive-critic | kirin-derive |
| Merge `ScanResultWidth` into `PrettyPrint` | printer-simplifier | kirin-prettyless |
| Standardize dialect import patterns | dialect-simplifier | dialects |
| Add module docs to kirin-cf and kirin-scf | dialect-critic | dialects |
| Document E0275 limitation for Region-containing types | dialect-critic | kirin-function |

### P3 -- Low Priority / Cleanup

| Issue | Source |
|-------|--------|
| Remove dead `Config.line_numbers` | printer-critic |
| Remove redundant `.map(\|x\| x)` in `Arena::iter()` | ir-critic |
| Remove `Clone` bound from `DenseHint` Index | ir-critic |
| `Detach::detach` return `()` not `eyre::Result<()>` | ir-critic |
| Remove deprecated `InputBuilder`/`InputContext` re-exports | derive-simplifier |
| Extract tokenize helper | printer-critic |
| `FxHashSet<Use>` -> `SmallVec<[Use; 2]>` for SSA uses | ir-critic |
| Token builder macro for derive | derive-critic |
| Remove unused `FieldAccess<'a>` phantom lifetime | derive-critic |
| Remove or use `_ir_path` in `BoundsBuilder` | parser-critic |

---

## Architectural Strengths

Confirmed across multiple reviewers -- these should be preserved:

- **Trait decomposition** in interpreter (ValueStore / StageAccess / BlockEvaluator / Interpreter): "textbook clean"
- **`SSACFGRegion` marker trait**: avoids n*m instance problem for CallSemantics
- **`DispatchCache` fn-pointer vtable**: O(1) stage dispatch without trait objects
- **`Continuation<V, Ext>`** with `Infallible`/`ConcreteExt`: well-typed extension pattern
- **`Layout` trait** in derive-core: "well-designed open type family" with 4 associated types
- **`Scan`/`Emit` visitor**: correct analysis/synthesis separation
- **Format string DSL**: bidirectional parser+printer specification from a single annotation
- **Arena design**: cache-friendly Vec-backed with soft delete, O(1) allocation/lookup
- **`SignatureSemantics`**: proper dispatch abstraction with `applicable`/`cmp_candidate`
- **Lattice hierarchy**: clean algebraic tower (Lattice -> HasBottom/HasTop -> FiniteLattice -> TypeLattice)
- **Builder APIs**: ergonomic `bon::bon` pattern with `.name().argument().stmt().new()` chains
- **kirin-constant**: ideal teaching example (19 lines lib.rs + 21 lines interpret_impl.rs)
- **`ParseStatementTextExt`**: blanket impl erasing `()` context is a "pit of success" design
- **Two-pass pipeline parser**: correctly handles forward references
- **Single-parse derive optimization**: parse `Input<StandardLayout>` once, feed 16 generators
- **Prelude modules**: well-curated (7 symbols in interpreter, correct `#[cfg(feature)]` gating)

---

## Per-Crate Reviews

### kirin-ir

**Reviewers**: ir-critic (triad), ir-simplifier (brainstorming)

#### Critic: Triad Design Review

Three personas reviewed: Dr. Lambda (PL theorist), Casey (compiler engineer), Alex (DSL user).

**Consensus recommendations**:

1. **Switch `InternTable` to `FxHashMap`** -- Free perf win, no API change. (`intern.rs:12`)
2. **Remove redundant `.map(|x| x)` in `Arena::iter()`** -- Dead code. (`arena/data.rs:66`)
3. **Remove `Clone` bound from `DenseHint` Index impls** -- Unnecessarily restrictive. (`arena/hint/dense.rs:56-60`)
4. **Gate `TestSSAValue` behind `#[cfg(test)]` or feature flag** -- Noise in public API. (`node/ssa.rs:52`)
5. **Document or remove `Arena::gc()`** -- Without automated reference remapping, it's a correctness hazard. At minimum add `# Safety` documentation warning that all references become stale.
6. **`Detach::detach` should return `()` not `eyre::Result<()>`** -- The implementation never errors. (`detach.rs:8`)

**Trade-off decisions**:

- **`SSAKind` builder variants**: Keep single enum for now but add `#[doc(hidden)]` to `BuilderBlockArgument`, `BuilderResult`, `Test` and `debug_assert!` in interpreter/pass entry points. Revisit when pass infrastructure is implemented.
- **`Successor`/`Block` distinction**: Keep current design until block arguments are added to branch operations. When that happens, `Successor` should carry block arguments, making the distinction real.
- **`FxHashSet<Use>` vs `SmallVec`**: Switch to `SmallVec<[Use; 2]>` since most SSA values have 1-3 uses.
- **`GetInfo` ergonomics / `WithStage` wrapper**: Add a `WithStage<'a, L>` convenience wrapper in a future PR. Additive change that doesn't break existing APIs.
- **Three-level function hierarchy**: Add `pipeline.simple_function()` convenience for the common case.

**Key observations**:

- `Successor` and `Block` have free bidirectional conversion defeating newtype safety (`node/block.rs:28-37`)
- `SSAKind` mixes IR-semantic variants with builder/test transient placeholders (`node/ssa.rs:113-125`)
- `CompileTimeValue` blanket impl carries no algebraic meaning -- purely a bound bundle (`comptime.rs:7`)
- `GetInfo` pass-the-context pattern is ergonomically heavy for IR walking
- Builder APIs are ergonomic (bon::bon pattern)
- Arena design is clean and cache-friendly (Vec-backed, O(1))
- Lattice hierarchy is well-structured
- `SignatureSemantics` is a proper abstraction

#### Simplifier: Brainstorming Plan

**Priority 1: Remove `PhantomData<L>` from `BlockInfo` and `RegionInfo`**

`BlockInfo<L>` and `RegionInfo<L>` contain no dialect-specific data -- `_marker: PhantomData<L>`. The `L` parameter exists solely for routing through `GetInfo<L>` to the correct arena. Making these non-generic would eliminate dialect bounds from many signatures.

**Priority 2: Dialect Trait Default Implementations**

14 supertrait bounds require ~100 lines of boilerplate per test dialect even with macros. Provide default empty-iterator implementations on `Has{X}/Has{X}Mut` traits so dialects only override what they use.

**Priority 3: StageDispatch Consolidation**

`stage_dispatch.rs` is 947 lines but only ~170 lines of actual logic. 6 near-duplicate dispatch methods. Merge `StageAction`/`StageActionMut`, reduce 6 methods to 2 by returning a richer result type.

**Priority 4: Remove Redundant `LinkedListNode::ptr` Field**

`LinkedListNode<Ptr>` stores its own ID alongside `next`/`prev`. Always redundant -- the caller already has the ID.

**Priority 5: Inline Query Traits**

`ParentInfo`, `LinkedListInfo`, `LinkedListElem` exist primarily for the `Detach` macro. Only implemented for StatementInfo and BlockInfo. Inline the detach logic directly.

**Priority 6: `CompileTimeValue` Trait Removal**

Blanket-implemented marker trait for `Clone + Debug + Hash + PartialEq`. Replace with constituent bounds or trait alias.

---

### kirin-interpreter + kirin-derive-interpreter

**Reviewers**: interp-critic (triad), interp-simplifier (brainstorming)

#### Critic: Triad Design Review

**Consensus recommendations**:

1. **Replace `call_handler` panic with error return** (`abstract_interp/interp.rs:47-54`): The `Option<fn(...)>` with `.expect()` should be initialized to a stub returning `Err(InterpreterError::custom(...))`.
2. **Unify cached/non-cached code paths** (~100 lines duplicated): `run_nested_calls` vs `run_nested_calls_cached` (`stack/exec.rs:142-242`) and `push_call_frame_with_stage` vs `push_call_frame_with_stage_cached` (`stack/call.rs:61-151`) differ only in dispatch resolution. Collapse with a closure parameter.
3. **Deduplicate `resolve_dispatch_for_stage` and `lookup_dispatch_cached`** (`stack/transition.rs:28-83`) -- identical logic.
4. **Replace `HashSet<Statement>` with `FxHashSet<Statement>`** (`stack/interp.rs:51`).
5. **Document `#[callable]` behavior**: The `#[callable]` / `#[wraps]` interaction for CallSemantics derivation is undocumented and surprising.
6. **Deduplicate `build_pattern`** -- character-for-character identical in `interpretable/scan.rs:41-63` and `eval_call/scan.rs:46-68`.

**Trade-off decisions**:

- **Fork in base Continuation**: Keep. Dialect impls generic over `I: Interpreter` need to construct Fork without knowing Ext.
- **AbstractValue without Lattice supertype**: Should add Lattice as supertype -- contracts already presuppose it.
- **5 type parameters on StackInterpreter**: Keep with existing defaults.

**Architectural strengths noted**:

- Trait decomposition: "textbook clean"
- `SSACFGRegion` marker trait: avoids n*m instance problem
- `DispatchCache` with fn-pointer vtable: "clever engineering -- O(1) stage dispatch"
- `Continuation<V, Ext>` with `Infallible`/`ConcreteExt`: well-typed
- Prelude well-curated (7 symbols)
- SummaryCache with fixed/computed/tentative entries: clean API

#### Simplifier: Brainstorming Plan

**Proposal 1 (highest ROI): Eliminate dispatch duplication**

Remove non-cached variants (`run_nested_calls`, `push_call_frame_with_stage`). Since the dispatch table is always pre-built in `new()`, confine `SupportsStageDispatch` bounds to the constructor. Halves method count in `exec.rs` and `call.rs`. ~100 lines removed, dramatically simpler public API bounds.

**Proposal 2: Collapse trait hierarchy**

Merge `ValueStore` + `StageAccess<'ir>` + `BlockEvaluator<'ir>` into single `Interpreter<'ir>`. No downstream code uses sub-traits independently. Eliminates 3 files.

> **Note**: Interp-critic **disagrees** -- calls the decomposition "textbook clean". Tension to resolve: clean in theory but unused in practice.

**Proposal 3: Add `try_active_stage_info`**

Fallible version of `active_stage_info::<L>()` that returns `Result` instead of panicking. Eliminates ~15 lines of boilerplate per dialect interpret impl.

**Proposal 4: Flatten `abstract_interp/stage.rs`**

Merge 70-line `stage.rs` into `interp.rs`. Trivial cleanup.

**Lower priority**:

- `'ir` lifetime threading is fundamental, not easily removable
- `Continuation` enum is mostly fine as-is
- `StackInterpreter` type parameter defaults already handle common case

---

### kirin-chumsky + kirin-chumsky-derive + kirin-chumsky-format

**Reviewers**: parser-critic (triad), parser-simplifier (brainstorming)

#### Critic: Triad Design Review

**Consensus recommendations**:

1. **Fix block forward-reference ordering** (`ast.rs:388`): Region emission registers blocks AFTER emitting bodies. Forward `br ^bbN` panics if `^bbN` hasn't been emitted. Two-pass emit needed: first pass registers all block names, second pass emits bodies. Mirrors the pipeline parser's own two-pass design.
2. **Replace `std::HashMap` with `FxHashMap`** in `EmitContext` (`traits.rs:261-262`) and in `parse_text.rs` hashmaps.
3. **Unify `input_requires_ir_type`** (`input.rs:42-61` and `input.rs:90-111`) into a single generic function parameterized by layout type.
4. **Remove or use `_ir_path` in `BoundsBuilder`** (`bounds.rs:17`). Dead parameter.
5. **Add Levenshtein suggestions for unknown field names** in format string validation, matching the pattern already used for stage names in `parse_text.rs`.
6. **Eliminate `tokens.to_vec()` in `parse_one_declaration`** (`syntax.rs:136`). O(N) allocation per declaration, O(N^2) total for pipeline parsing.

**Trade-off decisions**:

- **`EmitIR::emit` fallibility**: Keep infallible but change `ast.rs` call sites to propagate errors via a new `EmitResult` field on `EmitContext` that collects diagnostics. Avoids changing the trait signature while eliminating panics.
- **Generated AST type naming**: Rename `FooAST`/`FooASTSelf` to `__FooAST`/`__FooASTSelf` with `#[doc(hidden)]`.
- **`ParseDialect<L>` helper trait**: Add blanket-implemented supertrait bundling the HRTB bounds. Pure ergonomic improvement.

**Key observations**:

- HasParser / HasDialectParser duality is well-founded (non-recursive base functor + recursive handle via GAT)
- EmitIR is a correct catamorphism (fold AST into IR)
- EmitContext breaks compositionality (mutable state bags with panic on undefined names)
- ASTSelf coinductive type wrapper is clever but undocumented
- `for<'src>` HRTB pattern is sound but creates ergonomic cliff
- `collect_existing_ssas` scans entire arena on every `parse_statement` (O(total_ssas) per call)
- Two-pass pipeline parser is well-designed for forward references
- Format string compile-time parsing is good (no runtime overhead)
- `ParseStatementTextExt` is a "pit of success" design

#### Simplifier: Brainstorming Plan

| Priority | Change | Risk |
|----------|--------|------|
| **P0** | Collapse `HasParser<'tokens, 'src>` to `HasParser<'src>` | Low |
| **P1** | Shared `RecursiveAST<T>` replaces per-dialect `ASTSelf` | Low |
| **P1** | Merge `HasDialectParser` + `HasParser` into one trait | Medium |
| **P2** | Reduce AST type params from 4 to 2 | Low-Med |
| **P2** | `ParseAndEmit` helper trait for cleaner bounds | Low |

Things that work well and should stay: EmitIR/EmitContext pattern, format string DSL, DirectlyParsable marker, ParseStatementText/ParsePipelineText API, FieldKind classification, validation visitor.

---

### kirin-derive-core + kirin-derive-dialect + kirin-derive

**Reviewers**: derive-critic (triad), derive-simplifier (brainstorming)

#### Critic: Triad Design Review

**Consensus recommendations**:

1. **Auto-default PhantomData fields** without requiring `#[kirin(default)]`. The derive already detects PhantomData at `builder/helpers.rs:91`.
2. **Complete property lattice validation**: Add `constant => pure` check (only `speculatable => pure` currently validated). (`property/scan.rs:40-85`)
3. **Extract duplicated code**: `all_fields`/`field_pattern` duplicated between `field/iter/statement.rs:129-139` and `property/statement.rs:58-78`. `field_name_tokens` duplicated between `field/iter/helpers.rs:75-78` and `property/statement.rs:80-83`.
4. **Add `callable` to `error_unknown_attribute`** (`misc.rs:124-162`) with hint pointing to `#[derive(CallSemantics)]`.
5. **Remove unused phantom lifetime** on `FieldAccess<'a>` (`field/iter/statement.rs:178`).

**Trade-off decisions**:

- **Rename field-level `#[kirin(type = ...)]` to `#[kirin(ssa_type = ...)]`**: Recommended with deprecated alias. Struct-level means "IR type lattice", field-level means "SSA type expression" -- same name, different semantics.
- **String-based field classification** (`Collection::from_type(ty, "SSAValue")`): Keep as default, document limitation, optionally support explicit `#[kirin(argument)]` overrides.
- **Token builder macro**: 438 lines of 3 near-identical builders. Worth doing but not urgent.

**Key observations**:

- `Layout` trait: well-designed open type family with 4 associated types
- `Scan`/`Emit` two-phase visitor: correct separation of analysis and synthesis
- Single-parse optimization: parse `Input<StandardLayout>` once, feed all 16 generators
- Data-driven config: `FieldIterConfig`/`PropertyConfig` const arrays with macros for individual derives
- Three-layer pattern (core -> dialect -> proc-macro) is justified by proc-macro crate boundary
- Stringly-typed field classification is not formally sound but practically reliable
- `VariantRef<'a, L>` Wrapper/Regular classification is good algebraic modeling

#### Simplifier: Brainstorming Plan

**Recommended approach (Option B)**:

1. **Merge `kirin-derive-dialect` into `kirin-derive-core`** -- The split is artificial with exactly one consumer. Reduces three-layer to two-layer: `kirin-derive-core` (shared IR + generators) -> `kirin-derive` (proc-macro entry points). Eliminates an entire crate.
2. **Replace hand-rolled builders** (`TraitImplTokens` etc.) with direct struct construction -- saves ~300 lines.
3. **Consolidate config tables** into generators themselves, making `kirin-derive` even thinner.
4. **Remove deprecated re-exports** (`InputBuilder`, `InputContext`).
5. **Extract `ScanContext<S>`** to reduce repeated `Option<InputMeta> + HashMap` pattern.

More aggressive option (deferred): Unify `Scan`/`Emit` into single `DeriveVisitor` trait. Loses explicit phase separation in the type system, but both are always called in sequence.

---

### kirin-prettyless + kirin-lexer

**Reviewers**: printer-lexer-critic (triad), printer-lexer-simplifier (brainstorming)

#### Critic: Triad Design Review

**Consensus recommendations**:

1. **Remove or implement `Config.line_numbers`** -- Dead code. Set and tested but never read by the rendering path. Bat pager hardcodes its own `line_numbers(true)`.
2. **Extract tokenize helper** -- `Token::lexer(src).spanned().map(...).collect()` pattern duplicated.
3. **Document the roundtrip property** on `PrettyPrint` trait.
4. **Improve `Document` API documentation** for manual `PrettyPrint` implementors.

**Trade-off decisions**:

- **`sprint_with_globals` API discoverability**: Keep current API but improve docs. Users call `sprint()` and get numeric IDs instead of function names, then must discover the `_with_globals` variant.
- **`try_sprint()` variants**: Don't add yet. Panics are fine for debugging/diagnostic tool. `Document::render` returns `Result` for users who need non-panicking.
- **`PipelineDocument` arena reuse**: Low priority since pipeline printing is not a hot path.

**Key observations**:

- `PrettyPrint` trait is a natural transformation from IR -> Document algebra
- `ScanResultWidth` breaks compositionality (two-pass with `&mut Document`)
- `PrettyPrintName`/`PrettyPrintType` don't require `L: PrettyPrint` -- inconsistent
- `RenderStage` correctly erases dialect type behind trait object for heterogeneous pipeline printing
- Lexer is minimal and fast (single file, Logos-based)
- `EscapedLBrace`/`EscapedRBrace` Display semantics are correct but naming is confusing
- `lex()` function may be unused in practice (callers use `Token::lexer().spanned()` directly)

#### Simplifier: Brainstorming Plan

| Change | Traits removed | Methods removed | Lines saved (est.) |
|--------|---------------|-----------------|-------------------|
| RenderBuilder pattern (replace 12-method `PrettyPrintExt`) | 0 | ~24 | ~100 |
| Merge `ScanResultWidth` into `PrettyPrint` | 1 trait | 8 impls become default methods | ~60 |
| Collapse `PrintExt`/`PipelinePrintExt` method explosion | 0 | ~10 | ~80 |
| Remove `PrettyPrintName`/`PrettyPrintType` as separate traits | 2 traits | 8 impls | ~50 |
| Remove dead alignment code (`result_width`, `max_result_width`) | 0 | 2 | ~20 |
| Inline lexer into kirin-chumsky (optional) | 0 | 0 | 1 crate boundary |

Total: 3 traits eliminated, ~30 methods consolidated, ~310 lines saved. Core API surface drops from 5 traits to 2 (`PrettyPrint` + `RenderStage`).

Proposed builder pattern replacement:
```rust
// Instead of 12 methods, one entry point:
let output = statement.render(&stage)
    .config(config)        // optional
    .globals(&gs)          // optional
    .to_string();          // or .print(), .write(&mut w), .bat()
```

---

### Dialect Crates

**Crates**: kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-function
**Reviewers**: dialect-critic (triad), dialect-simplifier (brainstorming)

#### Critic: Triad Design Review

**Consensus recommendations**:

1. **Add module-level docs to kirin-cf and kirin-scf** matching kirin-arith's quality. These are reference implementations and should be self-documenting.
2. **Document E0275 limitation** for Region-containing types with `#[wraps]` + `HasParser`. Add to Lambda type docs and AGENTS.md.
3. **Extract function-by-name resolution into Pipeline** as a method, eliminating O(N) linear scan in Call's interpret impl and reducing code by ~30 lines.
4. **Add stage-resolution helper** to collapse repeated 8-line chains in interpret impls.
5. **Address Div/Rem panic**: Either use checked operations or add clear doc comment establishing the contract.

**Trade-off decisions**:

- **Return duplication** (kirin-cf vs kirin-function): Document the overlap, don't refactor. Practical impact is low.
- **PhantomData boilerplate**: Track as derive infrastructure improvement, not dialect-level fix.
- **ArithValue match-arm repetition** (~150 lines): Leave as-is. Mechanical and greppable; macro would hurt IDE support.

**Key observations**:

- `Return` exists in both kirin-cf and kirin-function (identical semantics)
- Property annotations well-structured (`#[kirin(pure)]` with `#[kirin(speculatable)]` overrides)
- `Lexical` vs `Lifted` in kirin-function: clean categorical distinction for calling conventions
- `#[wraps]` delegation produces proper coproduct types
- SCF correctly uses `Block` instead of `Region` for single-block regions
- ~~No comparison operations~~ (reviewers missed `kirin-cmp` crate)
- `Call` linear scan O(N) per call site -- will not scale
- ArithValue manual Hash/PartialEq for floats is correct (to_bits)
- Feature flag pattern (`#[cfg(feature = "interpret")]`) is perfectly consistent across all dialects
- kirin-constant is the simplest possible dialect -- ideal teaching example
- Format strings are discoverable and readable

#### Simplifier: Brainstorming Plan

**Priority order (impact / effort)**:

1. **Use `#[derive(Interpretable)]` for wrapper enums** (e.g., `StructuredControlFlow`) -- low effort, immediate win, removes ~20 lines per wrapper.
2. **Standardize imports** -- inconsistent patterns across dialects (trivial).
3. **Stage resolution helper** (`fn resolve_stage<L>() -> Result<&'ir StageInfo<L>, InterpreterError>`) -- low effort, cleaner code.
4. **Call resolution utility** -- extract `resolve_call_target` into kirin-interpreter, reducing `Call::interpret` from 120+ lines to ~20 lines.
5. **PhantomData auto-injection** -- derive macro change, removes ~40 lines across 5 crates.
6. **Attribute-driven interpretation** (`#[kirin(interpret = "binary_op")]`) -- high effort but best long-term ROI for reducing interpret boilerplate.

---

## Decisions (from developer interview)

| Question | Decision | Notes |
|----------|----------|-------|
| `Arena::gc()` | **Keep but document** | Not used now but needed for rewrite framework. Add safety docs. |
| Successor / block args | **Keep current design** | Successor is just an ID. Block arguments already supported separately. Reviewer's suggestion to bundle them was based on misunderstanding. |
| `Return` duplication | **Remove from kirin-cf** | `kirin-function::Return` is canonical. Remove `ControlFlow::Return`. |
| Trait hierarchy | **Keep decomposed** | ValueStore / StageAccess / BlockEvaluator separation is intentional for future custom interpreters. |
| EmitIR error handling | **Two-pass region emit** | First pass registers block names, second pass emits bodies. Mirrors pipeline parser. |
| Comparison dialect | **Already exists** | `kirin-cmp` crate exists (reviewers missed it). |
| Inline kirin-lexer | **Keep separate** | Other frontends may reuse the lexer. |
| `Config.line_numbers` | **Remove it** | Dead code. Bat pager handles its own. |
| `#[kirin(fn)]` naming | **Rename to `#[kirin(builder)]`** | More self-explanatory. |
| `#[kirin(type=...)]` rename | **Keep as-is** | Context makes it clear. Not worth the churn. |
| Generated AST naming | **Safe to rename** | `__FooAST`/`__FooASTSelf` with `#[doc(hidden)]`. No code references them directly. |
| Roundtrip test coverage | **Need to check** | Unknown if tests exist elsewhere. Should verify and add more if not. |
