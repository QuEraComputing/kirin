# Part III - Operational Semantics

> Part of the [Rust Interpreter Formalism](index.md).

## Reading Recipe

- **Formal read:** Use `⟨s, ρ, σ⟩ ⇓_ι Result<(σ', φ), ε>` for statement meaning and interpret engine behavior as transitions over continuations/worklists that consume `φ`.
- **API read:** Follow `dispatch_statement` (`dispatch.rs`) -> `Interpretable::interpret` (dialect `*/interpreter.rs`) -> `Effect` (`effect.rs`) -> concrete frame driver (`frame.rs`, `concrete.rs`) or abstract fixpoint driver (`abstract_interp.rs`).

## III.0 Symbol-to-code mapping

| Formal symbol / concept | Rust type / function | Code |
| --- | --- | --- |
| Statement judgment payload `φ` | `Effect<V, E>` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| CFG edge | `Edge<V>` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| Call effect | `CallEffect<V>` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| Structured scope | `Scope<V, E>`, `ScopeBody` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| Loop transition strategy | `ScopeHook`, `ScopeStep` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| Statement dispatch | `Interpretable<I>`, `InterpDispatch<I>` | [`crates/kirin-interpreter/src/dispatch.rs`](../../../crates/kirin-interpreter/src/dispatch.rs) |
| Runtime context | `Ctx<'_, I>` | [`crates/kirin-interpreter/src/ctx.rs`](../../../crates/kirin-interpreter/src/ctx.rs) |
| Frame protocol | `Frame<I>`, `FrameDriver` | [`crates/kirin-interpreter/src/frame.rs`](../../../crates/kirin-interpreter/src/frame.rs) |
| Scope continuation frame | `ScopeFrame<V, E>` | [`crates/kirin-interpreter/src/frame.rs`](../../../crates/kirin-interpreter/src/frame.rs) |
| Call continuation frame | `CallFrame<V>` | [`crates/kirin-interpreter/src/frame.rs`](../../../crates/kirin-interpreter/src/frame.rs) |
| Total frame enum | `StandardFrame<V, E>` | [`crates/kirin-interpreter/src/frame.rs`](../../../crates/kirin-interpreter/src/frame.rs) |
| Concrete driver entry | `ConcreteInterpreter::call_by_name` | [`crates/kirin-interpreter/src/concrete.rs`](../../../crates/kirin-interpreter/src/concrete.rs) |
| Concrete driver loop | `ConcreteInterpreter::run` | [`crates/kirin-interpreter/src/concrete.rs`](../../../crates/kirin-interpreter/src/concrete.rs) |
| Abstract driver entry | `AbstractInterpreter::analyze_by_name` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Abstract CFG fixpoint | `AbstractInterpreter::eval_cfg` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Abstract scope fixpoint | `AbstractInterpreter::eval_scope` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Abstract statement dispatch helper | `AbstractInterpreter::dispatch` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |

## III.1 Statement Semantics Judgment

Dialect-local rule:

`⟨s, ρ, σ⟩ ⇓_ι Result<(σ', φ), ε>`

where:

- `s` is current statement definition at `(stage, statement, env)`
- `ρ` is the active environment capability (`EnvIndex`)
- `σ` is the pre-state store view
- `σ'` is the post-state store view
- `ι` is the engine instance (materialized in Rust as `Ctx<'_, I>`)
- `φ` is `Effect<I::Value, I::Error>`
- `ε` is the interpreter error in `Result::Err`

Important correspondence note: in Rust, `Interpretable::interpret` returns
`Result<Effect<...>, ...>`; `σ'` is not returned explicitly. Instead, `σ -> σ'`
is induced by side effects through `Ctx` (`ctx.read`, `ctx.write`,
`ctx.write_results`) over the engine's `env_read`/`env_write` implementation.

API-level statement rule (canonical):

1. engine dispatches to `definition.interpret(&mut Ctx::new(...))`
2. dialect impl may mutate store via `ctx.write` / `ctx.write_results`
3. dialect impl returns `Result<Effect<...>, ...>`
4. engine consumes returned `Effect`

All statements in one run share the same interpreter type `I`; dialect code is
generic in source but monomorphic per instantiated engine.

## III.2 Closed Effect Algebra

The current control algebra is:

```rust
Effect<V, E> =
    Next
  | Jump(Edge<V>)
  | Branch(Vec<Edge<V>>)
  | Call(CallEffect<V>)
  | Yield(Product<V>)
  | Return(Product<V>)
  | Enter(Scope<V, E>)
  | EnterAny(Vec<Scope<V, E>>)
```

Interpretation ownership:

- Dialect emits `Effect`.
- Engine decides how to drive each variant.

This yields a two-layer semantics:

1. Statement layer: big-step style per statement (`⇓_ι` to one `Result`).
2. Engine layer: small-step style over continuation/worklist evolution
   (concrete frame stepping, abstract fixpoint iteration).

## III.2A Rule correspondence (formal -> API)

For each statement step, use this mapping:

- `⟨s, ρ, σ⟩ ⇓_ι Result<(σ', φ), ε>`
  - `s`: `statement.definition(stage_info).clone()`
  - `ρ`: `Ctx::env()`
  - `σ -> σ'`: induced by `ctx.write*` calls
  - `φ`: returned `Effect`
  - `ε`: returned error

Engine-level control evolution:

- concrete: `ConcreteInterpreter::run` + `Frame::step/resume`
- abstract: `AbstractInterpreter::{analyze, eval_function, eval_cfg, eval_scope}`

## III.3 Concrete Engine Semantics

Concrete execution uses explicit frame stack and deterministic stepping.

High-level loop:

1. pop top frame
2. `step(frame)` returns `FrameEffect`
3. apply effect:
   - `Continue(f)`: push `f`
   - `Push { parent, child }`: push parent then child
   - `Done`: resume parent with `resume_done`
   - `Complete(c)`: resume parent with `resume(c)`, or terminate if root

For statement-level control under concrete:

- `Next`: continue to next statement
- `Jump`: retarget current scope frame to target block with edge args
- `Call`: push call frame
- `Enter`: push child scope frame
- `Yield`: complete scope body and run hook/default completion path
- `Return`: unwind to enclosing function boundary
- `Branch` / `EnterAny`: error (`IndeterminateBranch`)

So concrete path choice must be decided by value-domain predicates.

## III.4 Abstract Engine Semantics

Abstract execution uses three engine-owned fixpoints:

1. **CFG fixpoint** inside a function body (`eval_cfg`)
2. **Scope fixpoint** for hook-driven structured loops (`eval_scope`)
3. **Interprocedural fixpoint** over function summaries (`analyze`)

Core abstract loop:

1. seed entry summary for root target
2. enqueue summary key
3. while worklist non-empty:
   - evaluate function under current entry summary
   - join/widen new information into return summary
   - re-enqueue dependent callers when summaries change

Recursion soundness note (current implementation): self-recursive same-key calls
are registered as callers too; when a recursive return summary rises, the same
summary key is re-enqueued so recursive call sites reobserve the new result.
This avoids base-case-only collapse in context-insensitive recursion.

Handling undecided effects:

- `Branch(edges)`: explore all edges, join successor entries
- `EnterAny(scopes)`: evaluate all alternatives, join results
- `RepeatOrFinish`: in scope loop, both continue and finish contribute

Reads of unbound SSA values return `bottom`.

## III.5 Multi-Dialect Dispatch Semantics

Dispatch path for any statement:

1. engine calls `info.dispatch_statement(stage, statement, env, self)`
2. stage dispatch resolves language enum variant
3. wrapped dialect impl `interpret(&mut Ctx<'_, I>)` runs
4. emitted `Effect` re-enters engine protocol

Thus mixed dialect programs (`cmp`, `arith`, `scf`, `cf`, `function`) run in one
uniform machine:

- dialect chooses local transfer meaning
- engine chooses global control mechanics

## III.6 Rule Schemas (Current)

Representative schemas:

- atomic arithmetic/cmp:
  - read operands from `ctx`
  - compute in `I::Value`
  - write result slots
  - emit `Next`
  - API anchors: `kirin-arith/src/interpreter.rs`, `kirin-cmp/src/interpreter.rs`

- CFG conditional transfer (`cf`):
  - `Some(true|false)` -> `Jump`
  - `None` -> `Branch([...])`
  - API anchor: `kirin-cf/src/interpreter.rs`

- structured conditional (`scf.if`):
  - `Some(true|false)` -> `Enter(scope)`
  - `None` -> `EnterAny([then, else])`
  - API anchor: `kirin-scf/src/interpreter.rs`

- loops (`scf.for`):
  - enter loop scope with hook
  - hook returns `Repeat`, `Finish`, or `RepeatOrFinish`
  - API anchor: `kirin-scf/src/interpreter.rs` (`ForHook::on_yield`)

## III.7 Determinism and Nondeterminism

- Concrete mode: deterministic for fixed inputs and program (aside from explicit
  host-level interruption/fuel mechanisms).
- Abstract mode: intentionally nondeterministic in control exploration order, but
  deterministic in lattice result for fixed join/widen strategy.
