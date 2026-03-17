# Graph Body Pretty Printing Design

Extends kirin-prettyless to print `digraph` and `ungraph` bodies following the text format defined in `docs/design/graph-ir-node.md`.

## Ordering Strategy

**The printer is simple — the builder does the work.**

Both `DiGraphBuilder` and `UnGraphBuilder` reindex the petgraph at `.new()` time so that node indices are already in the correct print order. The printer just iterates `graph.node_references()` in index order.

## Printer Design

Three new methods on `Document<'a, L>` in `ir_render.rs`.

### `print_ports`

Shared helper for the port+capture header.

**Signature:** `fn print_ports(&'a self, ports: &[Port], edge_count: usize) -> ArenaDoc<'a>`

**Output:** `(%p0: Type, %p1: Type) capture(%theta: f64, %phi: f64)`

- Edge ports `ports[..edge_count]` printed as `(%name: Type, ...)` — same format as block arguments (lookup `SSAInfo` via `port.expect_info(stage)`)
- If `ports[edge_count..]` is non-empty, append ` capture(%name: Type, ...)`
- If both empty, produce empty output

### `print_digraph`

**Signature:** `fn print_digraph(&'a self, digraph: &DiGraph) -> ArenaDoc<'a>`

```
digraph ^dg0(%q0: Qubit, %q1: Qubit) capture(%theta: f64) {
  %0 = hadamard %q0 -> Qubit;
  %1, %2 = cnot %0, %q1 -> (Qubit, Qubit);
  yield %1, %2;
}
```

**Logic:**
1. Print `digraph` keyword
2. Print name from `DiGraphInfo.name` via symbol table, fallback to `DiGraph::Display`
3. Print ports via `print_ports(ports, edge_count)`
4. Open `{`
5. Iterate `graph.node_references()` in index order — `print_statement` + `;` for each
6. Print `yield` with comma-separated SSAValue names + `;`
7. Close `}`

### `print_ungraph`

**Signature:** `fn print_ungraph(&'a self, ungraph: &UnGraph) -> ArenaDoc<'a>`

```
ungraph ^ug0(%p0: Wire) capture(%zero: f64, %pi: f64) {
  edge %w0 = wire -> Wire;
  edge %w1 = wire -> Wire;
  z_spider(%zero, %p0, %w0, %w1);
  edge %w2 = wire -> Wire;
  x_spider(%pi, %w0, %w2);
}
```

**Logic:**
1. Print `ungraph` keyword
2. Print name from `UnGraphInfo.name` via symbol table, fallback to `UnGraph::Display`
3. Print ports via `print_ports(ports, edge_count)`
4. Open `{`
5. Interleave edge statements and node statements by iterating both in order:
   - Track an `edge_cursor` into `edge_statements`
   - For each node in `graph.node_references()` index order, print all edge statements up to the ones this node needs (advance `edge_cursor`), then print the node
6. Print any remaining edge statements
7. Close `}`

Since the builder already ordered both `edge_statements` and nodes so that edges appear before their consuming nodes, the printer just advances two cursors in lockstep. No BFS or sorting in the printer.

## Derive Dispatch

In `crates/kirin-derive-chumsky/src/field_kind.rs`, the `print_expr` function:

```rust
FieldCategory::DiGraph => quote! { doc.print_digraph(#field_ref) },
FieldCategory::UnGraph => quote! { doc.print_ungraph(#field_ref) },
```

Parser-side `unimplemented!()`s remain — parser support is a separate future task.

## Files Changed

| Action | File | Change |
|--------|------|--------|
| Modify | `crates/kirin-prettyless/src/document/ir_render.rs` | Add `print_digraph`, `print_ungraph`, `print_ports` |
| Modify | `crates/kirin-derive-chumsky/src/field_kind.rs` | Replace `unimplemented!()` in `print_expr` |
