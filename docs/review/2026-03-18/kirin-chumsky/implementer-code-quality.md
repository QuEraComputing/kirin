# Implementer -- Code Quality Review: kirin-chumsky

## Clippy Workaround Audit

| Location | Allow Type | Reason | Classification | Action |
|----------|-----------|--------|---------------|--------|
| `src/tests.rs:385` | `allow(dead_code)` | `TestDialect` enum used only for `EmitContext` tests. The `Noop` variant is constructed by derive but never matched directly. | genuinely needed | Keep -- test-only code, derive generates usage |

## Logic Duplication

### 1. DiGraph and UnGraph emit_with methods share substantial structure (P2, confirmed)

**Files:** `src/ast/graphs.rs:113-197` (DiGraph) and `src/ast/graphs.rs:213-291` (UnGraph)

Both `emit_with` methods follow the same 4-5 phase pattern:
1. Collect port/capture info via `collect_port_info`
2. Build graph with ports/captures only (builder API)
3. Read back real port/capture SSAs and register them in emit context
4. Emit statements with relaxed dominance
5. Attach nodes to graph

Phases 1-3 are nearly identical between DiGraph and UnGraph (builder setup, SSA registration). The difference is only in phase 4 (UnGraph separates edge vs node statements) and phase 5 (different attach methods).

**Suggestion:** Extract phases 1-3 into a shared helper function that takes the graph builder and returns the registered SSA mappings. This would eliminate ~60 lines of duplication.

### 2. Port/capture builder loop pattern (P3, confirmed)

**File:** `src/ast/graphs.rs:137-143` and `src/ast/graphs.rs:237-243`

The builder loops for ports and captures are identical:
```rust
for (name, ty) in port_names.iter().zip(port_types.iter()) {
    builder = builder.port(ty.clone()).port_name(name.clone());
}
for (name, ty) in cap_names.iter().zip(cap_types.iter()) {
    builder = builder.capture(ty.clone()).capture_name(name.clone());
}
```
This appears twice (once for DiGraph, once for UnGraph).

### 3. `emit_with` + `EmitIR` wrapper pattern (P3, confirmed)

**File:** `src/ast/blocks.rs` and `src/ast/graphs.rs`

Every AST node (`Block`, `Region`, `DiGraph`, `UnGraph`) follows the same pattern: a public `emit_with` method that takes a closure for statement emission, plus an `EmitIR` impl that calls `emit_with` with `|stmt, ctx| stmt.emit(ctx)`. This is a deliberate design for composability but adds boilerplate. Not actionable -- the pattern enables `#[wraps]` dialect types to intercept statement emission.

## Rust Best Practices

### Missing `#[must_use]` annotations (P2, confirmed)

Zero `#[must_use]` annotations in the crate. Key candidates:
- `EmitContext::new()` -- constructor
- `EmitContext::lookup_ssa()`, `EmitContext::lookup_block()` -- pure lookups
- All `EmitIR::emit()` implementations return `Result` (already implicitly must-use via `Result`)
- Parser combinator functions -- these return parser objects that must be used

### `.clone()` in graph emit (P3, confirmed)

**File:** `src/ast/graphs.rs:138,142,238,242`

`ty.clone()` and `name.clone()` are called in loops building ports/captures. The types (`IR::Type` and `String`) are cloned because the builder takes owned values. The `name` clone could be avoided if `port_name` accepted `&str` instead of `S: Into<String>`, but this is a kirin-ir builder API concern, not a chumsky issue.

### `register_ssa` takes owned `String` (P3, confirmed)

**File:** `src/ast/graphs.rs:165,167,265,267` and `src/ast/blocks.rs:153`

`ctx.register_ssa(name.clone(), ssa)` requires cloning the name string. If `register_ssa` accepted `&str` and interned internally, these clones would be unnecessary. This is a cross-crate API concern.

## Summary

- P2 confirmed -- `src/ast/graphs.rs`: DiGraph/UnGraph emit_with share ~60 lines of identical port setup and SSA registration logic
- P2 confirmed -- Missing `#[must_use]` across the crate
- P3 confirmed -- Port/capture builder loop duplication
- P3 confirmed -- String clones in register_ssa calls due to owned-String API
