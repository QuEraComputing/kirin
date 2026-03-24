# Graph Visitation and Computational-Graph Validation

**Wave:** 3
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

The design's key differentiator is first-class graph execution shapes. This
wave validates that claim by landing:

- `VisitDiGraph<'ir>` and `VisitUnGraph<'ir>` runtime hooks,
- `DiGraphCursor` and `UnGraphCursor` stepping support,
- graph-boundary result consumption through `ConsumeResult`,
- a concrete DiGraph execution test that proves the new API can execute a
  computational graph and produce the expected outward result.

The goal is not to invent one universal graph scheduler. The goal is to prove
the framework can host graph traversal cleanly without collapsing graphs back
into block semantics.

## Scope

**Files to add or modify:**

- `crates/kirin-interpreter-2/src/traits/visit_digraph.rs`
- `crates/kirin-interpreter-2/src/traits/visit_ungraph.rs`
- `crates/kirin-interpreter-2/src/stack/digraph.rs`
- `crates/kirin-interpreter-2/src/stack/ungraph.rs`
- `crates/kirin-interpreter-2/src/stack/cursor.rs`
- `crates/kirin-interpreter-2/tests/digraph_exec.rs`
- `crates/kirin-interpreter-2/tests/graph_breakpoints.rs`
- shared test helpers in `kirin-test-utils` or `kirin-test-languages` only if
  the toy graph fixtures are reused outside this crate

**Out of scope:**

- forcing one scheduler on all graph dialects,
- broad UnGraph semantics beyond visitation-state support,
- workspace-wide dialect migration.

## Required Test Case

Add a small toy language that defines a computational-graph statement with a
`DiGraph` body. The test must assert that its outward result matches a
reference execution of the equivalent plain block computation.

This comparison is output-level only. It does **not** require implementing a
second full block-based version of the toy language.

## Implementation Steps

- [ ] Implement the public visitation traits and the internal cursor/state
  plumbing needed to step through graph statements.
- [ ] Keep `ExecutionLocation` statement-based even when the active cursor is a
  graph cursor.
- [ ] Add a minimal scheduler interface or internal policy hook for choosing the
  next graph visit without exposing a public generic-machine API.
- [ ] Land the DiGraph toy-language test:
  build a graph-bodied statement,
  execute it through `kirin-interpreter-2`,
  compare its outward result to the block reference computation.
- [ ] Add breakpoint coverage for graph execution to prove that graph stepping
  still reports statement locations.
- [ ] Add at least a smoke-level UnGraph test proving the runtime can host
  `UnGraphCursor` state even if no rich dialect semantics ship in this wave.

## Validation

Run:

```bash
cargo nextest run -p kirin-interpreter-2 -E 'test(digraph_exec|graph_breakpoints)'
cargo nextest run -p kirin-interpreter-2
```

## Success Criteria

1. `DiGraph` execution is no longer only a design promise; it is validated by a
   working computational-graph test.
2. Graph execution uses statement-based breakpoint locations.
3. The graph API remains scheduler-flexible instead of baking in one universal
   policy.
4. `UnGraph` is represented in the runtime surface even if its first concrete
   semantics stay intentionally small.
