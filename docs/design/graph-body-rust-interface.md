# Graph Body Rust Interface Design

This spec defines the Rust types, traits, and derive macro extensions needed for dialect authors to create statements containing `digraph` and `ungraph` bodies. It builds on the text format and semantics defined in `docs/design/graph-ir-node.md`.

## New ID Types

Three new opaque arena ID types, defined via `identifier!` in `kirin-ir/src/node/`:

```rust
identifier! { struct DiGraph }
identifier! { struct UnGraph }
identifier! { struct Port }
```

`Port` represents an SSAValue declaration at the boundary of a graph body (edge ports and captures). It gets `impl_from_ssa!` for free conversion to/from `SSAValue`, like `ResultValue` and `BlockArgument`.

### PortParent

`Port` tracks its owning graph body via:

```rust
pub enum PortParent {
    DiGraph(DiGraph),
    UnGraph(UnGraph),
}
```

### SSAKind Extensions

```rust
pub enum SSAKind {
    Result(Statement, usize),
    BlockArgument(Block, usize),
    Port(PortParent, usize),        // index within the graph's port list
    #[doc(hidden)]
    BuilderPort(usize),             // builder placeholder
    #[doc(hidden)]
    BuilderBlockArgument(usize),
    #[doc(hidden)]
    BuilderResult(usize),
    #[doc(hidden)]
    Test,
}
```

## Info Types

### DiGraphInfo

```rust
pub struct DiGraphInfo<L: Dialect> {
    pub(crate) id: DiGraph,
    pub(crate) parent: Option<Statement>,
    pub(crate) name: Option<Symbol>,
    pub(crate) ports: Vec<Port>,
    pub(crate) edge_count: usize,
    pub(crate) graph: petgraph::DiGraph<Statement, SSAValue>,
    pub(crate) yields: Vec<SSAValue>,
}
```

- `ports[..edge_count]` — edge ports (input directed edges from the enclosing scope)
- `ports[edge_count..]` — captures (read-only values from the enclosing scope, not edges)
- `graph` — petgraph adjacency list; nodes are `Statement` IDs, edges are `SSAValue` IDs (ResultValues of source statements)
- `yields` — output directed edges from the graph body, mapping positionally to the enclosing statement's results

### UnGraphInfo

```rust
pub struct UnGraphInfo<L: Dialect> {
    pub(crate) id: UnGraph,
    pub(crate) parent: Option<Statement>,
    pub(crate) name: Option<Symbol>,
    pub(crate) ports: Vec<Port>,
    pub(crate) edge_count: usize,
    pub(crate) graph: petgraph::UnGraph<Statement, SSAValue>,
    pub(crate) edge_statements: Vec<Statement>,
}
```

- `ports[..edge_count]` — boundary edge ports (edges connecting the inner graph to the outer scope)
- `ports[edge_count..]` — captures (read-only values, not edges)
- `graph` — petgraph adjacency list; nodes are `Statement` IDs (node statements only), edges are `SSAValue` IDs (produced by edge statements)
- `edge_statements` — `edge`-prefixed statement declarations (e.g., `edge %w0 = wire -> Wire;`), stored separately from the petgraph

### Key Differences Between DiGraph and UnGraph

| Aspect | DiGraph | UnGraph |
|--------|---------|---------|
| Node statements | Have operands (incoming) and results (outgoing) | Have only operands (edge connections + captures), no results |
| Edge representation | SSA def-use chains (ResultValues) | Explicit `edge` statements producing ResultValues |
| Output mechanism | `yields: Vec<SSAValue>` | Boundary edges are ports (no yield) |
| Edge statement storage | N/A | `edge_statements: Vec<Statement>` |

## Statement Parent Generalization

`StatementInfo.parent` generalizes from `Option<Block>` to `Option<StatementParent>`:

```rust
pub enum StatementParent {
    Block(Block),
    DiGraph(DiGraph),
    UnGraph(UnGraph),
}

pub struct StatementInfo<L: Dialect> {
    pub(crate) node: LinkedListNode<Statement>,  // only meaningful when parent is Block
    pub(crate) parent: Option<StatementParent>,
    pub(crate) definition: L,
}
```

## StageInfo Extensions

`StageInfo<L>` gains two new arenas:

```rust
pub digraphs: Arena<DiGraph, DiGraphInfo<L>>,
pub ungraphs: Arena<UnGraph, UnGraphInfo<L>>,
```

## New Traits

### Accessor Traits

```rust
pub trait HasDigraphs<'a> {
    type Iter: Iterator<Item = &'a DiGraph>;
    fn digraphs(&'a self) -> Self::Iter;
}

pub trait HasDigraphsMut<'a> {
    type IterMut: Iterator<Item = &'a mut DiGraph>;
    fn digraphs_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasUngraphs<'a> {
    type Iter: Iterator<Item = &'a UnGraph>;
    fn ungraphs(&'a self) -> Self::Iter;
}

pub trait HasUngraphsMut<'a> {
    type IterMut: Iterator<Item = &'a mut UnGraph>;
    fn ungraphs_mut(&'a mut self) -> Self::IterMut;
}
```

### Property Trait

```rust
pub trait IsEdge {
    fn is_edge(&self) -> bool;
}
```

`IsEdge` follows the same pattern as `IsTerminator`. `#[kirin(edge)]` on a struct sets `is_edge() = true`. `#[wraps]` on an enum auto-delegates `is_edge()` to the inner type — no annotation needed on wrapper variants.

### Dialect Supertrait

All new traits are added to the `Dialect` supertrait:

```rust
pub trait Dialect:
    // ... existing bounds ...
    + for<'a> HasDigraphs<'a>
    + for<'a> HasDigraphsMut<'a>
    + for<'a> HasUngraphs<'a>
    + for<'a> HasUngraphsMut<'a>
    + IsEdge
{
    type Type: CompileTimeValue;
}
```

## Derive Toolkit Extensions

### Field Classification

Two new categories in `FieldCategory`:

```rust
pub enum FieldCategory {
    Argument,
    Result,
    Block,
    Successor,
    Region,
    DiGraph,    // new
    UnGraph,    // new
    Symbol,
    Value,
}
```

Two new variants in `FieldData`:

```rust
pub enum FieldData<L: Layout> {
    // ... existing variants ...
    DiGraph,
    UnGraph,
}
```

Field classification in `parse_field` adds type-name checks for `"DiGraph"` and `"UnGraph"`, supporting `Single`, `Vec<T>`, and `Option<T>` collection wrapping.

`#[derive(Dialect)]` auto-generates `HasDigraphs`/`HasUngraphs` impls for all dialects. Dialects with no `DiGraph`/`UnGraph` fields get empty-iterator impls (same pattern as `HasBlocks`/`HasRegions`).

### #[kirin(edge)] Attribute

Applied to statement structs that create edge SSAValues in ungraph bodies:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
#[kirin(edge)]
pub struct ZxWire {
    res: ResultValue,
}
```

Constraints enforced by the derive/verifier:
- The statement must produce exactly one `ResultValue`
- The statement must not consume any edge SSAValues as operands
- The statement can only appear in `UnGraphInfo.edge_statements`
- The printer emits the `edge` prefix; the parser expects it

## Validation Rules

### Builder-Time: Ungraph Edge Use Count

When building an ungraph IR, the builder must reject any edge SSAValue (produced by an edge statement) that is referenced by more than 2 node statements. This enforces the strict ungraph constraint — each edge connects exactly 2 nodes.

This is checked at IR construction time (builder), not deferred to a later verification pass. If a future `HyperGraph` body kind is added, it will lift this restriction with its own builder.

### Verifier Checks

- **DiGraph**: SSA def-use chains form the directed edges. Fan-out validity depends on edge type semantics (e.g., `Tensor` allows fan-out, `Qubit` does not) — this is a dialect-level verifier concern.
- **UnGraph**: Each edge SSAValue connects exactly 2 node statements (enforced at build time). Edge statements are in `edge_statements`, not in the petgraph. Node statements have no results.
- **Edge statements**: Only valid inside ungraph bodies. Must have `is_edge() = true`. Must produce exactly one `ResultValue`.
- **Captures**: `ports[edge_count..]` are read-only values from the enclosing scope. They must not participate in graph topology.

## Dialect Author Experience

### Directed Graph Dialect

```rust
#[derive(Clone, Debug, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
pub struct Hadamard {
    input: SSAValue,
    res: ResultValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
pub struct QuantumEval<T: CompileTimeValue> {
    qubit: SSAValue,
    angle: SSAValue,
    body: DiGraph,
    res: ResultValue,
    #[kirin(default)]
    marker: PhantomData<T>,
}
```

### Undirected Graph Dialect

```rust
// Edge statement
#[derive(Clone, Debug, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
#[kirin(edge)]
pub struct ZxWire {
    res: ResultValue,
}

// Node statement — no results, only operands
#[derive(Clone, Debug, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
pub struct ZSpider {
    phase: SSAValue,
    edges: Vec<SSAValue>,
}

// Statement with ungraph body
#[derive(Clone, Debug, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
pub struct ZxEval<T: CompileTimeValue> {
    boundary_edges: Vec<SSAValue>,
    captures: Vec<SSAValue>,
    body: UnGraph,
    #[kirin(default)]
    marker: PhantomData<T>,
}
```

### Wrapper Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, Dialect, HasParser, PrettyPrint)]
#[wraps]
pub enum ZxDialect<T: CompileTimeValue> {
    Wire(ZxWire),          // is_edge() delegates to ZxWire → true
    ZSpider(ZSpider),      // delegates → false
    XSpider(XSpider),      // delegates → false
    ZxEval(ZxEval<T>),     // delegates → false
}
```

## Edge Metadata Query Path

Petgraph edge weights are `SSAValue` IDs. The SSAKind discriminant determines the query path:

**Internal edges** (produced by `edge` statements in ungraph, or by statement results in digraph):
1. Query petgraph edge → `SSAValue` (edge weight)
2. Look up `SSAInfo` → `SSAKind::Result(Statement, usize)` — parent is the producing statement
3. Look up `StatementInfo` → dialect definition (e.g., `ZxWire`, `WeightedWire(3.14)`)
4. Dialect definition carries whatever metadata the author defined

**Boundary port edges** (ports from the enclosing scope):
1. Query petgraph edge → `SSAValue` (edge weight)
2. Look up `SSAInfo` → `SSAKind::Port(PortParent, usize)` — parent is the graph body, index identifies the port
3. Port metadata (type, name) is in the `SSAInfo` itself; the enclosing statement's operands provide the outer-scope binding

All O(1) arena lookups.

## Future Work

- `HyperGraph` keyword and body kind — lifts the 2-use edge restriction, likely with a dedicated data structure
- Parser/printer implementation for graph bodies
- Interpreter traversal of graph bodies
- Builder APIs for constructing graph bodies
- Block ↔ DiGraph rewrite tooling
- Graph pattern matching / rewrite rules
