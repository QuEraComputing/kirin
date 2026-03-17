# Native Graph IR Node — Text Format and Semantics Design

This design introduces two new IR body kinds — `digraph` and `ungraph` — alongside Block and Region. A graph body uses standard statement syntax where SSAValues represent edges. The leading keyword (`^`, `digraph`, `ungraph`) selects the backing storage: Block (linked list), petgraph DiGraph, or petgraph UnGraph.

- For **directed graphs**, the text format follows MLIR graph region semantics (relaxed dominance, SSA def-use = directed edges).
- For **undirected graphs**, `edge`-prefixed statements introduce edge SSAValues, and statements that share edge references are connected.

## References

- **MLIR Graph Regions**: `RegionKind::Graph` relaxes SSA dominance. No syntactic difference from SSACFG regions. No support for undirected graphs.
- **CIRCT (MLIR)**: HW dialect encodes netlist topology via SSA def-use chains in graph regions.
- **HUGR (Quantinuum)**: Port-centric model. Edges implicit via shared link names. Hierarchy via nesting.
- **RVSDG**: All control flow as structural nodes with sub-regions. Typed ports. No standard text format.
- **DOT (Graphviz)**: `digraph`/`graph` keyword selects directedness. Subgraphs for hierarchy.

## Overview

### Three Body Kinds

| Keyword | Body Kind | Storage | Dominance | Edge Semantics |
|---------|-----------|---------|-----------|----------------|
| `^bb0(args)` | Block | Linked list | Enforced | N/A (sequential) |
| `digraph ^dg0(args)` | Directed graph | petgraph DiGraph | Relaxed | SSA def-use = directed edge |
| `ungraph ^ug0(args)` | Undirected graph | petgraph UnGraph | Relaxed | Shared edge reference = connection |

All three use standard statement syntax.

### The `capture(...)` Clause

Graph bodies can declare external (non-edge) value dependencies via an optional `capture(...)` clause:

```
digraph ^name(edge_args...) capture(captured_values...) { ... }
ungraph ^name(edge_args...) capture(captured_values...) { ... }
```

- **Edge arguments** (first list) — SSAValues that participate in graph topology
- **Captures** (second list) — read-only values from the enclosing scope, not edges
- The `capture(...)` clause is optional — omit it when the graph has no external dependencies
- All non-edge values used inside the body must be declared as captures — implicit capture is not allowed

### SSAValue Classification

Every SSAValue inside a graph body is one of:

1. **Graph arguments** → edge SSAValues
2. **Statement results** (in `digraph`) → edge SSAValues (outgoing directed edges)
3. **`edge`-prefixed statement results** (in `ungraph`) → edge SSAValues (undirected edges)
4. **Compound node results** (in `ungraph`) → edge SSAValues (output edges from inner graph)
5. **`capture(...)` arguments** → captured values, read-only, not edges
6. **Anything else** → error

The **statement's type definition** declares which of its operands are edges vs captured parameters. For example, `z_spider(%theta, %p0, %w0)` — the dialect author's struct specifies that `phase` is captured and the rest are edge connections.

## Directed Graph (`digraph`)

SSA values represent directed edges. A statement's operands are incoming edges, its results are outgoing edges. Dominance is relaxed — statements can be in any order.

```
digraph ^name(edge_args...) {
  statements...
  yield outputs...;
}
```

### Example: Quantum Circuit

```
specialize @quantum fn @bell_pair(Qubit, Qubit) -> (Qubit, Qubit) {
  digraph ^dg0(%q0: Qubit, %q1: Qubit) {
    %0 = hadamard %q0 -> Qubit;
    %1, %2 = cnot %0, %q1 -> (Qubit, Qubit);
    yield %1, %2;
  }
}
```

- `%q0`, `%q1` — input edge SSAValues (graph arguments)
- `%0` — edge from hadamard to cnot (SSA def-use = directed edge)
- `%1`, `%2` — output edges (yielded)

### Example: Parameterized Quantum Circuit

```
specialize @quantum fn @variational(Qubit, f64, f64) -> Qubit {
  digraph ^dg0(%q: Qubit) capture(%theta: f64, %phi: f64) {
    %0 = rz(%theta) %q -> Qubit;
    %1 = rx(%phi) %0 -> Qubit;
    yield %1;
  }
}
```

- `%q` — input edge (graph argument)
- `%theta`, `%phi` — captured values, read-only, not edges
- Function signature `(Qubit, f64, f64)` maps positionally: `%q` = param 0, `%theta` = param 1, `%phi` = param 2

### Example: Dataflow / Computational Graph

```
specialize @nn fn @layer(Tensor, Tensor, Tensor) -> Tensor {
  digraph ^dg0(%input: Tensor, %weights: Tensor, %bias: Tensor) {
    %0 = matmul %input, %weights -> Tensor;
    %1 = add %0, %bias -> Tensor;
    %2 = relu %1 -> Tensor;
    yield %2;
  }
}
```

## Undirected Graph (`ungraph`)

Statements that reference the same edge SSAValue are connected. Multiple statements sharing an edge = multi-way connection (supports hypergraphs).

```
ungraph ^name(edge_args...) {
  edge %name = dialect_edge_op(metadata...) -> EdgeType;
  ...
  node_statements...
}
```

### The `edge` Prefix

The `edge` keyword is a prefix that marks a statement as edge-creating. Dialect authors define their own edge operations and edge types.

```
edge %w0 = wire -> Wire;
edge %w1 = hadamard_wire -> ZXEdge;
edge %w2 = weighted_wire(3.14) -> WeightedEdge;
```

- The actual operation (`wire`, `hadamard_wire`, `weighted_wire`) is dialect-defined.
- The operation can take captured value operands (metadata, weights, etc.) but **must not consume any edge SSAValues**.
- The result is an edge SSAValue that participates in the graph topology.
- Dialect authors annotate their edge-creating statement structs (e.g., `#[kirin(edge)]`) so the printer emits the `edge` prefix automatically.

### Dialect-Defined Edge Types

The edge type (`-> Type`) is dialect-defined and carries whatever metadata the dialect needs:

| Dialect | Edge Operation | Edge Type | What the type encodes |
|---------|---------------|-----------|----------------------|
| Basic quantum | `wire` | `Wire` | Plain connection, no metadata |
| ZX calculus | `hadamard_wire` | `ZXEdge` | Hadamard vs plain edge distinction |
| ZX calculus | `plain_wire` | `ZXEdge` | Same type, different constructor |
| Weighted graph | `weighted_wire(3.14)` | `WeightedEdge` | Numeric weight on the edge |
| Tensor network | `bond(dim: 4)` | `TensorBond` | Bond dimension |
| Netlist | `signal(drive: high)` | `Signal` | Drive strength, logic level |

- Different edge operations can produce the same type (operation = constructor, type = classification)
- Edge operations can be parameterized with captured values
- Edge types are checked by the verifier

### Example: ZX Diagram

```
specialize @zx fn @simplify(Wire, Wire, f64, f64, f64) -> (Wire, Wire) {
  ungraph ^ug0(%p0: Wire, %p1: Wire) capture(%zero: f64, %pi: f64, %half_pi: f64) {
    edge %w0 = wire -> Wire;
    edge %w1 = wire -> Wire;
    edge %w2 = wire -> Wire;
    edge %w3 = wire -> Wire;
    edge %w4 = wire -> Wire;
    z_spider(%zero, %p0, %w0, %w1);
    x_spider(%pi, %w0, %w2, %w3);
    z_spider(%half_pi, %w1, %w3, %w4);
  }
}
```

- `%p0`, `%p1` — input edge SSAValues (graph arguments)
- `%zero`, `%pi`, `%half_pi` — captured values, not edges
- `%w0` appears in both `z_spider` and `x_spider` — they are connected by wire `%w0`

### Example: ZX Diagram with Edge Metadata

```
specialize @zx fn @colored(Wire, Wire, f64, f64) -> Wire {
  ungraph ^ug0(%p0: Wire, %p1: Wire) capture(%theta: f64, %phi: f64) {
    edge %w0 = hadamard_wire -> ZXEdge;
    edge %w1 = plain_wire -> ZXEdge;
    z_spider(%theta, %p0, %w0);
    x_spider(%phi, %w0, %w1);
  }
}
```

- `edge %w0 = hadamard_wire -> ZXEdge` — a Hadamard edge
- `edge %w1 = plain_wire -> ZXEdge` — a plain edge
- Different edge operations produce different edge semantics — dialect author has full control

## Hierarchy (Compound Nodes)

A statement inside a graph body can contain an inner graph body, creating a compound node. It follows the same convention as a function call: operands map positionally to the inner graph's `[edge_args ++ captures]`.

```
%out = compound_op(%edge0, %edge1, %captured0) {
  ungraph ^ug1(%ip0: Wire, %ip1: Wire) capture(%c: f64) {
    ...
  }
} -> Wire;
// %edge0 → %ip0, %edge1 → %ip1, %captured0 → %c
```

- Operands map positionally to inner `[edge_args ++ captures]`
- Compound node results are edge SSAValues in the outer graph

### Example: Nested ZX Diagram

```
specialize @zx fn @composed(Wire, Wire, f64, f64, f64, f64) -> Wire {
  ungraph ^ug0(%p0: Wire, %p1: Wire)
      capture(%theta: f64, %phi: f64, %alpha: f64, %beta: f64) {
    edge %w0 = wire -> Wire;
    edge %w1 = wire -> Wire;
    edge %w3 = wire -> Wire;
    z_spider(%theta, %p0, %w0, %w1);
    x_spider(%phi, %w2, %w3);
    %w2 = zx_sub(%w0, %w1, %alpha, %beta) {
      ungraph ^ug1(%ip0: Wire, %ip1: Wire) capture(%a: f64, %b: f64) {
        edge %iw0 = wire -> Wire;
        z_spider(%a, %ip0, %iw0);
        x_spider(%b, %ip1, %iw0);
      }
    } -> Wire;
  }
}
```

- `zx_sub(%w0, %w1, %alpha, %beta)` maps positionally: `%w0` → `%ip0`, `%w1` → `%ip1` (edges), `%alpha` → `%a`, `%beta` → `%b` (captures)
- Inner graph declares its own `capture(%a, %b)` — binds to the compound node's operands, not the outer scope
- Inner edge SSAValues (`%iw0`) are scoped to the inner body

### Example: Nested Directed Graph

```
specialize @hybrid fn @nested(Qubit, Qubit, f64) -> (Qubit, Qubit) {
  digraph ^dg0(%q0: Qubit, %q1: Qubit) capture(%theta: f64) {
    %0 = hadamard %q0 -> Qubit;
    %1 = sub_circuit(%q1, %theta) {
      digraph ^dg1(%iq: Qubit) capture(%t: f64) {
        %2 = rz(%t) %iq -> Qubit;
        %3 = hadamard %2 -> Qubit;
        yield %3;
      }
    } -> Qubit;
    %4, %5 = cnot %0, %1 -> (Qubit, Qubit);
    yield %4, %5;
  }
}
```

## Integration with Blocks and Functions

### Graph Body as Function Body

A function can have a graph body directly. The function signature maps positionally to `[edge_args ++ captures]`:

```
specialize @quantum fn @bell_pair(Qubit, Qubit) -> (Qubit, Qubit) {
  digraph ^dg0(%q0: Qubit, %q1: Qubit) {
    %0 = hadamard %q0 -> Qubit;
    %1, %2 = cnot %0, %q1 -> (Qubit, Qubit);
    yield %1, %2;
  }
}
```

- Function parameter count must equal `len(edge_args) + len(captures)`
- Types must match positionally: `[edge_arg_types ++ capture_types] == [function_param_types]`
- `yield` operand count must match the function's return type count

### Graph Body Inside a Block

Classical computation in a Block feeds into a graph body via a wrapping statement:

```
specialize @hybrid fn @vqe(f64, f64) -> Qubit {
  ^entry(%theta: f64, %phi: f64) {
    %angle = arith.add %theta, %phi -> f64;
    %q = qubit_alloc() -> Qubit;
    %result = quantum_eval(%q, %angle) {
      digraph ^dg0(%q_in: Qubit) capture(%angle: f64) {
        %0 = rz(%angle) %q_in -> Qubit;
        %1 = hadamard %0 -> Qubit;
        yield %1;
      }
    } -> Qubit;
    ret %result;
  }
}
```

- `%angle` is computed in the Block, captured by the digraph
- `quantum_eval(%q, %angle)` maps operands to inner `[edge_args ++ captures]`

## Semantic Rules

### Dominance

Graph bodies use relaxed dominance (like MLIR graph regions). Statement order is not semantically meaningful. SSAValues may be referenced before their textual definition.

### Scoping

- Graph argument and capture names are visible throughout the graph body.
- SSAValues defined inside the body are visible only within that body.
- Inner graph bodies (compound nodes) use a separate namespace — inner names don't leak out, and may reuse numeric indices without conflict.
- Outer scope values are **not** directly visible inside inner graphs — they must be threaded through the compound node's operand/capture mapping.

### `yield` Semantics

`yield` terminates a **digraph** body and declares its output edges:

- Yielded values must be edge SSAValues (not captured values).
- They map positionally to the enclosing operation's result types.
- **`ungraph` bodies do not have `yield`** — undirected graphs have no notion of directed output edges. Edge ports on the boundary provide the interface to the enclosing scope.

### Edge Multiplicity

**In an `ungraph`**, an edge SSAValue appearing in multiple statements connects them:

```
// Binary edge: %w0 connects exactly two nodes
edge %w0 = wire -> Wire;
z_spider(%theta, %w0);      // connected to %w0
x_spider(%phi, %w0);         // connected to %w0

// Hyperedge: %w1 connects three nodes
edge %w1 = wire -> Wire;
z_spider(%a, %w1);           // connected to %w1
x_spider(%b, %w1);           // connected to %w1
z_spider(%c, %w1);           // connected to %w1
```

**In a `digraph`**, an SSAValue used by multiple statements creates fan-out:

```
// Fan-out (valid): matmul's output feeds into both add and sub
%0 = matmul %input, %weights -> Tensor;
%1 = add %0, %bias -> Tensor;     // %0 used here
%2 = sub %0, %offset -> Tensor;   // %0 also used here
// Two directed edges from matmul: one to add, one to sub
```

```
// Fan-out (INVALID in quantum): qubits are linear, cannot be cloned
%0 = hadamard %q -> Qubit;
%1, %2 = cnot %0, %q1 -> (Qubit, Qubit);   // %0 used here
%3 = rz(%theta) %0 -> Qubit;                 // %0 also used here — ERROR: no-cloning violation
```

Whether fan-out is valid depends on the edge type's semantics — `Tensor` supports fan-out (implicit copy), `Qubit` does not (linear/no-cloning). This is a dialect-level concern enforced by the verifier.

### Cycles and Self-References

Because graph bodies have relaxed dominance, cycles and self-references are structurally valid.

**In a `digraph`** — a cycle means a statement consumes the result of a statement that transitively consumes its result:

```
// Feedback loop in a signal processing graph
digraph ^dg0(%input: Signal) {
  %0 = delay %2 -> Signal;          // consumes %2, defined below (cycle)
  %1 = add %input, %0 -> Signal;
  %2 = gain(0.5) %1 -> Signal;      // its result feeds back to delay
  yield %1;
}
```

**In an `ungraph`** — a self-loop means the same edge appears twice in one node's operands:

```
// Self-loop: edge %w0 connects z_spider to itself
ungraph ^ug0(%p0: Wire) {
  edge %w0 = wire -> Wire;
  z_spider(%theta, %p0, %w0, %w0);  // %w0 appears twice — self-loop
}
```

Whether cycles or self-loops are semantically valid is a dialect-level concern. Quantum circuits (DAGs) forbid cycles; signal processing graphs and feedback networks allow them.

### Equivalence Between Body Kinds

A directed acyclic graph body can be serialized into an equivalent Block (topological sort), and vice versa. These are different representations of the same semantics with different memory layout and algorithmic tradeoffs.

For undirected graphs, the Block form uses `edge`-prefixed statements plus relaxed dominance. The `ungraph` form uses petgraph UnGraph storage. Both represent the same graph — the keyword selects the backing.

### Reserved Keywords

Inside a graph body: `digraph`, `ungraph`, `edge`, `capture`.

## Deferred (Backlog)

- Parser/printer implementation for graph bodies
- Derive macro support for edge operand annotations
- Interpreter traversal of graph bodies
- Analysis tooling (dominance, liveness on graphs)
- Block <-> DiGraph rewrite tooling
- Hypergraph-specific validation and algorithms
- Graph pattern matching / rewrite rules language
- Rust data structure design (type aliases, traits, arena integration)
