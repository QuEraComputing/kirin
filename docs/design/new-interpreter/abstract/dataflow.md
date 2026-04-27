# Forward And Backward Dataflow Shapes

This document specializes the abstract framework in
[framework.md](framework.md) for forward constant propagation, backward
liveness, and widening/narrowing policies.

These analyses do not require a different frame protocol. They specialize:

- the transfer type `T`,
- the summary owner key `K`,
- the summary type `S`,
- the dependency index,
- and sometimes the frame family or traversal order.

## Forward Constant Propagation

Forward constant propagation can start with one summary owner per specialized
function:

```rust
pub struct FunctionOwner {
    pub function: SpecializedFunction,
}

pub enum ConstValue {
    Bottom,
    Const(Value),
    Top,
}

pub struct ConstSummary {
    pub env: AbstractEnv<ConstValue>,
}
```

Blocks and regions normally share the function activation env. A block frame
does not imply a block summary.

For a function with a CFG region body, the region frame can handle branch
transfers locally:

```rust
pub enum ForwardTransfer<V> {
    Branch(Vec<BlockTransfer<V>>),
}

pub struct BlockTransfer<V> {
    pub target: Block,
    pub args: Vec<V>,
}
```

The region frame owns the traversal order:

```text
branch transfer
    -> bind target block args in the current activation env
    -> schedule or enter the target block inside the same function owner
```

No top-level block summary is required unless the analysis needs block-level
convergence or more precise scheduling.

### Owner Choices

The first implementation should keep blocks local to the function owner:

```text
K = FunctionOwner
Token = ()
```

This is simple and fits finite constant propagation.

Later, the analysis can choose finer owners:

- CFG loop headers for widening placement,
- `scf.for` statements for loop-carried convergence,
- graph nodes for graph-body analyses.

Those are changes to `K` and frame semantics, not changes to `FrameEffect`.

## Backward Liveness

Backward liveness uses the same framework but reverses the information-flow
direction.

```rust
pub struct LivenessSummary {
    pub live: IndexSet<SSAValue>,
}

pub enum BackwardTransfer<D> {
    Branch(Vec<D>),
}

pub struct BackwardEdge {
    pub predecessor: Block,
    pub uses_required_by_successor: IndexSet<SSAValue>,
}
```

The active frame family can be specialized for backward traversal. A backward
region frame may process blocks from exits toward entries, and a backward
dependency index schedules predecessors when successor facts change.

```rust
pub struct BackwardSummaryDeps<K> {
    pub predecessors_by_successor: IndexMap<K, Vec<K>>,
}
```

`StatementEffect::Transfer(T)` still works. The difference is that `T` carries
backward obligations instead of forward target values.

## Branch Is Direction-Neutral

`StatementEffect::Transfer(T)` does not assume forward execution. A forward
analysis can interpret branch edges as successor facts. A backward analysis can
interpret branch edges as predecessor obligations.

That is why the generic statement effect keeps transfer opaque:

```rust
pub enum StatementEffect<F, C, T> {
    Done,
    Push(F),
    Complete(C),
    Transfer(T),
}
```

The active frame consumes `T`.

## Widening And Narrowing

Widening and narrowing live in `Summary::merge`.

```rust
pub enum FixpointPhase {
    Join,
    Widen,
    Narrow,
}
```

Finite analyses, such as simple constant propagation or liveness over finite
sets, can implement widening as join and narrowing as no-op.

Infinite-height analyses, such as intervals, use a strategy:

```rust
pub struct WidenNarrowStrategy {
    pub widen_after: usize,
    pub narrow_iterations: usize,
}
```

The strategy is owner-local. Each summary owner can track visits and decide
when to widen:

```rust
pub struct IntervalSummary {
    pub env: AbstractEnv<Interval>,
    pub visits: usize,
    pub narrow_visits: usize,
}
```

The dependency index decides what to revisit after a summary changes. The
summary strategy decides whether that change came from join, widening, or
narrowing.

## Choosing Summary Owners

The best owner shape depends on the analysis:

| Analysis | Simple owner | Finer owner when needed |
| --- | --- | --- |
| constant propagation | function | CFG loop header, `scf.for`, graph node |
| interval analysis | function | loop header or loop statement for targeted widening |
| liveness | function or region | block, loop header, graph node |
| graph dataflow | graph | graph node or SCC head |

The framework does not force one choice. Dialect authors and analysis authors
can specialize frame families and `Interpretable` impls for the interpreter
type they are building.
