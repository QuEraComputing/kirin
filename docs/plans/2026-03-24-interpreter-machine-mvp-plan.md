# Interpreter Machine MVP Plan

**Date:** 2026-03-24
**Status:** draft
**Primary crates:** `crates/kirin-interpreter-2`, `crates/kirin-derive-interpreter-2`

## Goal

Build a minimal but real single-stage concrete interpreter around the new
machine design.

This first implementation should prove:

- the `Machine<'ir>` / `ConsumeEffect<'ir>` split
- the typed `Interpreter<'ir>` shell contract
- the shell-facing `Control<Stop>` model
- the basic step/run driver loop
- the machine-composition seam needed for later growth

The MVP does **not** need to implement the full dynamic staged runtime.

## Why Start Here

The dynamic design now depends on family-relative storage and stage-entry
derivation.

Those pieces are easier to stabilize after we have a working typed shell with:

- one concrete cursor model
- one concrete value store
- one machine/effect/control pipeline
- real tests for local and lifted semantics

The single-stage shell is the smallest implementation that still exercises the
new machine mechanism honestly.

## Guardrails

- Keep the implementation stage-local.
- Keep the shell control language separate from semantic machine effects.
- Do not hardcode a global “machine per dialect” rule; machine choice must stay
  compatible with later family-relative interpretation modes.
- Prefer a small trait surface and a small working interpreter over premature
  dynamic-shell abstractions.
- It is acceptable to narrow some features for the MVP if the docs are updated
  to record the temporary restriction.

## Explicit Non-Goals

- no `DynamicInterpreter`
- no `StageStore`
- no cross-stage boundary execution
- no resumable boundary protocol
- no full derive-macro support for the new runtime
- no attempt to preserve every detail of the old `StackInterpreter` API

## Wave 1: Core Runtime Vocabulary

**Goal:** establish the new semantic and shell primitives in code.

**Scope:**

- add the core traits and types:
  - `Machine<'ir>`
  - `ConsumeEffect<'ir>`
  - `Control<Stop>`
  - `StepResult`
  - `StepOutcome`
  - `RunResult`
  - `SuspendReason`
- keep `Machine<'ir>` thin:
  - `type Effect`
  - `type Stop`
- keep `ConsumeEffect<'ir>` separate with its own `Error`
- add `Control::map_stop`

**Success criteria:**

- the new primitives compile in isolation
- the result/control types are usable without any dynamic-stage machinery

## Wave 2: Typed Single-Stage Shell Skeleton

**Goal:** implement one concrete single-stage shell over one top-level machine.

**Scope:**

- introduce a minimal `SingleStageInterpreter` for one stage/language
- give it:
  - pipeline reference
  - active/root `CompileStage`
  - one top-level machine
  - one value store
  - one cursor stack
  - shell driver state
- add the typed shell trait:
  - `Interpreter<'ir>`
- make the concrete single-stage shell implement:
  - `ValueStore`
  - `StageAccess<'ir>`
  - `Interpreter<'ir>`

**Constraints:**

- keep this single-stage only
- no stage switching
- stage identity still uses `CompileStage` so the shell remains compatible with
  later dynamic orchestration

**Success criteria:**

- a single-stage interpreter can be constructed against `Pipeline<StageInfo<L>>`
- typed shell methods compile and can be called from tests

## Wave 3: MVP Execution Loop

**Goal:** make the shell execute statements end-to-end.

**Scope:**

- implement:
  - `interpret_current`
  - `consume_effect`
  - `consume_control`
  - provided `run`
  - provided `run_until_break`
- add a first concrete cursor model for statement stepping
- support a minimal execution-seed surface sufficient for the MVP
- wire one fully working path:
  - current statement
  - local semantic effect
  - machine consumption
  - shell control application

**MVP simplification allowed:**

- it is acceptable to start with a narrower body/seed model than the full
  design if needed to land a working interpreter quickly
- if the MVP narrows execution seeds or cursor kinds, update the design docs to
  record the temporary restriction

**Success criteria:**

- one statement can be interpreted and advanced through the new shell
- `run()` and `run_until_break()` exercise the same primitive pipeline
- the shell returns `StepOutcome` and `RunResult` as designed

## Wave 4: Local vs Lifted Machine APIs

**Goal:** prove the machine-composition seam without dynamic staging.

**Scope:**

- implement the structural traits:
  - `ProjectMachine`
  - `ProjectMachineMut`
  - `LiftEffect`
  - `LiftStop`
- add interpreter forwarding helpers:
  - `project_machine`
  - `project_machine_mut`
  - `lift_effect`
  - `lift_stop`
  - `interpret_local`
  - `interpret_lifted`
  - `consume_local_effect`
  - `consume_lifted_effect`
  - `consume_local_control`
- test with:
  - one leaf machine
  - one simple composite machine

**Success criteria:**

- local semantic tests can run against a projected submachine
- lifted effect/control flow works against a composed top-level machine

## Wave 5: Single-Stage Concrete MVP Hardening

**Goal:** make the single-stage shell usable as the baseline runtime.

**Scope:**

- add sibling driver-control traits:
  - `FuelControl`
  - `BreakpointControl`
  - `InterruptControl`
- implement the agreed suspension policy on the single-stage shell:
  - breakpoint before fuel before host interrupt
- make `step()` available as:
  - provided default when clone bounds permit, or
  - explicit override in the concrete shell when that is cleaner
- add focused tests for:
  - final-step vs completed behavior
  - stop clears stack
  - invalid `Pop` / `Replace` handling
  - fuel semantics
  - breakpoint semantics
  - host-interrupt semantics

**Success criteria:**

- the single-stage shell can serve as the stable typed baseline for future
  expansion
- shell control and suspension behavior are tested independently from
  cross-stage concerns

## Wave 6: MVP Documentation And Migration Checkpoint

**Goal:** reconcile the design notes with what the MVP actually proved.

**Scope:**

- review the machine-design docs after the MVP lands
- explicitly record any places where the implementation intentionally narrowed
  the earlier design
- promote the single-stage shell from design-only to implementation-backed API
- log the next deferred wave entry points:
  - `SingleStageFamily`
  - `StageStore`
  - `StageShellLayout`
  - `DynamicInterpreter`
  - stage boundaries
  - new derive support

**Success criteria:**

- the docs describe the implemented MVP truthfully
- the deferred dynamic/runtime work starts from a tested typed shell instead of
  a speculative abstraction

## Recommended Order Of Attack

1. Wave 1
2. Wave 2
3. Wave 3
4. Wave 4
5. Wave 5
6. Wave 6

Do not start the dynamic shell before Wave 4 is stable. The machine-composition
surface must be proven in the single-stage shell first.

## Deferred After This Plan

These are deliberately postponed until the MVP exists:

- family-relative stage storage
- stage-entry derivation from stage enums
- dynamic shell orchestration stack
- resumable cross-stage boundaries
- dynamic typed stage views
- derive macros for the new runtime family

Those areas will almost certainly need adjustment once the MVP reveals the
parts of the design that are too abstract or too awkward in real code.
