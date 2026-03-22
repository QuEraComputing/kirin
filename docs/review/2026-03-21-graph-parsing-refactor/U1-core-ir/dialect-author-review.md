# U1: Core IR (kirin-ir) -- Dialect Author Review

## Workflow Trace

**Goal**: Add a new graph-containing operation `MyGraphOp { graph: DiGraph, ... }`.

1. Define the struct with `DiGraph` field, annotate with `#[derive(Dialect)]`, `#[kirin(type = T)]`.
2. The derive auto-generates `HasDigraphs`/`HasDigraphsMut` returning an iterator over the `DiGraph` field.
3. To build the graph programmatically, use `BuilderStageInfo::digraph()` (via `DiGraphBuilder`):
   ```rust
   let dg = stage.digraph().port(MyType::F64).port_name("p0").node(stmt).yield_value(ssa).new();
   ```
4. The builder resolves `Unresolved(Port(...))` placeholders against declared ports, builds the `petgraph::Graph`, and allocates the `DiGraphInfo` in the stage arena.

**Friction points**: The builder API `.port().port_name()` pattern (two chained calls for one concept) is slightly awkward -- a `.port_named("p0", MyType::F64)` would be more ergonomic. Minor issue.

## Findings

### [P2] [likely] Finding -- `DiGraphInfo` / `UnGraphInfo` structural duplication

`DiGraphInfo` and `UnGraphInfo` share ~80% of their fields and accessor methods (id, parent, name, ports, edge_count, edge_ports, capture_ports). Similarly, `DiGraphBuilder` and `UnGraphBuilder` duplicate port allocation and placeholder resolution logic (~100 lines). A shared `GraphInfoBase` or trait could reduce this. Not blocking, but a maintenance concern.

**Files**: `crates/kirin-ir/src/node/digraph.rs`, `crates/kirin-ir/src/node/ungraph.rs`, `crates/kirin-ir/src/builder/digraph.rs`, `crates/kirin-ir/src/builder/ungraph.rs`

### [P2] [likely] Finding -- `attach_nodes_to_ungraph` duplicates BFS logic from `UnGraphBuilder::new`

The `attach_nodes_to_ungraph` method (~140 lines) in `stage_info.rs` nearly duplicates the BFS reordering logic from `UnGraphBuilder::new` (~130 lines). A shared helper would reduce error surface.

**File**: `crates/kirin-ir/src/builder/stage_info.rs:381`

### [P3] [confirmed] Finding -- `digraph` and `ungraph` modules are `pub(crate)` but types are `pub use`-d

`node/mod.rs` declares `pub(crate) mod digraph` and `pub(crate) mod ungraph`, then re-exports their types. This is fine for encapsulation, but means a dialect author looking for `DiGraphInfo` docs might not find the source module via module paths. Informational only.

## Domain Alignment

| Domain Concept | IR Mapping | Fit |
|---|---|---|
| Directed data-flow graph (DFG) | `DiGraph` + `petgraph::Directed` | Natural -- nodes are Statements, edges are SSAValues (def-use), matches standard DFG semantics |
| Undirected hyperedge graph | `UnGraph` + `petgraph::Undirected` + edge statements | Natural -- edge SSAs connect exactly 2 nodes, edge statements produce the SSA |
| Port (boundary interface) | `Port` type, `SSAKind::Port(PortParent, idx)` | Natural -- ports are typed SSA values at graph boundary, cleanly separating graph I/O |
| Graph captures (free variables) | `capture_ports()` slice of ports list | Natural -- mirrors MLIR's capture semantics, split by `edge_count` index |
| Forward references in graphs | `BuilderSSAKind::Unresolved(ResolutionInfo)` | Natural -- relaxed dominance via placeholder SSAs resolved at build time |

## Strengths

- The `Port` / `PortParent` abstraction cleanly unifies directed and undirected graph boundary values under SSAKind.
- `BuilderSSAKind::Unresolved(ResolutionInfo)` is a principled placeholder system that supports forward references for graph bodies without special-casing the SSA model.
- The builder pattern (`.port().capture().node().yield_value().new()`) follows the same ergonomic style as `block()` and `region()`, giving dialect authors a consistent construction vocabulary.
- `finalize()` validation ensures no unresolved SSAs leak into finalized IR -- a strong safety invariant.
