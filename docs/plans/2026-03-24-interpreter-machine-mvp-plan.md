# Interpreter Machine MVP Plan

**Date:** 2026-03-24
**Status:** implementation-backed through Wave 5
**Primary crate:** `crates/kirin-interpreter-2`

`crates/kirin-derive-interpreter-2` is explicitly post-MVP.

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

## MVP Checkpoint

The current `kirin-interpreter-2` implementation has completed the first five
waves of this plan:

- core machine/control/result vocabulary
- a typed single-stage shell
- an end-to-end single-stage execution loop
- local vs lifted machine helpers
- shell-owned fuel, breakpoint, and interrupt controls

The MVP has proven the single-stage machine mechanism in code, but it has also
made a few intentional narrowing decisions that should be treated as current
truth until a later expansion pass changes them.

### Current Implementation Truth

- `kirin-interpreter-2` now defines the full public seed family:
  - `BlockSeed`
  - `RegionSeed`
  - `DiGraphSeed`
  - `UnGraphSeed`
- the single-stage shell now uses a closed internal cursor enum over:
  - block
  - region
  - digraph
  - ungraph
- `SingleStageInterpreter` owns the driver convenience methods directly:
  - `step()`
  - `run()`
  - `run_until_break()`
- `Interpreter<'ir>` currently remains the primitive semantic shell trait with
  forwarding helpers, not the full provided-default driver surface described in
  the design notes.
- `run_until_break()` is currently equivalent to `run()` because the MVP shell
  already stops on all suspension reasons, including breakpoints.
- shell breakpoints support both:
  - `BeforeStatement`
  - `AfterStatement`
  through an internal post-step checkpoint in the single-stage shell.
- the current CFG branch semantics used in the MVP tests bind target block
  arguments during `interpret(...)` before returning `Control::Replace(...)`.
  This is a working MVP choice, not a settled framework-wide rule.

### Still Deferred After The MVP

- lifting `step()` / `run()` / `run_until_break()` into `Interpreter<'ir>` as
  shared provided defaults
- revisiting whether block-argument binding belongs in statement interpretation
  or in a higher-level helper boundary
- `SingleStageFamily`
- `StageStore`
- `StageShellLayout`
- `DynamicInterpreter`
- stage-boundary execution
- `crates/kirin-derive-interpreter-2`

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

## Reference Reuse, Not Migration

`crates/kirin-interpreter` is reference material, not the implementation target.

The best reuse sources for `crates/kirin-interpreter-2` are the low-level
runtime substrate pieces:

- `value_store.rs`
  minimal SSA read/write contract
- `frame.rs`
  per-call frame ownership shape
- `frame_stack.rs`
  top-frame SSA scoping, push/pop/current, max-depth handling
- `stage_access.rs`
  pipeline + active-stage ownership idea, in a much smaller single-stage form
- `block_eval.rs`
  `bind_block_args`
- `stack/transition.rs`
  concrete cursor advancement and fuel spending ideas
- `stack/call.rs`
  callee-entry sequencing only
- `error.rs`
  baseline runtime error taxonomy

The pieces that should **not** be transplanted into `kirin-interpreter-2`
are:

- `Continuation`
- `CallSemantics`
- `SSACFGRegion`
- dynamic stage-dispatch caches
- `Staged`
- `run_nested_calls`
- any hidden product-value writeback policy copied from the old runtime

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
- no `crates/kirin-derive-interpreter-2` work in the MVP
- no v2 derive macros in the MVP; use manual impls in tests/examples
- no attempt to preserve every detail of the old `StackInterpreter` API

## Testing Baseline

The first test matrix should stay small and should reuse existing shared IR
fixtures rather than inventing a new dialect universe.

Preferred baseline:

- `CompositeLanguage`
- `build_add_one`
- `build_linear_program`
- `build_select_program`

The first helpers worth adding to `kirin-test-utils`, if two crates need them,
are:

- `entry_cursor`
- `entry_block_args`
- `push_entry_frame_with_args`

Keep synthetic test-only dialects inline in `crates/kirin-interpreter-2` until
they are reused elsewhere.

## Wave 1: Core Runtime Vocabulary

**Goal:** establish the new semantic and shell primitives in code.

**Scope:**

- create the new crate skeleton for `crates/kirin-interpreter-2`
- add the core traits and types:
  - `Machine<'ir>`
  - `Interpretable<'ir, I>`
  - `ConsumeEffect<'ir>`
  - `Control<Stop>`
  - `StepResult`
  - `StepOutcome`
  - `RunResult`
  - `SuspendReason`
- keep `Machine<'ir>` thin:
  - `type Effect`
  - `type Stop`
- define and document that `Interpretable::Machine` is the statement's local
  semantic machine vocabulary, not the family-selected top-level shell machine
- keep `ConsumeEffect<'ir>` separate with its own `Error`
- add `Control::map_stop`

**Success criteria:**

- the new primitives compile in isolation
- the result/control types are usable without any dynamic-stage machinery
- the crate has a stable low-level module layout for later waves

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
- no public stage-switch API on the MVP shell
- keep `StageAccess<'ir>` minimal: fixed stage identity, typed stage view, and
  only the resolution needed by the single-stage shell
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
- add one concrete cursor kind for statement stepping
- support one minimal execution-seed kind sufficient for the MVP
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
- the first execution tests pass against:
  - `build_linear_program`
  - `build_add_one`
  - `build_select_program`

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
- keep these manual; still no v2 derives in this wave

**Success criteria:**

- local semantic tests can run against a projected submachine
- lifted effect/control flow works against a composed top-level machine
- at least one call-capable inline test dialect exercises frame/call behavior

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

**If scope must narrow further:**

- keep fuel mandatory
- allow breakpoint and host-interrupt support to slip past the initial shell
  MVP as long as the plan is updated explicitly

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
- explicitly revisit leaf-machine binding and derive design only after the
  single-stage shell API is implementation-backed

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
