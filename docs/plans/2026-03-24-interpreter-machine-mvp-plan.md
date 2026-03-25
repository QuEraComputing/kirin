# Interpreter Machine MVP — Phased Plan Index

**Date:** 2026-03-24
**Design document:** `docs/design/2026-03-24-kirin-interpreter-machine/index.md`
**Pattern:** additive new crate + single-stage MVP first
**Primary crate:** `crates/kirin-interpreter-2`
**Status:** single-stage MVP implemented through Wave 6 checkpoint

---

## Summary

This plan builds a minimal but real single-stage concrete interpreter around
the new machine design before any dynamic-shell work begins.

The sequencing is intentional:

1. prove the machine/effect/control split in one typed shell,
2. validate the local-vs-lifted machine seam in code,
3. harden shell driver controls on that baseline runtime, and
4. only then revisit family-relative storage, dynamic staging, and new derive
   support.

`crates/kirin-derive-interpreter-2` is explicitly post-MVP.

## Goal

Build a single-stage concrete interpreter that proves:

- the `Machine<'ir>` / `ConsumeEffect<'ir>` split
- the typed `Interpreter<'ir>` shell contract
- the shell-facing `control::Shell<Stop>` model
- the basic step/run driver loop
- the machine-composition seam needed for later growth

The MVP does **not** include the full dynamic staged runtime.

## Dependency Graph

```text
wave-1 (core runtime vocabulary)
   |
wave-2 (typed single-stage shell skeleton)
   |
wave-3 (mvp execution loop)
   |
wave-4 (local vs lifted machine APIs)
   |
wave-5 (single-stage shell hardening)
   |
wave-6 (documentation + migration checkpoint)
```

## Reference Reuse, Not Migration

`crates/kirin-interpreter` is reference material, not the implementation
target.

The most relevant reuse sources for `crates/kirin-interpreter-2` are the
low-level runtime substrate pieces:

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

The pieces that should **not** be transplanted into `kirin-interpreter-2` are:

- `Continuation`
- `CallSemantics`
- `SSACFGRegion`
- dynamic stage-dispatch caches
- `Staged`
- `run_nested_calls`
- any hidden product-value writeback policy copied from the old runtime

## Testing Baseline

The initial fixture matrix should stay small and reuse shared IR fixtures:

- `CompositeLanguage`
- `build_add_one`
- `build_linear_program`
- `build_select_program`

The first helpers worth promoting to `kirin-test-utils`, if reused outside this
crate, are:

- `entry_cursor`
- `entry_block_args`
- `push_entry_frame_with_args`

Keep synthetic test-only dialects inline in `crates/kirin-interpreter-2`
until they are reused elsewhere.

## Wave Structure

### Wave 1: Core Runtime Vocabulary

**Depends on:** nothing
**Status:** implemented
**Crate(s):** `crates/kirin-interpreter-2`

**Goal:** establish the new semantic and shell primitives in code.

**Scope:**

- create the new crate skeleton
- add the core traits and types:
  - `Machine<'ir>`
  - `Interpretable<'ir, I>`
  - `ConsumeEffect<'ir>`
  - `control::Shell<Stop>`
  - `result::Stepped`
  - `result::Step`
  - `result::Run`
  - `result::Suspension`
- keep `Machine<'ir>` thin:
  - `type Effect`
  - `type Stop`
- define and document that `Interpretable::Machine` is the statement's local
  semantic machine vocabulary, not the family-selected top-level shell machine
- keep `ConsumeEffect<'ir>` separate with its own `Error`
- add `Shell::map_stop`

**Exit Criteria:**

- the new primitives compile in isolation
- the result/control types are usable without any dynamic-stage machinery
- the crate has a stable low-level module layout for later waves

### Wave 2: Typed Single-Stage Shell Skeleton

**Depends on:** Wave 1 complete
**Status:** implemented
**Crate(s):** `crates/kirin-interpreter-2`

**Goal:** implement one concrete single-stage shell over one top-level machine.

**Scope:**

- introduce a minimal `interpreter::SingleStage` for one stage/language
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

**Exit Criteria:**

- a single-stage interpreter can be constructed against `Pipeline<StageInfo<L>>`
- typed shell methods compile and can be called from tests

### Wave 3: MVP Execution Loop

**Depends on:** Wave 2 complete
**Status:** implemented
**Crate(s):** `crates/kirin-interpreter-2`

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

**Exit Criteria:**

- one statement can be interpreted and advanced through the new shell
- `run()` and `run_until_break()` exercise the same primitive pipeline
- the shell returns `result::Step` and `result::Run` as designed
- the first execution tests pass against:
  - `build_linear_program`
  - `build_add_one`
  - `build_select_program`

### Wave 4: Local vs Lifted Machine APIs

**Depends on:** Wave 3 complete
**Status:** implemented
**Crate(s):** `crates/kirin-interpreter-2`

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

**Exit Criteria:**

- local semantic tests can run against a projected submachine
- lifted effect/control flow works against a composed top-level machine
- at least one call-capable inline test dialect exercises frame/call behavior

### Wave 5: Single-Stage Concrete MVP Hardening

**Depends on:** Wave 4 complete
**Status:** implemented
**Crate(s):** `crates/kirin-interpreter-2`

**Goal:** make the single-stage shell usable as the baseline runtime.

**Scope:**

- add sibling driver-control traits:
  - `control::Fuel`
  - `control::Breakpoints`
  - `control::Interrupt`
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

**Scope reduction if needed:**

- keep fuel mandatory
- allow breakpoint and host-interrupt support to slip past the initial shell
  MVP only if the plan is updated explicitly

**Exit Criteria:**

- the single-stage shell can serve as the stable typed baseline for future
  expansion
- shell control and suspension behavior are tested independently from
  cross-stage concerns

### Wave 6: Documentation And Migration Checkpoint

**Depends on:** Wave 5 complete
**Status:** implemented
**Crate(s):** `docs/design`, `docs/plans`

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

**Exit Criteria:**

- the docs describe the implemented MVP truthfully
- the deferred dynamic/runtime work starts from a tested typed shell instead of
  a speculative abstraction

## Verification Checkpoints

After each wave:

1. `cargo test -p kirin-interpreter-2`
2. `cargo clippy -p kirin-interpreter-2 --tests -- -D warnings`

Additional checkpoints by wave:

- Wave 1: crate primitives compile and stay self-contained
- Wave 2: shell construction and stage/value access compile in tests
- Wave 3: `build_linear_program`, `build_add_one`, and `build_select_program`
  pass through the new shell
- Wave 4: local-vs-lifted composition tests pass
- Wave 5: driver-control tests cover breakpoint, fuel, and host interrupt
- Wave 6: design docs and plan record the implementation-backed truth

## Current MVP Checkpoint

The current `kirin-interpreter-2` implementation has completed all six waves
of this MVP plan.

### Current Implementation Truth

- the full public seed family exists:
  - `BlockSeed`
  - `RegionSeed`
  - `DiGraphSeed`
  - `UnGraphSeed`
- the single-stage shell uses a closed internal cursor enum over:
  - block
  - region
  - digraph
  - ungraph
- `interpreter::SingleStage` owns the driver convenience methods directly:
  - `step()`
  - `run()`
  - `run_until_break()`
- `Interpreter<'ir>` remains the primitive semantic shell trait with
  forwarding helpers, not the full provided-default driver surface described in
  the design notes
- `run_until_break()` is currently equivalent to `run()` because the MVP shell
  already stops on all suspension reasons, including breakpoints
- shell breakpoints support both:
  - `BeforeStatement`
  - `AfterStatement`
  through an internal post-step checkpoint
- the current CFG branch semantics used in the MVP tests bind target block
  arguments during `interpret(...)` before returning `Shell::Replace(...)`
  this is a working MVP choice, not a settled framework-wide rule

## Guardrails

- Keep the implementation stage-local.
- Keep the shell control language separate from semantic machine effects.
- Do not hardcode a global “machine per dialect” rule; machine choice must stay
  compatible with later family-relative interpretation modes.
- Prefer a small trait surface and a small working interpreter over premature
  dynamic-shell abstractions.
- If the implementation narrows the design, record that narrowing immediately.

## Explicit Non-Goals

- no `DynamicInterpreter`
- no `StageStore`
- no cross-stage boundary execution
- no resumable boundary protocol
- no `crates/kirin-derive-interpreter-2` work in the MVP
- no v2 derive macros in the MVP; use manual impls in tests/examples
- no attempt to preserve every detail of the old `StackInterpreter` API

## Major Risks

### Risk 1: The single-stage shell can harden the wrong trait boundary

The current implementation keeps `step()`, `run()`, and `run_until_break()` on
`interpreter::SingleStage` rather than lifting them into shared provided defaults
on `Interpreter<'ir>`.

Mitigation:

- keep this deviation documented in the checkpoint section
- do not generalize the driver trait surface until a second shell exists to
  validate the default layering

### Risk 2: CFG branch binding can fossilize in the wrong layer

The current MVP test semantics bind block arguments during `interpret(...)`
before returning `Shell::Replace(...)`.

Mitigation:

- treat this as a provisional semantic choice, not framework law
- revisit it once a second non-trivial dialect or derive-based implementation
  exercises the same boundary

### Risk 3: Dynamic-shell design can drift away from the implementation-backed core

The family-relative storage and stage-boundary sections remain design-only.

Mitigation:

- treat this plan as the completion gate for the single-stage shell only
- start dynamic-shell work from this checkpoint instead of from earlier
  speculative drafts

## Deferred After This Plan

These topics remain intentionally postponed until after the single-stage MVP:

- lifting shared driver defaults into `Interpreter<'ir>`
- `SingleStageFamily`
- `StageStore`
- `StageShellLayout`
- `DynamicInterpreter`
- stage-boundary execution
- dynamic typed stage views
- derive macros for the new runtime family
