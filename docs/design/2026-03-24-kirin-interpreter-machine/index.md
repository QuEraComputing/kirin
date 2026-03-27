# Kirin Interpreter Machine Design

**Date:** 2026-03-24
**Status:** alternative design direction with single-stage MVP implemented
**Primary crates:** `crates/kirin-interpreter-2`, `crates/kirin-derive-interpreter-2`

## Summary

This design takes a different direction from the earlier
`2026-03-23-kirin-interpreter-refactor` docs.

The core idea is:

- the interpreter framework is a machine for executing statement-level
  operational semantics
- dialects own their semantic machine types, semantic effect types, and
  semantic stop payloads
- the framework owns activation-stack management for same-stage execution,
  per-frame value environments, cursor-stack management, driver loops,
  breakpoints, fuel, and the shell control language
- `Block`, `Region`, `DiGraph`, and `UnGraph` do not carry inherent
  operational meaning in the framework
- statements decide how to execute nested bodies, using framework helpers or
  dialect-specific logic

This folder is intentionally additive. The earlier design docs remain unchanged.

## MVP Checkpoint

`crates/kirin-interpreter-2` now backs the single-stage portion of this design
with a real implementation.

The implemented part currently covers:

- `Machine<'ir>` / `ConsumeEffect<'ir>`
- the primitive `Interpreter<'ir>` shell contract
- `interpreter::SingleStage`
- `interpreter::Position<'ir>`
- `interpreter::Driver<'ir>`
- a closed internal cursor stack over block/region/digraph/ungraph execution
- local vs lifted machine helpers
- `control::Shell<Stop>`, `result::Step`, and `result::Run`
- shell-owned fuel, breakpoint, and interrupt controls

The dynamic-shell, family-relative storage, and stage-boundary sections in this
design folder remain design-only and intentionally deferred.

## Key Decisions

- `Interpretable` is the primary semantic trait.
- `Machine<'ir>` owns associated `Effect` and `Stop` types.
- effects are consumed by a separate `ConsumeEffect<'ir>` trait on machine
  types.
- `Interpreter<'ir>` is the semantic typed shell trait over one top-level
  machine.
- `interpreter::Position<'ir>` is the read-only execution-position trait for
  typed shells and typed stage views.
- `interpreter::Driver<'ir>` layers `step`, `run`, and `run_until_break` over
  shells and typed stage views that expose position plus driver-control state.
- `ConsumeEffect` mutates machine-owned semantic state and returns shell-facing
  `control::Shell<Stop>`.
- `interpreter::SingleStage<L>` owns the activation stack for same-stage
  execution; the top activation frame is the current invocation.
- `ValueStore`, `TypedStage`, `Position`, and driver stepping project over the
  top activation frame.
- the shell manages per-frame cursor stacks and consumes `Shell`.
- the framework defines public execution seeds, but keeps full cursors internal.
- there are two interpreter shells:
  - `interpreter::SingleStage<L>`
  - `DynamicInterpreter`
- `interpreter::SingleStage<L>` exposes typed value/effect/machine APIs.
- `Function`, `StagedFunction`, and `SpecializedFunction` own function identity
  and specialization structure; call-like statements own dispatch policy, while
  the shell owns specialized-function invocation and return/resume mechanics.
- `DynamicInterpreter` orchestrates a heterogeneous set of single-stage
  interpreters and does not expose raw typed effect/value APIs directly.
- machine and shell selection are family-relative:
  - the stage enum says which dialect lives at each stage
  - the chosen single-stage interpreter family maps that dialect to a shell and
    machine type
- typed shells expose both local and lifted interpret/consume APIs.
- typed shells and typed stage views share the same `Interpreter<'ir>` core,
  with `Position<'ir>` and `Driver<'ir>` layered on top when available.
- downstream `Interpretable` authors stay bound only to `Interpreter<'ir>`;
  they do not need `Driver<'ir>` or `Position<'ir>` for ordinary dialect
  semantics.
- driver APIs use `result::Step` and `result::Run`.
- stage switching is a public capability on both shells:
  - `interpreter::SingleStage<L>` errors when switching is requested
  - `DynamicInterpreter` executes stage switches through stage-boundary
    protocols

## Document Map

- [machine.md](machine.md)
  Machine responsibilities, public traits, seeds, actions, and step lifecycle.
- [interpreter-shells.md](interpreter-shells.md)
  `interpreter::SingleStage<L>` and `DynamicInterpreter`.
- [state-and-effects.md](state-and-effects.md)
  Dialect-defined state, effect composition, projection traits, and testing
  implications.
- [derive-and-tooling.md](derive-and-tooling.md)
  Derive surface for `Interpretable` and machine composition traits, with user
  examples.
- [stage-boundaries.md](stage-boundaries.md)
  Stage switching, boundary protocols, and cross-stage execution.

## Relation To Earlier Docs

The earlier refactor design centered the runtime around body-shape executors
such as `ExecBlock` and `ExecRegion`. This design replaces that framing with a
statement-centric interpreter machine.

The practical consequence is:

- body execution helpers become optional reusable defaults
- statement operational semantics become the semantic authority for dispatch
  policy and control effects
- single-stage shells own generic activation and invocation mechanics for
  `SpecializedFunction`
- framework-owned traits are reduced to machine composition traits and shell
  APIs

This direction is a better fit for:

- dialect-owned call/yield/return conventions
- dialect-owned graph traversal state
- staged programs with heterogeneous values/states/effects across stages
- small dialect-local operational-semantics tests

## Deferred After MVP

The next implementation phase should stop at a single-stage concrete
interpreter MVP.

The following topics remain intentionally deferred until that typed shell and
machine mechanism have been proven in code:

- the exact `StageStore` trait surface
- the `SingleStageFamily` and `StageShellLayout` derive details
- the first concrete `DynamicInterpreter`
- stage-boundary adapter registry and resolution
- machine-side and interpreter-side derive macro expansion for the new runtime
