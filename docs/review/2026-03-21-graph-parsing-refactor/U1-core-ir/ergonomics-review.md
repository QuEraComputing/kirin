# U1: Core IR -- Ergonomics/DX Review

## Toy Scenario

I want to model a quantum circuit as a directed graph. Each gate is a node, wires are SSA edges between them.

```rust
let mut stage = BuilderStageInfo::<MyDialect>::default();

// Create gate statements
let h_gate = stage.statement().definition(MyDialect::H(arg0)).new();
let cnot_gate = stage.statement().definition(MyDialect::CNOT(res_h, arg1)).new();

// Build graph body
let dg = stage.digraph()
    .port(QubitType)        // edge port 0
    .port_name("q0")
    .port(QubitType)        // edge port 1
    .port_name("q1")
    .node(h_gate)
    .node(cnot_gate)
    .yield_value(res_cnot_ctrl)
    .yield_value(res_cnot_tgt)
    .new();
```

Problem: how do I reference port 0 inside `h_gate`'s definition? The port SSA value does not exist until `digraph().new()` allocates it. I must use `stage.block_argument().index(0)` style placeholders, but the method is named `block_argument` -- not `port_ref` or `graph_port`. The placeholder type `ResolutionInfo::Port(BuilderKey::Index(0))` reveals the answer, but only after reading the source.

## Findings

### [P1] [confirmed] DiGraph/UnGraph builder `new()` method name violates Rust convention -- digraph.rs:85, ungraph.rs:78

The `#[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]` suppression is a clear signal: `new()` consumes `self` and returns a `DiGraph` ID, not `Self`. Users writing `.new()` expect a constructor. Consider `.build()` or `.finish()` to match standard builder conventions and avoid the clippy override.

### [P2] [likely] Port placeholder creation lacks a dedicated builder method -- builder/mod.rs

Users must manually create SSA placeholders with `BuilderSSAKind::Unresolved(ResolutionInfo::Port(BuilderKey::Index(0)))` to reference a port inside a graph node. Blocks have `stage.block_argument().index(0)`, but graphs have no analogous `stage.port_ref().index(0)` or `stage.capture_ref().index(0)` convenience. This forces users into low-level resolution info internals.

### [P2] [likely] DiGraphInfo and UnGraphInfo are nearly identical -- digraph.rs, ungraph.rs

90%+ code duplication between `DiGraphInfo` and `UnGraphInfo`. Only differs in `petgraph::Directed` vs `petgraph::Undirected` and `yields` vs `edge_statements`. The builder code in `builder/digraph.rs` and `builder/ungraph.rs` duplicates even more (~230 lines each of nearly identical resolution/BFS logic). Not a user-facing issue per se, but internal contributors will find the duplication surprising and error-prone.

### [P3] [uncertain] Concept budget for graph operations is high

`DiGraph`, `UnGraph`, `Port`, `PortParent`, `SSAKind::Port`, `ResolutionInfo::Port`, `ResolutionInfo::Capture`, `BuilderKey`, `edge_count` boundary convention -- all needed to use a graph body.

## Concept Budget Table

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| SSAValue / ResultValue / BlockArgument | node/ssa.rs | Low |
| Block / Region distinction | AGENTS.md | Med |
| DiGraph / UnGraph | node/digraph.rs, ungraph.rs | Med |
| Port / PortParent | node/port.rs | Med |
| edge_count boundary (ports[:edge_count] vs ports[edge_count:]) | DiGraphInfo methods | High |
| BuilderStageInfo lifecycle (build -> finalize) | builder/stage_info.rs | Med |
| Placeholder resolution (BuilderSSAKind -> SSAKind) | node/ssa.rs | High |
| Signature / SignatureSemantics | signature/ | Med |

## Lifetime Complexity

(i) **Hidden by derive**: `L: Dialect` bound on `DiGraphInfo<L>` -- users never see `PhantomData<L>` thanks to derive.
(ii) **Visible necessary**: `'a` on `DiGraphBuilder<'a, L>` -- borrows `BuilderStageInfo`, required.
(iii) **Visible avoidable**: None found.

## Strengths

- `BuilderStageInfo` doc comments are excellent -- the `ignore` code blocks walk through the full lifecycle.
- `Signature<T, C = ()>` is clean and intuitive. Constraints default away for simple cases.
- `AsBuildStage` with `#[diagnostic::on_unimplemented]` gives a helpful error when users pass the wrong type.
- `FinalizeError` variants are clear and actionable.
