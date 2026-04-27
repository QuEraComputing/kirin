# New Interpreter Design

**Status:** design draft

## Summary

The new interpreter design fuses the old split between call frames and cursors
into one generic frame protocol. A frame is a continuation object anchored at an
IR traversal location. Concrete execution applies frame effects to a stack.
Abstract execution applies the same frame effects to finite frame tables,
summary tables, driver-specific dependency indexes, and a worklist.

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
- [Abstract Interpreter Generic Traits](abstract/generic-traits.md)
  - `AbstractState`
  - dependency indexes
  - generalized abstract addresses
  - summaries
  - widening/narrowing
- [k-CFA Example](abstract/k-cfa-example.md)
  - call strings
  - call/function nodes
  - `KCfaDeps`
  - generalized frame-node addresses
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
- Let abstract drivers interpret frame effects as table, dependency-index, and
  worklist updates.
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

Recommendation: use `StandardCompletion`.

### Mandatory Statement Fast Path

Atomic statements should not require an extra `StatementFrame`. `BlockFrame`
directly executes them through `StatementDispatch`. Non-atomic statements return
`StatementEffect::Push(child)`, and the block pushes a `StatementFrame` with
that pending child.

Recommendation: mandatory fast path from the start.

### Statement Transfers

Statement-local control flow is returned as `StatementEffect::Transfer(T)`, not
as a completion. The active traversal frame owns traversal order and consumes
the transfer payload directly.

Concrete interpreters can use `ConcreteTransfer<V>` with only `Jump`. Forward
abstract interpreters can use `ForwardTransfer<V>` with only `Branch`, using a
one-edge branch for unconditional jumps. Backward analyses can use their own
backward transfer payload.

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

## Deferred Work

- Add derive or macro support for lift/project composition glue.
- Add derive support for language-level `Frame`, `Completion`, `Error`, and
  summary enums.
- Add `DiGraphFrame` and `UnGraphFrame` after block and region execution are
  stable.
- Move `Location` to `kirin-ir` only if it proves useful outside the
  interpreter crate.
- Decide whether dependency-index helpers belong in the interpreter crate or
  stay in abstract-driver-specific modules.
- Add concrete examples for widening/narrowing strategies once the first
  abstract interpreter is implemented.
