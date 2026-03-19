# PL Theorist — Formalism Review: kirin-interpreter

## Abstraction Composability

### Trait decomposition: ValueStore / StageAccess / BlockEvaluator / Interpreter

The interpreter is factored into four traits with strict layering:

1. `ValueStore` (`value_store.rs:8-24`) — pure SSA read/write interface. No lifetimes, no stage awareness.
2. `StageAccess<'ir>` (`stage_access.rs:14-101`) — pipeline reference + active stage. Returns `'ir`-lived references.
3. `BlockEvaluator<'ir>` (`block_eval.rs:18-73`) — block execution contract. Requires `ValueStore + StageAccess<'ir>`.
4. `Interpreter<'ir>` (`interpreter.rs:14-16`) — blanket supertrait over `BlockEvaluator<'ir>`. No new methods.

This decomposition follows the **stratified interface** pattern: each layer adds exactly one capability, and downstream consumers bind against the highest layer they need. The layering is:

```
ValueStore < StageAccess<'ir> < BlockEvaluator<'ir> = Interpreter<'ir>
```

This is a clean linear hierarchy (no diamond inheritance). The blanket `Interpreter` trait serves as the public API surface — dialect authors write `I: Interpreter<'ir>` and get all four capabilities.

**Composability**: A hypothetical "read-only analysis" that only needs value inspection could bound on `ValueStore + StageAccess<'ir>` without requiring `BlockEvaluator`. This flexibility is not currently exploited but is available.

### L on method, not trait — coinductive resolution

`Interpretable<'ir, I>` (`interpretable.rs:16-22`) places the language parameter `L` on the method `interpret<L>` rather than the trait. This is a deliberate encoding choice to break E0275 (recursive trait resolution):

- If `L` were on the trait: `Interpretable<'ir, I, L>` with `L: Interpretable<'ir, I, L>` creates an infinite regression during trait resolution (the compiler tries to prove `L: Interpretable` which requires `L: Interpretable` recursively).
- With `L` on the method: the compiler resolves `InnerType: Interpretable<'ir, I>` at the impl level (no `L`), then `L: Interpretable<'ir, I>` at the call site. The coinductive trait solver handles the self-referential method-level bound.

This is an instance of the **defunctionalization trick** — moving a type parameter from a "declaration" position (trait) to a "use" position (method) to exploit Rust's different resolution strategies at each level.

The same technique is used for `CallSemantics<'ir, I>` (`call.rs:12-26`).

### Continuation algebra

`Continuation<V, Ext>` (`control.rs:17-50`) is a sum type with 7 variants:

- `Continue` — identity continuation (next instruction)
- `Jump(Block, Args<V>)` — intra-procedural control transfer
- `Fork(SmallVec<...>)` — nondeterministic branch
- `Call { callee, stage, args, result }` — inter-procedural call
- `Return(V)` — function exit
- `Yield(V)` — inline body exit (SCF-style)
- `Ext(Ext)` — interpreter-specific extension

The `Ext` parameter defaults to `Infallible` (abstract interpreters have no extra variants) and is set to `ConcreteExt` (Break/Halt) for the stack interpreter. This is a clean **open recursion** pattern — the base continuation algebra is fixed, and extensions are injected via the type parameter.

Formally, `Continuation<V, Ext>` is the free monad of the "control flow effect" functor, where `Continue` is the unit, `Jump`/`Fork` are the intra-procedural effects, and `Call`/`Return`/`Yield` are the inter-procedural effects. `Ext` extends the effect signature.

### SSACFGRegion marker trait

`SSACFGRegion` (`call.rs:33-35`) is a marker trait that provides blanket `CallSemantics` impls for both `StackInterpreter` and `AbstractInterpreter`. This separates the **structural property** (this body type is an SSA CFG region) from the **operational semantics** (how to execute it), following the typeclass pattern where a marker trait enables coherent blanket impls.

The alternative would be to require each body type to manually implement `CallSemantics`. The marker approach gives a single declaration point for the "is an SSA CFG" property and derives the execution strategy automatically.

### AbstractValue and the widening contract

`AbstractValue: HasBottom` (`value.rs:21-47`) requires `widen` and `narrow` operations. The algebraic contracts are documented in comments:

- Widening: `x <= widen(x, y)` and `y <= widen(x, y)`, ascending chain must stabilize.
- Narrowing: `x meet y <= narrow(x, y) <= x`, descending chain must stabilize.

These match the standard definitions from Cousot & Cousot (1977, 1992). The `HasBottom` supertrait ensures every abstract domain has a least element, which is required for fixpoint initialization.

The `narrow` default (`self.clone()`) is the identity narrowing operator, which trivially satisfies the contract but provides no refinement. This is the standard "safe default" in abstract interpretation.

## Literature Alignment

### Correspondence to CESK machines

The `StackInterpreter` implements a variant of the CESK machine model (Felleisen & Friedman, 1987):

- **C**ontrol: `Frame::cursor` (current statement position)
- **E**nvironment: `Frame::values` (SSA value bindings)
- **S**tore: `ValueStore` trait (mutable value storage)
- **K**ontinuation: `FrameStack` (call stack as a list of frames)

The `Continuation` type extends the standard CESK transition rules with `Fork` (nondeterminism), `Yield` (inline body return), and `Ext` (interpreter extensions).

### Abstract interpretation framework

The `AbstractInterpreter` follows the standard abstract interpretation framework:

- Fixpoint computation over CFG blocks using a worklist algorithm (`FixpointState`, `DedupScheduler`)
- `AnalysisResult<V>` stores per-SSA-value abstract state
- `is_subseteq` on `AnalysisResult` checks convergence pointwise

The inter-procedural analysis uses a nested fixpoint: the inner loop iterates over blocks within a function, and the outer loop (in `CallSemantics` for `AbstractInterpreter`, `call.rs:79-154`) iterates over function summaries until convergence. This is the standard approach from Sagiv, Reps, and Horwitz (1996) for interprocedural dataflow.

The `SummaryCache` stores computed function summaries keyed by `(CompileStage, SpecializedFunction)`, and the `tentative` -> `computed` promotion pattern implements optimistic analysis: summaries start at bottom and are refined until convergence.

### Widening strategy

`WideningStrategy` provides delayed widening, where widening is applied only after a threshold number of visits. This matches the standard delayed-widening technique from Blanchet et al. (2003) in the Astree analyzer.

## Semantic Ambiguity

### `Continuation::Fork` in concrete interpreters

`Fork` (`control.rs:33`) is documented as "only reachable when `BranchCondition::is_truthy` returns `None`", and the concrete interpreter panics if it encounters it. However, the type system does not prevent dialect impls from constructing `Fork` when `Ext = ConcreteExt`. A phantom-type-based approach could make `Fork` unrepresentable for concrete interpreters, but this would significantly complicate the `Continuation` type. The current approach is pragmatic.

### `Staged<'a, 'ir, I, L>` ownership model

`Staged` (`stage.rs:15-18`) borrows the interpreter mutably (`&'a mut I`) and holds a stage reference (`&'ir StageInfo<L>`). The stage reference has lifetime `'ir` (from the pipeline), while the interpreter borrow has lifetime `'a` (typically shorter). This creates a scoped API where `Staged` cannot outlive the interpreter borrow. The asymmetric lifetimes are correct but could be confusing — documentation should clarify that `Staged` is a temporary builder, not a persistent handle.

### `AnalysisResult::bottom()` vs lattice bottom

`AnalysisResult::bottom()` (`result.rs:38-44`) creates an empty result (no values, no return). This is used as the initial value for fixpoint iteration. However, `AnalysisResult` itself does not implement `HasBottom` — it has a `bottom()` inherent method but not the lattice trait. Its `is_subseteq` method (`result.rs:87-114`) does pointwise comparison but uses the entry convention that "missing in other = not subsumed" for blocks and "None return = smaller than Some return". This is consistent but informal — the partial order on `AnalysisResult` is not a lattice (no join/meet operations are defined). Fixpoint convergence relies on the values-level `Lattice` impl, not an `AnalysisResult`-level one.

### Error type unification

`ValueStore::Error` and `BlockEvaluator`'s `Self::Error: From<InterpreterError>` constraint mean that dialect errors must be convertible from `InterpreterError`. This is a standard error-chaining pattern, but it means that `InterpreterError` variants (like `FuelExhausted`, `MaxDepthExceeded`) are mixed with domain-specific errors. The `Custom(Box<dyn Error>)` variant in `InterpreterError` provides the escape hatch.

## Alternative Formalisms Considered

### 1. Continuation: Sum type vs. Freer monad

**Current**: `Continuation<V, Ext>` as a flat enum with 7 variants + extensible `Ext`.
**Alternative A**: Freer monad encoding — `enum Effect { Jump, Call, Return, ... }` with a monadic interpreter that handles effects.
**Alternative B**: CPS encoding — `fn(interpreter) -> Result<Value>` closures for each continuation.

| Metric | Flat enum (current) | Freer monad | CPS |
|--------|---------------------|-------------|-----|
| Pattern matching | Direct, exhaustive | Indirect via handlers | None (opaque) |
| Extensibility | Via `Ext` parameter | Via effect extension | Via closure composition |
| Performance | Zero-cost dispatch | Allocation per effect | Closure allocation |
| Type safety | Full, compile-time | Full | Partial (closures opaque) |

The flat enum is the right choice: it enables exhaustive pattern matching in the transition function, has zero allocation overhead, and the `Ext` parameter provides sufficient extensibility.

### 2. Interpreter trait hierarchy: Linear vs. diamond

**Current**: Linear — `ValueStore < StageAccess < BlockEvaluator = Interpreter`.
**Alternative A**: Diamond — `ValueStore` and `StageAccess` independently feed into `Interpreter`.
**Alternative B**: Single monolithic trait.

| Metric | Linear (current) | Diamond | Monolithic |
|--------|-------------------|---------|------------|
| Minimal bounds | Yes (use lowest needed) | Yes | No (all or nothing) |
| Coherence issues | None | Potential ambiguity | None |
| Implementation count | 3 traits | 3 traits | 1 trait |
| Learning curve | Low (clear ordering) | Medium | Low |

Linear is correct here because `BlockEvaluator` genuinely requires both `ValueStore` and `StageAccess` — they are not independent for block evaluation. The linear chain reflects the true dependency.

### 3. L-on-method vs. L-on-trait for Interpretable

**Current**: `L` on method, coinductive resolution.
**Alternative A**: `L` on trait with explicit cycle-breaking (e.g., `PhantomData<L>` marker).
**Alternative B**: Dynamic dispatch via `Box<dyn Interpretable>`.

| Metric | L-on-method (current) | L-on-trait | Dynamic dispatch |
|--------|----------------------|------------|------------------|
| E0275 safety | Immune | Vulnerable | N/A |
| Trait bound count | 3 per method call | 1 per impl | 0 |
| Monomorphization | Full | Full | None |
| Derive macro complexity | Medium | Low | Low |

L-on-method is the principled solution. The extra method-level bounds are hidden by derive macros, and the coinductive resolution is well-understood in the Rust compiler. This is a genuine advance in encoding technique.

## Summary

- [P3] [confirmed] `AnalysisResult` has `bottom()` and `is_subseteq` but does not implement `HasBottom`/`Lattice` — informal partial order — `result.rs:38-44`
- [P3] [confirmed] `Continuation::Fork` is constructible in concrete interpreter contexts despite being documented as unreachable — `control.rs:33`
- [P3] [confirmed] `Staged` lifetime asymmetry (`'a` vs `'ir`) could benefit from documentation clarifying scoped-builder semantics — `stage.rs:15-18`
- [P3] [informational] The L-on-method technique for breaking E0275 is a well-motivated encoding choice — `interpretable.rs:16-22`
- [P3] [informational] The interpreter trait decomposition is a clean stratified interface with good composability properties — `value_store.rs`, `stage_access.rs`, `block_eval.rs`, `interpreter.rs`
