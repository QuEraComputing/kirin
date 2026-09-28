# Abstract call boundary: shared root and nested preparation

Status: **scope agreed; implementation not started** (2026-09-28).

This is the first, independently implementable part of the
[unified analysis refactor](index.md). That parent plan owns the architectural
findings and final storage/work-boundary design. This subplan changes only the
sparse-forward abstract call boundary; concrete `CallFrame` stays unchanged.

Agreed scope:

```text
Root request ───┐
                ├─ AbstractCallFrame owns common call preparation
Nested request ─┘
```

- Nested: bind the current return approximation, then `Done`.
- Root: complete preparation with the selected analysis-instance handle; the
  runner drains the worklist and reads its converged result.

The current abstract public entry duplicates preparation from `summarize_call`,
while concrete `call()` already constructs a root request and runs it. Follow
that concrete construction pattern without changing `FrameEffect`.

## 1. One abstract call-boundary frame

### Request and destination

Introduce a representation-independent abstract call request, with root and nested
constructors analogous to concrete `CallRequest`. Illustrative destinations:

```text
Root
Caller { caller_environment, result_slots }
```

The request contains the callable, lookup stage, argument product, and destination.
It does not resolve the callable or select an environment during construction.
The private/configured stack composition converts it to `AbstractCallFrame`.
Keep concrete `CallRequest` and `CallFrame` unchanged; do not add abstract-only
fields or policy branches to them just to share a request name.

### One preparation path

The abstract boundary frame owns the ordering:

1. Resolve the callable and discover its body using existing services.
2. Select the analysis context key using the existing policy.
3. Get or allocate the context's environment; contribute inputs and arrange
   initial/repeated callee work through existing engine operations.
4. For a nested request, register the caller's dependency, including self-recursion,
   before consuming the callee's current result.
5. Deliver according to the destination.

Engine capabilities still provide linking, context selection, input merging,
scheduling, dependency registration, and fact access. The boundary frame owns
their sequence. Do not require `CallServices` merely to gain resolution: that
would incorrectly impose concrete `alloc_env`/`free_env` on abstract engines.
Use narrow capabilities; keep storage/merge policy inside the engine. Calls must
not yield to the worklist between preparation, dependency registration, and the
current-result read. Preserve the existing atomic call-processing guarantee.

### Result timing

Nested calls consume the current approximation, bind it into caller slots, and
finish with `Done`. They do not push a callee body or recursively drain the
worklist. Future changes cause the caller's work to be reevaluated.

A root request completes preparation with a handle identifying the selected
analysis instance. It does **not** return a supposedly converged value. The
analysis runner drains pending work, then obtains the root result through the
same result-access operation used by nested calls. The handle carries identity,
not copied authoritative facts. Its exact completion representation must fit the
existing homogeneous stack completion type; no change to `FrameEffect` is needed.

```text
analyze(callee, args)
    construct root abstract request
    run request frame → selected instance
    drain worklist using existing solver
    read selected instance's result → return

nested Call effect
    construct nested abstract request
    run same boundary frame
    bind current result; Done → resume caller
```

`analyze` remains a thin runner. It contains no independent linking, discovery,
context selection, or input-seeding logic. The shared result accessor owns
missing-return and multi-result behavior; public entry only requests final
delivery after solving. Do not add a second finalization frame unless an actual
continuation requires one.

Remove the old `summarize_call` lifecycle after migrating the frame to the required
engine operations. Merely forwarding both entry paths to a renamed monolithic
`summarize_call` helper would share code but would not establish the agreed
frame-owned boundary. Update `ForwardDataflowFrameEngine` documentation, which
currently assigns the atomic protocol to the engine.

### Incremental scope

This first change may retain the existing summary-backed merge/result operations
temporarily. It must introduce no new summary storage. Migrating those operations
to authoritative environment facts is a later change behind the same boundary.
`*Semantics` and the current work solver also remain until their separate follow-up.
This isolates call-entry behavior from storage and convergence refactoring.

The concrete and abstract boundary implementations share infrastructure and a
frame lifecycle, not necessarily one generic call-frame implementation. Concrete
waits for a callee's execution; abstract execution reads an approximation and
schedules reevaluation. These are algorithmic differences, not merely allocation
policy switches. Do not force a universal policy trait to hide them.

## 2. Validation

- Root and nested calls use the same preparation implementation, including named
  callees, resolved target stage, context selection, and body discovery.
- Context-insensitive sharing and bounded constprop contexts preserve results.
- Same-context recursion converges; a nested call never recursively drains work.
- New callees run even for empty argument products; zero/multiple-result behavior
  and missing-return approximations remain correct.
- CFG, Block, and DiGraph entries preserve behavior; unsupported bodies retain
  their existing errors. Concrete custom traversal remains unchanged.
- Failures propagate without stale active-frame/work state; existing environment
  lifetime and capability tests remain green.
- A root request's preparation completion is not exposed as a converged result.

Run focused interpreter/toy-lang/body-kind tests, then workspace tests and
doctests for the implementation. This document alone changes no runtime behavior.

## 3. Source pointers

- [Concrete entry](../../../crates/kirin-interpreter/src/engines/concrete/interp.rs)
- [Concrete call frame](../../../crates/kirin-interpreter/src/engines/concrete/frames/call_frame.rs)
- [Concrete traversal configuration](../../../crates/kirin-interpreter/src/engines/concrete/frames/protocol.rs)
- [Abstract call frame](../../../crates/kirin-interpreter/src/engines/sparse_forward/frames.rs)
- [Abstract entry, summarize_call, and seed_entry_block](../../../crates/kirin-interpreter/src/engines/sparse_forward/interp.rs)
- [Frame driver and abstract capabilities](../../../crates/kirin-interpreter/src/core/frame.rs)
- [Current solver hooks](../../../crates/kirin-interpreter/src/fixpoint/solver.rs)
