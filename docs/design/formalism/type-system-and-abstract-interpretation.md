# Part IV - Type System & Abstract Interpretation

> Part of the [Rust Interpreter Formalism](index.md).

This part keeps both styles: lattice shorthand for analysis arguments and
concrete API references for the current `AbstractInterpreter` implementation.

## Reading Recipe

- **Formal read:** Read lattice/order claims (`⊑`, join/widen, fixpoint) as obligations on abstract domains and merge behavior consumed by the interpreter.
- **API read:** Inspect `crates/kirin-ir/src/{comptime.rs,lattice.rs}` for domain interfaces, then `crates/kirin-interpreter/src/abstract_interp.rs` (`CallContext`, `WideningStrategy`, `with_analysis`, `analyze_by_name`, `eval_cfg`, `eval_scope`) and example domains in `crates/{kirin-constprop,kirin-interval}`.

## IV.0 Symbol-to-code mapping

| Formal symbol / concept | Rust type / trait / function | Code |
| --- | --- | --- |
| Compile-time type universe | `CompileTimeValue` | [`crates/kirin-ir/src/comptime.rs`](../../../crates/kirin-ir/src/comptime.rs) |
| Value-to-type relation | `Typeof<Ty>` | [`crates/kirin-ir/src/comptime.rs`](../../../crates/kirin-ir/src/comptime.rs) |
| Placeholder type marker | `Placeholder` | [`crates/kirin-ir/src/comptime.rs`](../../../crates/kirin-ir/src/comptime.rs) |
| Lattice operations | `Lattice` | [`crates/kirin-ir/src/lattice.rs`](../../../crates/kirin-ir/src/lattice.rs) |
| Bottom element | `HasBottom` | [`crates/kirin-ir/src/lattice.rs`](../../../crates/kirin-ir/src/lattice.rs) |
| Top element | `HasTop` | [`crates/kirin-ir/src/lattice.rs`](../../../crates/kirin-ir/src/lattice.rs) |
| Widening operator | `Widen` | [`crates/kirin-ir/src/lattice.rs`](../../../crates/kirin-ir/src/lattice.rs) |
| Context abstraction (summary-key strategy) | `CallContext<V>` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Merge/widen behavior | `WideningStrategy<V>` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Context-insensitive baseline (API name) | `ContextInsensitive` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Abstract engine | `AbstractInterpreter<...>` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Analysis entrypoint | `AbstractInterpreter::analyze_by_name` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| CFG fixpoint kernel | `AbstractInterpreter::eval_cfg` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Scope fixpoint kernel | `AbstractInterpreter::eval_scope` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Constprop analysis crate | `kirin-constprop` | [`crates/kirin-constprop/src/lib.rs`](../../../crates/kirin-constprop/src/lib.rs) |
| Interval analysis crate | `kirin-interval` | [`crates/kirin-interval/src/lib.rs`](../../../crates/kirin-interval/src/lib.rs) |

## IV.1 Separation of Concerns

Current Kirin separates:

1. **Compile-time IR type metadata** (`L::Type` on SSA info/signatures)
2. **Runtime abstract interpretation domain** (`I::Value` in `AbstractInterpreter`)

They may be related in a specific analysis, but are not the same framework type.

## IV.2 Compile-Time Type Layer

Compile-time type model is dialect-defined (`CompileTimeValue`) and threaded via
IR construction, signatures, and operation metadata. This layer supports parsing,
printing, signatures, and lowering decisions.

This formalism does not assume a single global HM-style type inference pass in
the interpreter engine.

## IV.3 Abstract Domain Requirements

`AbstractInterpreter` currently requires:

- `V: Clone + PartialEq + Widen + HasBottom`
- `E: From<InterpreterError>`

Analysis parameter (`P`) controls:

- context abstraction / summary keying (`CallContext`)
- merge/widen behavior (`WideningStrategy`, typically join then widen)

API-level realization:

- summary keying: `CallContext::key`
- state merge: `WideningStrategy::merge`
- analysis injection: `AbstractInterpreter::with_analysis(...)`

The context-insensitive baseline keys summaries by `(stage, specialization)`
with `widen_after` threshold.

Strategy motivation:

- Context abstraction and merge behavior are analysis-specific precision/cost knobs.
- Keeping them in analysis parameter `P` avoids hard-coding one recursion/context strategy
  in the engine.
- `ContextInsensitive` gives the baseline behavior; analyses can opt into custom
  context abstractions (for example constprop's bounded arg-tuple contexts).

## IV.4 Soundness Shape

Dialect rule soundness obligation:

- concrete transfer over-approximated by abstract transfer on corresponding
  abstract values

Because dialect code is generic over `I: Interp`, the same rule body executes on
abstract values; soundness depends on domain operations (`join`, `widen`,
branch-condition traits, loop-condition traits), not duplicated dialect logic.

## IV.5 Three Engine-Owned Fixpoints

Abstract engine convergence relies on:

1. **CFG fixpoint**:
   - block input states joined across incoming edges
   - widening after configured number of merges
2. **Structured scope fixpoint**:
   - loop scopes re-enter with merged/widened entry products until stable
3. **Interprocedural summary fixpoint**:
   - per-summary-key entry/return products joined over call graph until stable

These are all engine responsibilities; dialects only provide one-step `Effect`.

## IV.6 Undecided Control and Precision

When value domain cannot decide:

- CFG condition -> `Effect::Branch`
- structured condition -> `Effect::EnterAny`
- loop hook condition -> `ScopeStep::RepeatOrFinish`

Concrete engine rejects these; abstract engine explores alternatives and joins
results. Precision depends on domain + merge behavior, not on dialect duplication.

## IV.7 Domain Examples (In-Tree)

- const-propagation style domain (`kirin-constprop`)
- interval domain (`kirin-interval`)
- test lattices (`kirin-test-types`)

Constprop currently uses context-sensitive, value-based polyvariant summary keys
(`ConstPropContext`):

- fully constant argument tuples get distinct contexts
- unknown/non-constant or budget overflow collapse to shared `Unknown`
- precision increases on recursive constants, while overflow degrades to sound
  `Top` and still terminates

### Why context sensitivity matters (worked recursion examples)

The context-insensitive baseline (`ContextInsensitive`) keys every summary by
`(stage, specialization)`, so all call sites of a function share one entry
summary. Under that keying, a recursive function joins the arguments of *all* its
self-calls into a single entry product. For `factorial(Const(5))` the recursive
call `factorial(n-1)` would join `5 ⊔ 4 ⊔ 3 ⊔ … = Top` at the shared entry, and
the analysis would only ever prove `factorial : Top → Top`. Context-insensitive
keying is sound but imprecise on recursion over distinct constants.

`ConstPropContext` keys each fully-constant argument tuple under its own summary,
which makes two recursion shapes precise:

- **Linear recursion — factorial.** `factorial(Const(5))` unfolds
  `5 → 4 → 3 → 2 → 1` under five distinct keys; each level folds its callee's
  return back exactly, yielding `Const(120)`. See
  [`example/toy-lang/programs/factorial.kirin`](../../../example/toy-lang/programs/factorial.kirin)
  (test `constprop_source_recursive_factorial`).
- **Overlapping-subproblem recursion — fibonacci.** `fib(n)` calls both
  `fib(n-1)` and `fib(n-2)`, so the call graph is a DAG in which `fib(k)` is
  reached along many paths. Per-constant summary keys memoize each `fib(k)`
  once, so the analysis is both *precise* (`fib(Const(10)) = Const(55)`) and
  *non-explosive* — the shared summaries are what stop the two call sites from
  re-deriving every subproblem. See
  [`example/toy-lang/programs/fibonacci.kirin`](../../../example/toy-lang/programs/fibonacci.kirin)
  (test `constprop_source_recursive_fibonacci`).

On unknown input both functions key the single shared `Unknown` context; every
self-call lands on that same key, and the self-dependency re-enqueueing converges
the rising return summary to sound `Top` (tests
`constprop_source_recursive_{factorial,fibonacci}_unknown_is_top`). A
per-function context budget (`max_contexts`) bounds the number of distinct
constant contexts, so deep/wide constant recursion degrades to `Unknown`/`Top`
rather than diverging.

Any new domain should document:

1. partial order and join/widen behavior
2. `bottom` meaning and unbound-read interpretation
3. branch/loop decision semantics (`Option<bool>`-style traits)
4. convergence argument under configured analysis context/merge behavior
