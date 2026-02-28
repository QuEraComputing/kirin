+++
rfc = "0013"
title = "interpreter runtime kernel and scheduler abstraction"
status = "Implemented"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-28T01:27:05.904936Z"
last_updated = "2026-02-28T02:15:36.296354Z"
+++

# RFC 0013: interpreter runtime kernel and scheduler abstraction

## Summary

This RFC extracts shared interpreter runtime mechanics into reusable kernel
abstractions while preserving current fluent user APIs. It introduces a public
advanced `runtime` module in `kirin-interpreter` with `FrameStack`, scheduler
and work-executor traits, a shared
`WorkExecutor::consume_continuation` transition boundary,
a fork-handling strategy hook, and a lightweight observer interface. It keeps
`StackInterpreter` stage chains unchanged, keeps
`AbstractInterpreter::analyze(callee, stage, args)` as the dynamic stage
boundary API, and adds fluent `in_stage/with_stage` chains to
`AbstractInterpreter`.

## Motivation

- Problem: duplicated runtime logic across concrete and abstract interpreters:
  - frame-stack and value access logic is duplicated between
    `crates/kirin-interpreter/src/stack/frame.rs` and
    `crates/kirin-interpreter/src/abstract_interp/interp.rs`.
  - block argument binding logic is duplicated in
    `crates/kirin-interpreter/src/eval/block.rs`,
    `crates/kirin-interpreter/src/stack/transition.rs`, and
    `crates/kirin-interpreter/src/abstract_interp/fixpoint.rs`.
  - concrete call-driving loops are duplicated in
    `crates/kirin-interpreter/src/eval/call.rs` and
    `crates/kirin-interpreter/src/eval/block.rs`.
- Problem: downstream developers can implement new interpreters today, but they
  must copy internal runtime logic instead of reusing explicit shared tools.
- Why now: RFC 0012 established stage-dynamic boundaries and dual typed/dynamic
  APIs. Without a shared kernel, complexity and duplication will keep growing as
  we add more interpreter variants.
- Stakeholders:
  - `kirin-interpreter` maintainers
  - downstream dialect authors implementing `Interpretable`
  - downstream developers implementing custom interpreters (tracing, profiling,
    path exploration, model checking)

## Goals

- Preserve current `StackInterpreter` fluent stage-chain API shape.
- Preserve explicit dynamic stage-boundary API for abstract interpretation:
  `analyze(callee, stage, args)`.
- Add `AbstractInterpreter` fluent `in_stage/with_stage` chain APIs with
  `.analyze(...)` terminal methods.
- Introduce reusable runtime abstractions (`FrameStack`, scheduler traits,
  `WorkExecutor::consume_continuation`, fork strategy hook, observer hook)
  without changing current concrete/abstract semantics.
- Keep `eval` as semantic contracts and move execution mechanics to `runtime`
  so downstream authors have a clear API split.

## Non-goals

- Implement new scheduler families in this RFC (for example branch-spawning
  symbolic schedulers).
- Add backward-analysis direction abstractions in this RFC.
- Add residualization hooks for partial evaluation in this RFC.
- Redesign `Continuation`, `Interpretable`, or `EvalCall` semantics.

## Guide-level Explanation

Common usage remains fluent and stage-scoped:

```rust
// concrete
let v = interp.in_stage::<L>().call(callee, &args)?;

// abstract (new fluent chain)
let a = abs.in_stage::<L>().analyze(callee, &args)?;
```

Dynamic stage-boundary analysis remains explicit:

```rust
let a = abs.analyze(callee, stage_id, &args)?;
```

Advanced users can use `kirin_interpreter::runtime` to build custom
interpreter engines without copying stack/abstract internals.

## Reference-level Explanation

### API and syntax changes

Public additions in `crates/kirin-interpreter/src/lib.rs`:

- new expert module export: `pub mod runtime;`
- new abstract stage builders:
  - `AbstractInterpreter::in_stage::<L>()`
  - `AbstractInterpreter::with_stage(stage: &StageInfo<L>)`
  - `InStage::analyze(callee, args)`
  - `WithStage::analyze(callee, args)`

Public APIs intentionally preserved:

- concrete stage chains in `crates/kirin-interpreter/src/stack/stage.rs`
- concrete dynamic call boundary:
  `StackInterpreter::call(callee, stage, args)`
- abstract dynamic call boundary:
  `AbstractInterpreter::analyze(callee, stage, args)`
- current abstract summary APIs in
  `crates/kirin-interpreter/src/abstract_interp/interp.rs`

Error surface addition:

- add `InterpreterError::UnsupportedForkAction { action: &'static str }` in
  `crates/kirin-interpreter/src/error.rs` for engines that receive
  unsupported fork actions.

Module boundary decision (explicit):

- `eval` module remains the semantic contract layer:
  - `EvalCall`
  - `EvalBlock`
  - `SSACFGRegion`
- `runtime` module owns execution mechanics:
  - frame storage helpers
  - scheduler/driver logic
  - continuation-consumption helpers
  - fork strategy and observer hooks
- `eval` implementations should become thin adapters delegating mechanics to
  `runtime`.

New runtime kernel abstractions:

```rust
pub struct FrameStack<V, X> { ... }

pub trait Scheduler<W> {
    fn push(&mut self, work: W) -> bool;
    fn pop(&mut self) -> Option<W>;
    fn is_empty(&self) -> bool;
}

pub trait UniqueScheduler<W>: Scheduler<W> {
    fn push_unique(&mut self, work: W) -> bool;
}

pub trait WorkExecutor<'ir, W>: Interpreter<'ir> {
    fn execute_work(&mut self, work: &W)
        -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>;
    fn consume_continuation(
        &mut self,
        work: &W,
        cont: &Continuation<Self::Value, Self::Ext>,
    ) -> Result<(), Self::Error>;
}

pub struct BranchBatch<BW>(smallvec::SmallVec<[BW; 2]>);

pub enum ForkAction<BW> {
    Reject,
    PropagateEdges,
    Spawn(BranchBatch<BW>),
    Handled,
}

pub trait ForkStrategy<'ir, I, W>
where
    I: Interpreter<'ir>,
{
    type BranchWork;

    fn on_fork(
        &mut self,
        interp: &mut I,
        work: &W,
        branches: &[(Successor, Args<I::Value>)],
    ) -> Result<ForkAction<Self::BranchWork>, I::Error>;
}

pub trait WorkLoopRuntime<'ir, W>: WorkExecutor<'ir, W> {
    type Queue: Scheduler<W>;
    type Fork: ForkStrategy<'ir, Self, W>;
    type Obs: RuntimeObserver;

    fn queue_mut(&mut self) -> &mut Self::Queue;
    fn fork_mut(&mut self) -> &mut Self::Fork;
    fn observer_mut(&mut self) -> &mut Self::Obs;
}

pub trait RuntimeObserver {
    fn on_event(&mut self, event: &RuntimeEvent);
}

#[derive(Default)]
pub struct NoopObserver;

impl RuntimeObserver for NoopObserver {
    #[inline(always)]
    fn on_event(&mut self, _event: &RuntimeEvent) {}
}

pub struct Driver<O: RuntimeObserver = NoopObserver> {
    observer: O,
}

impl<O: RuntimeObserver> Driver<O> {
    #[inline(always)]
    fn emit(&mut self, event: &RuntimeEvent) {
        self.observer.on_event(event);
    }
}
```

Naming decision:

- use `WorkLoopRuntime` (not `RuntimeEngine`) to precisely describe this trait
  as the runtime state required by the scheduler work loop.

### Semantics and invariants

- `WorkExecutor::consume_continuation` is the canonical internal
  transition-commit boundary for one continuation.
- `FrameStack` is shared storage plumbing only; it does not change walking
  strategy semantics.
- `UniqueScheduler` exists for queues that need duplicate suppression:
  - primary use case is abstract worklist scheduling, where duplicate checks
    should be O(1) instead of scanning the queue.
  - concrete example key: `(block_id, abstract_state_fingerprint)`. multiple
    incoming edges may produce the same key; dedup prevents queue blow-up.
- Fork action semantics:
  - `Reject`: fork is illegal; return error (concrete default).
  - `PropagateEdges`: treat fork as multiple CFG edge propagations from current
    state without branch-state splitting (abstract default).
  - `Spawn(...)`: strategy emits branch work items via `BranchBatch` (small
    inline storage; heap only when branch count exceeds inline capacity).
  - `Handled`: strategy already performed fork handling; engine does nothing.
- `AbstractValue` (`join/widen/narrow`) remains value-level merge logic.
  Fork strategy is control-level fork behavior.
- Observer hooks are observational only in v1 and do not intercept or modify
  interpreter control flow.
- Observer default path is zero-overhead friendly by construction:
  - default observer type is `NoopObserver` (ZST).
  - `emit` is monomorphized and `#[inline(always)]`, allowing no-op hooks to be
    optimized away in hot paths.
- Engine defaults are fixed in this RFC:
  - concrete stack engine: `ForkAction::Reject`
  - abstract worklist engine: `ForkAction::PropagateEdges`
- If an engine receives `ForkAction::Spawn(...)` without spawn support wired, it
  must return `InterpreterError::UnsupportedForkAction { action: "spawn" }`.

Event ordering contract (v1):

1. `BeforeStep`
2. `AfterStep`
3. `BeforeConsumeContinuation`
4. state transitions (`FramePushed`/`FramePopped`, `SsaWritten`, `ForkObserved`,
   call/return events)
5. `AfterConsumeContinuation`
6. `AfterIteration`

This ordering must be deterministic for a given execution and scheduler order.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-interpreter` | add `runtime` kernel abstractions, add abstract stage-chain APIs, internal dedup migration to shared kernel | `tests/concrete_interp.rs`, `tests/abstract_fixpoint.rs`, `tests/test_dialect_coverage.rs`, new runtime unit tests |

## Drawbacks

- Introduces additional abstractions and module boundaries for maintainers.
- Public `runtime` module increases advanced API surface that must be
  documented and tested.
- Fork strategy API is broader than current needs (`Spawn(...)`) and requires
  careful docs to avoid confusion.

## Risks and Mitigations

- Risk: behavior regressions during internal dedup migration.
  - Mitigation: keep semantic entrypoints unchanged and run existing interpreter
    test suites before/after each migration slice.
- Risk: `runtime` becomes a grab-bag of mixed-level APIs.
  - Mitigation: enforce module boundary rule (`eval` contracts, `runtime`
    mechanics) and review new additions against that rule.
- Risk: observer hooks add overhead in hot loops.
  - Mitigation: default generic `NoopObserver` with monomorphized no-op calls;
    benchmark smoke tests after integration.

## Rationale and Alternatives

### Proposed approach rationale

- Keeps the current end-user API mental model intact.
- Extracts clear reusable runtime primitives for downstream custom interpreters.
- Validates abstractions against existing concrete/worklist engines before
  adding new scheduler families.

### Alternative A

- Description: keep current duplicated internals and add only docs.
- Pros: no refactor churn.
- Cons: duplicated bugs and maintenance cost continue; poor reuse story for
  downstream interpreter implementations.
- Reason not chosen: does not address the core architectural duplication.

### Alternative B

- Description: unify concrete and abstract into one monolithic generic engine.
- Pros: maximum code unification.
- Cons: high type complexity, larger migration blast radius, harder review and
  debugging.
- Reason not chosen: too disruptive for current goals.

### Alternative C

- Description: implement branch-spawning/path-exploration schedulers now.
- Pros: immediate support for more interpreter families.
- Cons: scope explosion and weaker validation of core abstractions.
- Reason not chosen: this RFC intentionally establishes kernel interfaces first.

## Prior Art

- RFC 0010 (`interpreter-framework-improvements`): merged session concepts into
  interpreter-owned execution APIs.
- RFC 0011 (`abstract-interpretation-infrastructure`): established abstract
  worklist/fixpoint model.
- RFC 0012 (`stage-dynamic-interpreter-dispatch-and-call-side-resolution`):
  established dynamic stage-boundary dispatch and stage-carrying calls.
- Common interpreter architecture pattern: shared runtime kernel + pluggable
  scheduler/driver + optional event hooks.

## Backward Compatibility and Migration

- Breaking changes: none required for current public entrypoints.
- Migration steps:
  1. No action required for existing users.
  2. Optional adoption of new abstract stage chains:
     `in_stage::<L>().analyze(...)` / `with_stage(...).analyze(...)`.
  3. Optional adoption of `runtime` helpers for custom interpreter
     implementations.
- Compatibility strategy:
  - keep existing stack/abstract dynamic entrypoints.
  - add abstractions and fluent APIs incrementally.

Compatibility commitment in this RFC:

- `runtime` is public and intended for downstream reuse, but this RFC does not
  define a strict long-term stability policy beyond normal project evolution.
- Any future hardening/stability tiering for `runtime` should be covered by a
  separate RFC.

## How to Teach This

- Teach two layers:
  - standard: fluent stage APIs for concrete and abstract interpreters.
  - advanced: `runtime` module for interpreter authors.
- Add a small custom-interpreter skeleton example demonstrating:
  - scheduler + executor wiring
  - fork strategy defaulting
  - observer event collection
- Document `ForkAction` with side-by-side examples for `Reject` and
  `PropagateEdges`.

## Reference Implementation Plan

1. Add `runtime` module skeleton and tests:
   - `frame_stack.rs`
   - `scheduler.rs`
   - `driver.rs`
   - `fork.rs`
   - `observer.rs`
   - shared args/stage helper modules
2. Migrate stack and abstract frame plumbing to `FrameStack`.
3. Extract shared block arg binding and SSA arg-read helpers.
4. Introduce scheduler and executor traits; adapt current concrete and abstract
   loops to shared driver paths where appropriate.
5. Add `UniqueScheduler` and migrate abstract queue dedup to `push_unique`.
6. Add fork strategy hook returning `ForkAction` and default strategy
   implementations:
   - concrete -> `Reject`
   - abstract -> `PropagateEdges`
   - unsupported spawn path -> `InterpreterError::UnsupportedForkAction`
7. Add observer hook emission points and default generic `NoopObserver`.
8. Add abstract `in_stage/with_stage` builders with `.analyze(...)` methods.
9. Add runtime contract-test helpers and run interpreter/workspace tests.

Reusable runtime contract-test helpers:

- `assert_scheduler_progress(...)`:
  input: seeded scheduler, max step budget, expected terminal condition.
  verifies scheduler + loop driver make forward progress and terminate.
- `assert_unique_scheduler_dedup(...)`:
  input: duplicate-heavy work stream + expected unique cardinality.
  verifies `push_unique` enqueues each work item once.
- `assert_consume_continuation_contract(...)`:
  input: table-driven continuation cases (`Continue/Jump/Call/Return`) and
  expected queue/frame/value effects.
  verifies `consume_continuation` behavior for `Continue/Jump/Call/Return`.
- `assert_fork_action_contract(...)`:
  input: each `ForkAction` variant and a mock strategy.
  verifies `Reject`, `PropagateEdges`, `Handled`, and unsupported `Spawn`.
- `assert_event_ordering(...)`:
  input: emitted runtime event trace + expected sequence grammar.
  verifies strict v1 event ordering contract.
- `assert_frame_stack_invariants(...)`:
  input: scripted push/pop/use sequence.
  verifies push/pop/current/depth/max-depth and active-stage fallback.

Rollout rule:

- Land each slice as behavior-preserving PRs with green tests before moving to
  the next slice.

### Acceptance Criteria

- [x] Existing `StackInterpreter` tests pass unchanged.
- [x] Existing abstract dynamic stage-boundary tests pass unchanged via
      `analyze(callee, stage, args)`.
- [x] New abstract stage-chain APIs are implemented and tested.
- [x] Duplicate frame/worklist runtime mechanics are replaced by shared runtime
      primitives (`FrameStack`, `UniqueScheduler`) while block-argument binding
      remains on the `EvalBlock` contract default.
- [x] `UniqueScheduler` is used for abstract queue deduplication (no O(n)
      queue scan in the core loop).
- [x] Fork strategy defaults preserve current concrete and abstract semantics.
- [x] Unsupported spawn action returns explicit
      `InterpreterError::UnsupportedForkAction`.
- [x] Observer hook smoke tests validate expected event emission order.
- [x] `eval` remains contract-focused while `runtime` owns shared execution
      mechanics and extension points.
- [x] Runtime contract-test helpers validate scheduler/continuation/fork/event/
      frame invariants.
- [x] `cargo test -p kirin-interpreter` and `cargo test --workspace` pass.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - branch-spawning scheduler based on `ForkAction::Spawn(...)`
  - backward-analysis direction abstraction (separate RFC)
  - residualization hooks for partial evaluation (separate RFC)

## Unresolved Questions

- Should observer payloads remain lightweight and object-safe only, or include
  optional typed payload extensions in a follow-up?
- Should `UniqueScheduler` remain a separate capability trait, or should
  deduplication become mandatory for all scheduler implementations?
- Should `runtime` expose default scheduler container implementations in v1, or
  only traits and helper drivers?

## Checklist Status

`rfc-checklist.md` review outcome:

- [x] State the problem and motivation concretely.
- [x] Define clear goals and non-goals.
- [x] Describe current behavior with exact file references.
- [x] Describe proposed behavior with enough detail to implement.
- [x] Identify affected crates and likely touch points.
- [x] Include at least two alternatives with trade-offs.
- [x] Explain backward compatibility and migration impact.
- [x] Define test and validation work per affected crate.
- [x] List key risks and mitigations.
- [x] End with explicit open questions or decision points.
- [x] Keep terminology consistent with Kirin docs and code.

## Future Possibilities

- Add path-exploration interpreters using `ForkAction::Spawn(...)`.
- Add backward-analysis direction support once a backward engine RFC lands.
- Add residualization hooks for partial evaluation/specialization workflows.
- Extend observer hooks to support debugger-grade pause/intercept semantics.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-28T01:27:05.904936Z | RFC created from template |
| 2026-02-28 | Replaced template with full runtime-kernel proposal |
| 2026-02-28 | Clarified eval/runtime boundary, risks, rollout, and checklist status |
| 2026-02-28 | Refined scheduler and fork model: `UniqueScheduler`, `ForkAction`, `WorkLoopRuntime`, observer zero-overhead pattern, and runtime contract-test helper list |
| 2026-02-28 | Tightened type contracts (`WorkLoopRuntime: Interpreter`), switched `Spawn` payload to `BranchBatch`, clarified unsupported fork error payload, extended event ordering to six points, and expanded runtime test-helper specs |
| 2026-02-28 | Implemented runtime module and interpreter migrations (`FrameStack`, scheduler dedup, abstract stage-chain APIs, observer/fork contracts), validated with `cargo test -p kirin-interpreter` and `cargo test --workspace` |
