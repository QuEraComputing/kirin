# Graph Body Pretty Printing Design

Extends kirin-prettyless to print `digraph` and `ungraph` bodies following the text format defined in `docs/design/graph-ir-node.md`.

## New Methods on `Document<'a, L>`

Three new methods in `ir_render.rs`, parallel to `print_block`/`print_region`.

### `print_ports`

Shared helper for the port+capture header used by both graph kinds.

**Signature:** `fn print_ports(&'a self, ports: &[Port], edge_count: usize) -> ArenaDoc<'a>`

**Output format:** `(%p0: Type, %p1: Type) capture(%theta: f64, %phi: f64)`

- Edge ports `ports[..edge_count]` printed as `(%name: Type, ...)` — same format as block arguments (lookup `SSAInfo` via `port.expect_info(stage)` for name and type)
- If `ports[edge_count..]` is non-empty, append ` capture(%name: Type, ...)`
- If both empty, produce empty output (no parens)

### `print_digraph`

**Signature:** `fn print_digraph(&'a self, digraph: &DiGraph) -> ArenaDoc<'a>`

**Output format:**
```
digraph ^dg0(%q0: Qubit, %q1: Qubit) capture(%theta: f64) {
  %0 = hadamard %q0 -> Qubit;
  %1, %2 = cnot %0, %q1 -> (Qubit, Qubit);
  yield %1, %2;
}
```

**Logic:**
1. Print keyword `digraph`
2. Print name from `DiGraphInfo.name` via symbol table, fallback to `DiGraph::Display` (`^dg{id}`)
3. Print ports via `print_ports(ports, edge_count)`
4. Open `{`
5. Iterate `graph.node_references()` in index order — for each node weight (`Statement` ID), call `print_statement` + `;`
6. Print `yield` with the yields vec as comma-separated SSAValue names + `;`
7. Close `}`

`yield` is not a real statement — it's metadata from `DiGraphInfo.yields`, formatted inline after all node statements.

### `print_ungraph`

**Signature:** `fn print_ungraph(&'a self, ungraph: &UnGraph) -> ArenaDoc<'a>`

**Output format:**
```
ungraph ^ug0(%p0: Wire, %p1: Wire) capture(%zero: f64) {
  edge %w0 = wire -> Wire;
  edge %w1 = wire -> Wire;
  z_spider(%zero, %p0, %w0, %w1);
  x_spider(%pi, %w0, %w2);
}
```

**Logic:**
1. Print keyword `ungraph`
2. Print name from `UnGraphInfo.name` via symbol table, fallback to `UnGraph::Display` (`^ug{id}`)
3. Print ports via `print_ports(ports, edge_count)`
4. Open `{`
5. Print `edge_statements` first — prepend `edge ` prefix to each, then `print_statement` + `;`. The vec membership is the discriminant (no `is_edge()` check needed).
6. Iterate `graph.node_references()` in index order for node statements — `print_statement` + `;`
7. No `yield` (ungraph has no yields)
8. Close `}`

## Statement Ordering

Petgraph's `node_references()` iterates in node index (insertion) order. The printer does not sort. The builder is responsible for inserting nodes in a sensible order (topological for DAGs, text order for cyclic graphs). This follows the same approach as petgraph's own DOT printer.

## Derive Dispatch

In `crates/kirin-derive-chumsky/src/field_kind.rs`, the `print_expr` function's `DiGraph`/`UnGraph` arms change from `unimplemented!()` to:

```rust
FieldCategory::DiGraph => quote! { doc.print_digraph(#field_ref) },
FieldCategory::UnGraph => quote! { doc.print_ungraph(#field_ref) },
```

Parser-side `unimplemented!()`s (`ast_type`, `parser_expr`, `ast_kind_name`) remain — parser support is a separate future task.

## Files Changed

| File | Change |
|------|--------|
| `crates/kirin-prettyless/src/document/ir_render.rs` | Add `print_digraph`, `print_ungraph`, `print_ports` |
| `crates/kirin-derive-chumsky/src/field_kind.rs` | Replace `unimplemented!()` in `print_expr` for DiGraph/UnGraph |
