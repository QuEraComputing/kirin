# New Interpreter Design

**Status:** implemented refactor baseline

## Summary

The new interpreter design fuses the old split between call frames and cursors
into one generic frame protocol. A frame is a continuation object anchored at an
IR traversal location. Concrete execution applies frame effects to a stack.
Abstract execution can reuse the same frame protocol inside a worklist
fixpoint driver. A simple abstract interpreter can rebuild frames per summary
owner, evaluate them over abstract values, and merge candidate summaries with
join/widen/narrow policies. A more precise abstract interpreter can use a
generalized continuation store, where the concrete frame stack becomes interned
continuation entries addressed by owner, resume location, and bounded context.

The interpreter shell is generic over the total frame, completion, and error
types. The shell owns the immutable program root and the SSA environment store.
It does not understand dialect control flow. Frames return small structural
effects, and they mutate SSA state through constrained interpreter
capabilities.

The design keeps four channels explicit:

1. `Location` describes semantic-independent IR traversal positions.
2. `Frame` carries semantic traversal state.
3. `Completion` reports that a frame has completed.
4. `Error` reports protocol, IR, or dialect failures.

`Frame`, `Completion`, `Error`, and summaries use lift/project-style
composition. Failed projection for bubbling paths returns the original value.

## Table Of Contents

- [Generic Interpreter Traits](generic-interpreter-traits.md)
  - `Location`
  - `Env`
  - `Frame`
  - `FrameEffect`
  - `StatementEffect`
  - `StatementDispatch`
  - `Interpretable`
  - lift/project algebra
- [Concrete Interpreter Design](concrete-interpreter.md)
  - concrete stack shell
  - `StatementFrame`
  - `BlockFrame`
  - `RegionFrame`
  - function and call frames
- [Abstract Interpreter Framework](abstract/framework.md)
  - summary owners
  - abstract stores
  - continuation store
  - push policy
  - owner semantics
  - dependency index
- [Simple Owner-Local Fixpoint](abstract/simple-fixpoint.md)
  - abstract values
  - abstract env snapshots
  - worklist driver
  - widening/narrowing
  - abstract `StatementEffect::Transfer`
- [k-CFA Specialization](abstract/k-cfa-example.md)
  - call strings
  - function owners
  - call continuations
  - return dependencies
  - contextual address allocation
- [Forward And Backward Dataflow Shapes](abstract/dataflow.md)
  - constant propagation
  - liveness
  - direction-neutral branch transfers
  - widening/narrowing
- [Dialect Examples](dialect-examples.md)
  - dialect `Interpretable`
  - SCF frame protocol
  - language-level composition

## Goals

- Generalize cursors and call frames into one frame protocol.
- Keep the interpreter shell generic over the total frame type.
- Let dialect authors define frames and completions without changing the
  interpreter crate.
- Provide reusable standard frames for common IR traversal and call
  conventions.
- Keep SSA activation storage owned by the interpreter shell, not by dialect
  frames.
- Keep the concrete driver loop flat and deterministic.
- Let abstract drivers reuse the frame protocol inside worklist fixpoint
  algorithms.
- Keep `FrameEffect` specific to frame structure, not env mutation.
- Support both concrete interpretation and abstract interpretation without
  baking either execution strategy into the core frame trait.

## Non-Goals

- The interpreter does not mutate the IR program.
- The concrete shell is not a scheduler or worklist engine.
- Abstract interpretation does not require a public graph data structure. A
  graph is a useful mental model, but the concrete API is table-based.
- The first version does not generate composition glue. Macros can reduce
  boilerplate later.
- `Location` starts in the new interpreter crate, not `kirin-ir`. It may move
  to `kirin-ir` later if the abstraction stabilizes.

## Decision Log

### Returned Effects Vs Direct Shell Mutation

Frames return structural effects instead of mutating the concrete stack or
abstract state tables directly. This centralizes driver discipline in the shell
while still letting frames read and write SSA state through interpreter
capabilities.

Recommendation: return `FrameEffect<F, C>`.

### Consuming Frames By Value

`step` and `resume` consume `self`. This avoids borrowing the top frame and the
interpreter mutably at the same time. The driver pops the frame, steps it, and
applies the returned effect.

Recommendation: consume frames by value and return the next frame state in
`FrameEffect`.

### Immediate Resume Vs Inbox

When a child completes, the shell immediately calls the parent frame's
`resume(completion, interp)`. The parent owns any logic required to interpret
that completion and decide the next frame effect.

Recommendation: direct resume.

### Traversal Frames Bubble Unknown Completions

Traversal frames project the total completion type into their local completion
type. If projection fails, the original completion is returned through
`FrameEffect::Complete(original)`.

Recommendation: project owned completion, otherwise bubble the original.

### Standard Completion Variants

The interpreter crate provides reusable completion variants, but they are not
privileged by the shell. They compose with dialect completions through the same
lift/project algebra.

Recommendation: use `StandardCompletion` for frame-level completions, not
atomic statement completion.

### Mandatory Statement Fast Path

Atomic statements should not require an extra `StatementFrame`. `BlockFrame`
directly executes them through `StatementDispatch`. Non-atomic statements return
`StatementEffect::Push(child)`, and the child returns `FrameEffect::Done` when
the parent statement may advance.

Recommendation: mandatory fast path from the start.

### Statement Transfers

Statement-local control flow is returned as `StatementEffect::Transfer(T)`, not
as a completion. The active traversal frame owns traversal order and consumes
the transfer payload directly.

Concrete interpreters can use a jump-only transfer payload. Forward abstract
interpreters can use a branch transfer payload, using a one-edge branch for
unconditional jumps. Backward analyses can use their own backward transfer
payload.

Recommendation: specialize the transfer type per interpreter/frame family.

### Block Argument Binding

The traversal frame that enters a block binds block arguments. For `BlockFrame`,
that means the frame that consumes a transfer into a block writes target block
arguments before entering the target block.

Recommendation: block-entry frame owns argument binding.

### Root Completion

If the root frame returns `FrameEffect::Complete(c)`, interpretation is done and
the shell returns `c`.

Recommendation: root `Complete(c)` is the final result.

### Naming

Settled names:

- `Frame`
- `FrameEffect`
- `StatementEffect`
- `Completion`
- `StandardCompletion`
- `InterpreterError`
- `StatementDispatch`
- `Interpretable::interpret`
- `ProjectOrSelf`
- `HasLocation`
- `Env`
- `Callee`

### Abstract Continuation Store

Abstract interpreters use a continuation store instead of trying to preserve an
unbounded concrete frame stack. The store maps continuation addresses to
interned continuation entries. A continuation address contains the current
summary owner, resume location, and bounded context token history.

Recommendation: use `KontStore<K, Token, F>` with interned entries.

### Abstract Owner Lifecycle

Summary owners are initialized through one idempotent `ensure_owner` operation.
This operation creates the owner-specific bottom summary, installs the root
empty continuation, and initializes dependency-index bookkeeping.

Recommendation: use one `OwnerSemantics` trait for bottom summaries, entry
frames, owner completion, and summary-to-completion conversion. The driver owns
`ensure_owner`.

### Abstract Push Policy

Abstract push handling uses one policy to derive owner choice, context token,
entry summary candidate, and resume kind. This keeps k-CFA-style calls
consistent because the callee owner, call-string token, callee entry facts, and
caller resume behavior all come from the same call event.

Recommendation: use a unified `AbstractPushPolicy`.

### Summary Change Scheduling

`Summary::merge` returns a summary-specific change event. A dependency index
turns that change into work items, including owner reanalysis and waiting
continuation resumes. The driver does not hard-code self reanalysis as a
special case.

Recommendation: summary change scheduling belongs to `SummaryDependencyIndex`.

## Deferred Work

- Add derive or macro support for lift/project composition glue.
- Add derive support for language-level `Frame`, `Completion`, `Error`, and
  summary enums.
- Add `DiGraphFrame` and `UnGraphFrame` after block and region execution are
  stable.
- Move `Location` to `kirin-ir` only if it proves useful outside the
  interpreter crate.
- Decide which standard dependency-index helpers belong in the interpreter
  crate and which should stay in abstract-driver-specific modules.
- Add concrete examples for widening/narrowing strategies once the first
  abstract interpreter is implemented.
