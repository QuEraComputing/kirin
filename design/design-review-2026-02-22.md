# Kirin Design Review Report — 2026-02-22

Conducted by a triad-design-review panel (PL theorist, compiler engineer, DSL user) across all 18 crates, grouped into 6 review areas.

---

## Review Groups

1. **Core IR**: `kirin-ir`, `kirin-lexer`
2. **Derive Infrastructure**: `kirin-derive-core`, `kirin-derive-dialect`, `kirin-derive`
3. **Parser/Printer**: `kirin-chumsky`, `kirin-chumsky-derive`, `kirin-chumsky-format`, `kirin-prettyless`, `kirin-prettyless-derive`
4. **Interpreter**: `kirin-interpreter`, `kirin-derive-interpreter`
5. **Dialects**: `kirin-cf`, `kirin-scf`, `kirin-constant`, `kirin-arith`, `kirin-bitwise`, `kirin-function`
6. **Testing**: `kirin-test-utils`

---

## Critical Issues (RED)

### 1. Branch/ConditionalBranch cannot carry block arguments
**Crate**: `kirin-cf`
**Location**: `crates/kirin-cf/src/lib.rs:7`, `crates/kirin-cf/src/interpret_impl.rs:29-33`

`Branch { target: Successor }` and `ConditionalBranch` always pass `smallvec![]` to `Continuation::Jump`. Without block arguments, the IR cannot express SSA value flow across control flow edges. The `Continuation::Jump` infrastructure already supports arguments — the dialect operations just don't surface them.

**Recommendation**: Add `args: Vec<SSAValue>` to `Branch` and both arms of `ConditionalBranch`, or make `Successor` itself carry arguments.

### 2. No comparison operations
**Crates**: `kirin-arith`, `kirin-bitwise` (gap)

There is no built-in dialect operation that produces a boolean/comparison result. `ConditionalBranch` requires a `BranchCondition::is_truthy()` value, but no operation produces one. This makes conditional branching unusable without a custom dialect.

**Recommendation**: Add comparison operations (eq, lt, gt, le, ge, ne) either as a new `kirin-cmp` crate or folded into `kirin-arith`.

### 3. Generated AST types leak into user code
**Crates**: `kirin-chumsky-derive`, `kirin-chumsky-format`

Users must reference `FooAST::Variant` and `.0` unwrap `ASTSelf` wrappers. The `parse::<L>(input, stage)` function that returns IR directly exists but isn't documented as the primary API.

**Recommendation**: Promote `parse::<L>()` as primary API. Consider `#[doc(hidden)]` on generated AST types.

---

## High-Priority Issues (YELLOW)

### Core IR

#### HashSet<Use> per SSA value is allocation-heavy
**Location**: `crates/kirin-ir/src/node/ssa.rs:63`

Most SSA values have 1-3 uses. A `SmallVec<[Use; 2]>` would have much better cache behavior and lower allocation overhead.

#### Arena::gc() has no automated ID remapping
**Location**: `crates/kirin-ir/src/arena/gc.rs`

After `gc()` compacts the arena, callers must manually walk the entire IR to remap IDs. This is error-prone and undocumented.

#### Dialect god-trait (14 supertraits)
**Location**: `crates/kirin-ir/src/language.rs:72-92`

Every dialect must implement 14 traits even if most return empty iterators. Prevents fine-grained capability-based bounds on optimization passes.

**Consensus**: Keep current design (derive macro eliminates boilerplate) but document the rationale.

#### Lattice traits lack documented algebraic laws
**Location**: `crates/kirin-ir/src/lattice.rs`

No documentation specifying that `join` must be associative/commutative/idempotent, or that `is_subseteq` should be consistent with `meet`.

#### 6 function-related types undocumented
**Location**: `crates/kirin-ir/src/node/function.rs`

`Function`, `StagedFunction`, `SpecializedFunction`, `FunctionInfo`, `StagedFunctionInfo`, `SpecializedFunctionInfo` — the three-level refinement hierarchy needs module-level documentation.

#### StatementIter missing DoubleEndedIterator
**Location**: `crates/kirin-ir/src/node/block.rs:140-163`

`BlockIter` has `DoubleEndedIterator` but `StatementIter` does not. Asymmetry.

#### Successor/Block bidirectional conversion
**Location**: `crates/kirin-ir/src/node/block.rs:27-37`

Free bidirectional conversion between `Successor` and `Block` undermines the newtype safety that was intended.

### Derive Infrastructure

#### 15 redundant parses of DeriveInput per #[derive(Dialect)]
**Location**: `crates/kirin-derive/src/lib.rs:204-244`

Each generator (`emit_field_iter` x10, `emit_property` x4, `DeriveBuilder` x1) independently parses the same `DeriveInput`. Should parse `Input<StandardLayout>` once and share.

#### Stringly-typed field classification
**Location**: `crates/kirin-derive-core/src/ir/statement.rs:121-176`

`parse_field` matches on the literal string `"SSAValue"`. Type aliases or renames silently misclassify. Inherent proc-macro limitation, but a lint/warning would help.

#### No attribute documentation
Users have no doc comments on `KirinStructOptions`, `KirinFieldOptions`, or `StatementOptions` explaining available `#[kirin(...)]` attributes.

#### #[wraps] outside #[kirin()] namespace
Bare `#[wraps]` attribute is inconsistent with `#[kirin(...)]` namespace for other attributes. Documented in AGENTS.md as a darling limitation.

#### Attribute struct duplication
`KirinStructOptions`, `KirinEnumOptions`, and `GlobalOptions` duplicate 6-7 fields each due to darling's `supports(...)` attribute requirement.

### Parser/Printer

#### Three attribute namespaces
`#[kirin(...)]`, `#[chumsky(...)]`, and `#[wraps]` — should unify under `#[kirin]`.

**Recommendation**: Migrate `#[chumsky(format = "...")]` to `#[kirin(format = "...")]`.

#### String allocations in EmitContext
**Location**: `crates/kirin-chumsky/src/traits.rs:168-169`

`HashMap<String, SSAValue>` should use `HashMap<&'src str, SSAValue>` since source text outlives the emit context.

#### Two-pass pipeline parsing re-tokenizes
**Location**: `crates/kirin-chumsky/src/function_text/parse_text.rs:219-297`

Second pass re-parses the same tokens. Should cache `Declaration` from first pass.

#### Poor error messages for stage dialect mismatch
**Location**: `crates/kirin-chumsky/src/function_text/parse_text.rs:67-217`

"stage has no registered parser dialect" doesn't list available dialects. Should collect dialect names during HList traversal.

#### HasParser vs HasDialectParser ad-hoc distinction
**Location**: `crates/kirin-chumsky/src/traits.rs:37-81`

Split forced by Chumsky's `Recursive` handle. Any type that starts non-recursive and later needs recursion requires a breaking trait change.

#### EmitContext uses stringly-typed lookups
**Location**: `crates/kirin-chumsky-format/src/generate/ast.rs:222-223`

Runtime panics on undefined SSA values. A typed context with indices would be safer.

### Interpreter

#### call_handler type-erased Option<fn>
**Location**: `crates/kirin-interpreter/src/abstract_interp/interp.rs:38-44`

Runtime panic if `analyze()` not used as entry point. Type safety lost.

**Consensus**: Keep (adding a type parameter is worse) but document invariant.

#### worklist.contains() is O(n)
**Location**: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:314`

Should add `HashSet<Block>` alongside `VecDeque` for O(1) membership check.

#### recursion_depth linear scan
**Location**: `crates/kirin-interpreter/src/stack.rs:144-146`

O(n) scan of frame stack per call. O(n^2) for recursive chains. Maintain a counter map.

#### #[callable] implicit behavior change
Adding `#[callable]` to any variant changes the meaning of `#[wraps]` on all other variants.

**Recommendation**: Require explicit `#[callable]` on all forwarding variants.

#### No simple "just run this function" API
`StackInterpreter` requires understanding frames, `call` vs `run` vs `step`.

**Recommendation**: Add `StackInterpreter::eval(func, args) -> Result<V, E>`.

#### FxHashMap for per-frame SSA values
**Location**: `crates/kirin-interpreter/src/frame.rs:16`

SSA values are typically dense indices. `Vec<Option<V>>` would give O(1) with better cache locality. Profile before changing.

#### No law-checking for AbstractValue
Broken `widen` can cause non-termination (defended only by `max_iterations`). Add debug assertions.

#### AnalysisResult cloning in fixpoint
**Location**: `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs:171,186`

Full result clones during recursive fixpoint convergence. Potentially expensive.

#### Blanket impl duplication
**Location**: `crates/kirin-interpreter/src/call_semantics.rs:38-121`

Frame setup logic duplicated between `push_call_frame_with_args` and `SSACFGRegion` blanket impl.

### Dialects

#### Div/Rem interpreter panics on division by zero
**Location**: `crates/kirin-arith/src/interpret_impl.rs:48-63`

Should return `InterpreterError::Custom` instead of panicking.

#### Duplicate Return (cf vs function)
`ControlFlow::Return` and `function::Return<T>` both produce `Continuation::Return(v)`. Composing both creates parser conflicts.

**Recommendation**: Document intended relationship. Is `function::Return` lowered to `ControlFlow::Return`?

#### E0275 on Lambda + #[wraps]
**Location**: `crates/kirin-function/tests/lambda_print.rs:8-11`

Region-containing types under `#[wraps]` hit recursive trait resolution overflow. Fundamental composability limitation.

#### Missing interpreter impls (bitwise, scf)
`kirin-bitwise` and `kirin-scf` have no interpreter support. Undocumented WIP.

#### ArithValue boilerplate (~230 lines)
**Location**: `crates/kirin-arith/src/types/arith_value.rs`

12-arm match repeated across 8 trait impls. A macro would help.

#### PhantomData<T> proliferation
Every operation in every dialect carries `PhantomData<T>`. Zero-cost but aesthetically unfortunate.

**Consensus**: Keep. The derive macro generates it; users rarely see it.

#### scf::If uses Block instead of Region
**Location**: `crates/kirin-scf/src/lib.rs`

Limits nesting. In MLIR, `scf.if` uses regions.

#### Lexical/Lifted naming opaque to non-PL audience
**Location**: `crates/kirin-function/src/lib.rs:21,31`

Add doc comments explaining in plain terms.

### Testing

#### SimpleType has bottom == top
**Location**: `crates/kirin-test-utils/src/simple_type.rs:17-51`

Single-element lattice. Cannot distinguish type errors from valid types. Fine for parser tests but confusing.

**Recommendation**: Rename to `TrivialType` or add prominent doc comment.

#### Two-crate-versions problem
`kirin-test-utils` depends on `kirin-chumsky`, preventing `kirin-chumsky` tests from using `TestDialect`.

**Recommendation**: Split into `kirin-test-types` (pure lattices) and `kirin-test-utils` (full helpers).

#### pub use SimpleIRType::*
**Location**: `crates/kirin-test-utils/src/lib.rs:35`

Namespace pollution. Users should import variants explicitly.

#### Bound::saturating_add convention undocumented
**Location**: `crates/kirin-test-utils/src/interval.rs:48`

`NegInf + PosInf = NegInf` is arbitrary and undocumented.

#### Div/Rem on Interval return top()
**Location**: `crates/kirin-test-utils/src/interval.rs:321-333`

Sound but imprecise. Acceptable for tests but worth a doc comment.

#### rustfmt helper silently degrades
**Location**: `crates/kirin-test-utils/src/rustfmt.rs:4-31`

Returns input unformatted if rustfmt not installed. Should warn.

---

## Consensus Recommendations Summary

### Must-Do
1. Add block arguments to `Branch`/`ConditionalBranch`
2. Add comparison operations
3. Replace `HashSet<Use>` with `SmallVec<[Use; 2]>` in SSAInfo
4. Parse `DeriveInput` once in `#[derive(Dialect)]` and share across generators
5. Use `&'src str` keys in `EmitContext`
6. Add `HashSet<Block>` for O(1) worklist membership in abstract interpreter
7. Wrap div/rem in checked operations returning `InterpreterError`

### Should-Do
8. Add debug assertions for widening laws in `WideningStrategy::merge`
9. Document lattice laws, function hierarchy, GC story, builder defaults
10. Promote `parse::<L>()` as primary API
11. Extract shared frame-setup logic from duplicated blanket impls
12. Add `StackInterpreter::eval()` convenience method
13. Add `DoubleEndedIterator` to `StatementIter`
14. Document `call_handler` invariant
15. Improve pipeline parsing error messages with dialect list

### Nice-to-Have
16. Unify attribute namespaces under `#[kirin]`
17. Split `kirin-test-utils` into types + utils
18. Make `#[callable]` behavior explicit
19. Add recursion depth counter map
20. Macro for `ArithValue`-style enum boilerplate
21. Rename `SimpleType` to `TrivialType` or document degeneracy
22. Remove `pub use SimpleIRType::*`
23. Document `Lexical`/`Lifted` naming in plain terms
24. Document `Return` duplication relationship
