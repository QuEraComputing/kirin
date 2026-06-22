# Rust Interpreter Formalism (Aligned to Current Kirin)

> Formal/semantic companion to the primary design doc
> [`docs/design/interpreter/index.md`](../interpreter/index.md). This formalism describes the intended operational model.

## Current Architecture Snapshot

Kirin interpreter behavior is defined by a two-persona split:

- **Dialect authors** implement statement meaning once, in `interpret(&mut Ctx<'_, I>)`.
- **Compiler authors** pick a stage enum, value domain, error, engine, and linker.

One dialect implementation serves both execution and analysis because it is
generic over `I: Interp`; the concrete/abstract distinction is carried by
`I::Value` and engine behavior.

## Formal Core

We use:

- `P`: immutable pipeline IR (`Pipeline<S>`)
- `σ`: dynamic SSA store/environment state (`EnvStackStore<V>`)
- `κ`: explicit continuation as frame stack (scope + call frames)
- `ρ`: active environment capability (`EnvIndex`)
- `ι`: instantiated interpreter/engine value (`I: Interp`)
- `φ`: emitted statement effect in `Effect<V, E>`
- `ε`: interpreter error

Machine configuration (conceptual):

`Σ = (P, σ, κ, mode)`

where `mode` is either concrete execution or abstract fixpoint evaluation.

### Shared judgment notation

All parts use the same statement-level semantic judgment:

`⟨s, ρ, σ⟩ ⇓_ι Result<(σ', φ), ε>`

where:

- `s` is the current statement definition
- `ρ` is the active `EnvIndex`
- `σ` is the pre-state store view
- `σ'` is the post-state store view
- `ι` is the concrete engine instance supplying `Ctx<'_, I>`
- `φ` is the emitted `Effect<I::Value, I::Error>`
- `ε` is an error in the `Result::Err` branch

Interpretation of this judgment against Rust API:

- `interpret` returns `Result<Effect<...>, ...>` directly.
- `σ -> σ'` is implicit, induced by `Ctx` reads/writes through the engine's
  `Interp` implementation.
- concrete and abstract engines then consume `φ` using different global drivers
  (frame stack vs fixpoint worklists).

### Canonical API-level rule schema

Statement-level rule (Rust):

`Interpretable::interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error>`

Operational reading:

- input state is carried in `ctx` (`stage`, `statement`, `env`) and engine store
- output control is `Effect`
- output store is whatever changed through `ctx.write`/`ctx.write_results`
- errors propagate through `Result::Err`

## Part Structure

- [Part I - Syntax](syntax.md)
  - SSA/stage model and dialect composition
  - operation classes and dispatch surface
- [Part II - State & Environment Model](state-environment-model.md)
  - `Interp`, `Ctx`, environment semantics, activation lifecycle
- [Part III - Operational Semantics](operational-semantics.md)
  - `Effect` algebra, concrete frame-stepping, abstract worklists
- [Part IV - Type System & Abstract Interpretation](type-system-and-abstract-interpretation.md)
  - compile-time type metadata vs runtime abstract domains
  - lattice/widening obligations and the three abstract fixpoints

## Invariants to Preserve

1. Dialect semantics are engine-blind (generic over `I: Interp`).
2. Concrete rejects undecided control (`Branch`, `EnterAny`, `RepeatOrFinish`).
3. Abstract explores undecided control and joins/widens to converge.
4. Block/call/scope arities are checked where values land.
5. Engine owns fixpoint convergence; dialects provide one-step transfer only.
