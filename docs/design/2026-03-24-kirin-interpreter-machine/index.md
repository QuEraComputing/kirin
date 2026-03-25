# Kirin Interpreter Machine Design

**Date:** 2026-03-24
**Status:** alternative design direction
**Primary crates:** `crates/kirin-interpreter-2`, `crates/kirin-derive-interpreter-2`

## Summary

This design takes a different direction from the earlier
`2026-03-23-kirin-interpreter-refactor` docs.

The core idea is:

- the interpreter framework is a machine for executing statement-level
  operational semantics
- dialects own their semantic machine types, semantic effect types, and
  semantic stop payloads
- the framework owns cursor-stack management, driver loops, breakpoints, fuel,
  and the shell control language
- `Block`, `Region`, `DiGraph`, and `UnGraph` do not carry inherent
  operational meaning in the framework
- statements decide how to execute nested bodies, using framework helpers or
  dialect-specific logic

This folder is intentionally additive. The earlier design docs remain unchanged.

## Key Decisions

- `Interpretable` is the primary semantic trait.
- `Machine<'ir>` owns associated `Effect` and `Stop` types.
- effects are consumed by a separate `ConsumeEffect<'ir>` trait on machine
  types.
- `Interpreter<'ir>` is the typed shell trait over one top-level machine.
- `ConsumeEffect` mutates machine-owned semantic state and returns shell-facing
  `Control<Stop>`.
- the shell manages an internal cursor stack and consumes `Control`.
- the framework defines public execution seeds, but keeps full cursors internal.
- there are two interpreter shells:
  - `SingleStageInterpreter<L>`
  - `DynamicInterpreter`
- `SingleStageInterpreter<L>` exposes typed value/effect/machine APIs.
- `DynamicInterpreter` orchestrates a heterogeneous set of single-stage
  interpreters and does not expose raw typed effect/value APIs directly.
- machine and shell selection are family-relative:
  - the stage enum says which dialect lives at each stage
  - the chosen single-stage interpreter family maps that dialect to a shell and
    machine type
- typed shells expose both local and lifted interpret/consume APIs.
- driver APIs use `StepOutcome` and `RunResult`.
- stage switching is a public capability on both shells:
  - `SingleStageInterpreter<L>` errors when switching is requested
  - `DynamicInterpreter` executes stage switches through stage-boundary
    protocols

## Document Map

- [machine.md](machine.md)
  Machine responsibilities, public traits, seeds, actions, and step lifecycle.
- [interpreter-shells.md](interpreter-shells.md)
  `SingleStageInterpreter<L>` and `DynamicInterpreter`.
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
- statement operational semantics become the semantic authority
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
