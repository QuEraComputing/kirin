# Graph Body Pretty Printing Design

Extends kirin-prettyless to print `digraph` and `ungraph` bodies following the text format defined in `docs/design/graph-ir-node.md`.

## Ordering Strategy

**The printer is simple — the builder does the work.**

Both `DiGraphBuilder` and `UnGraphBuilder` reindex the petgraph at `.new()` time so that node indices are already in the correct print order. The printer just iterates `graph.node_references()` in index order.

### DiGraph: Topological Order

At builder `.new()` time:
1. Build initial petgraph from SSA def-use analysis
2. Run `petgraph::algo::toposort`
3. If `Ok` (DAG): rebuild petgraph with nodes in topo order, remap edges
4. If `Err` (cycle): keep insertion order (no reindex)

### UnGraph: BFS from Boundary Ports

At builder `.new()` time:
1. Build initial petgraph from shared edge SSAValues
2. BFS from boundary-port-connected nodes:
   - For each node being visited, collect all its unvisited edges
   - Visit those edge statements (mark as visited)
   - For each newly visited edge, enqueue the other endpoint node
   - Visit the node
3. Remaining isolated nodes appended at the end
4. Rebuild petgraph with nodes in BFS visit order, remap edges
5. Reorder `edge_statements` to match BFS edge visitation order

**Invariant:** every edge a node references is ordered before the node in `edge_statements`. This produces grouped output:

```
ungraph ^ug0(%p0: Wire) capture(%zero: f64, %pi: f64) {
  edge %w0 = wire -> Wire;
  edge %w1 = wire -> Wire;
  z_spider(%zero, %p0, %w0, %w1);
  edge %w2 = wire -> Wire;
  x_spider(%pi, %w0, %w2);
  another_spider(%zero, %w1, %w2);
}
```

Both reindex operations are O(N + E), run once at construction time.

## Builder Design

### DiGraphBuilder

```rust
DiGraphBuilder::from_stage(stage)
    .name("dg0")
    .port(Qubit).port_name("q0")
    .port(Qubit).port_name("q1")
    .capture(F64).capture_name("theta")
    .node(hadamard_stmt)
    .node(cnot_stmt)
    .yield_value(ssa1)
    .yield_value(ssa2)
    .new()  // -> DiGraph
```

**Fields:**
- `stage: &'a mut StageInfo<L>`
- `parent: Option<Statement>`
- `name: Option<String>`
- `ports: Vec<(L::Type, Option<String>)>` — edge ports
- `captures: Vec<(L::Type, Option<String>)>` — capture ports
- `nodes: Vec<Statement>` — graph node statements
- `yields: Vec<SSAValue>` — output edges

**`.new()` logic:**
1. Allocate `DiGraph` ID
2. Create `Port` SSAValues for edge ports (index `0..ports.len()`) and captures (index `ports.len()..`)
3. Resolve `BuilderPort(index)` placeholders in statement operands to real `Port` IDs
4. Build petgraph: add each statement as a node, analyze SSA def-use to add directed edges (if operand is a `ResultValue` of another node in this graph, add edge from producer → consumer with `SSAValue` as weight)
5. Topo sort + reindex (DAG) or keep insertion order (cycle)
6. Set `StatementParent::DiGraph(id)` on all statements
7. Allocate `DiGraphInfo` in the arena

### UnGraphBuilder

```rust
UnGraphBuilder::from_stage(stage)
    .name("ug0")
    .port(Wire).port_name("p0")
    .port(Wire).port_name("p1")
    .capture(F64).capture_name("zero")
    .edge(wire_stmt1)
    .node(z_spider_stmt)
    .node(x_spider_stmt)
    .edge(wire_stmt2)
    .node(another_stmt)
    .new()  // -> UnGraph
```

`.edge()` and `.node()` can be interleaved in any order.

**Fields:**
- `stage: &'a mut StageInfo<L>`
- `parent: Option<Statement>`
- `name: Option<String>`
- `ports: Vec<(L::Type, Option<String>)>` — boundary edge ports
- `captures: Vec<(L::Type, Option<String>)>` — capture ports
- `edge_stmts: Vec<Statement>` — edge declaration statements
- `nodes: Vec<Statement>` — node statements

**`.new()` logic:**
1. Allocate `UnGraph` ID
2. Create `Port` SSAValues for boundary edge ports and captures
3. Resolve `BuilderPort(index)` placeholders
4. Build petgraph: add each node statement as a petgraph node, find shared edge SSAValues between nodes to add undirected edges
5. **Validate:** each edge SSAValue referenced by at most 2 node statements (strict ungraph constraint)
6. BFS reindex from boundary-connected nodes + reorder `edge_stmts` to match
7. Set `StatementParent::UnGraph(id)` on all node statements and edge statements
8. Allocate `UnGraphInfo` in the arena

### BuilderPort Resolution

Same pattern as `BuilderBlockArgument`. Statements reference `SSAKind::BuilderPort(index)` placeholders. At `.new()` time, the builder scans statement operands and replaces `BuilderPort(index)` with real `Port` IDs. Index maps to `all_ports[index]` where `all_ports = edge_ports ++ capture_ports`.

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
| Create | `crates/kirin-ir/src/builder/digraph.rs` | `DiGraphBuilder` |
| Create | `crates/kirin-ir/src/builder/ungraph.rs` | `UnGraphBuilder` |
| Modify | `crates/kirin-ir/src/builder/mod.rs` | Declare new modules |
| Modify | `crates/kirin-prettyless/src/document/ir_render.rs` | Add `print_digraph`, `print_ungraph`, `print_ports` |
| Modify | `crates/kirin-derive-chumsky/src/field_kind.rs` | Replace `unimplemented!()` in `print_expr` |
