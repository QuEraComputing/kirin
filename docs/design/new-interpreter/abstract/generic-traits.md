# Abstract Interpreter Generic Traits

Abstract interpretation uses the same `Frame`, `FrameEffect`, `Location`,
`Completion`, and lift/project algebra as concrete interpretation. It should
not use a LIFO frame stack as its primary state, and it should not require a
graph data structure as the public abstraction.

The core shape is closer to traditional k-CFA: finite tables keyed by abstract
nodes, plus a driver-specific dependency index and worklist.

## Abstract State

```rust
pub struct FrameNodeKey {
    pub location: Location,
    pub context: NodeContext,
}

pub struct NodeContext {
    pub entries: Vec<Location>,
}

pub struct AbstractState<K, F, S, Store, Deps> {
    pub frames: IndexMap<K, F>,
    pub summaries: IndexMap<K, S>,
    pub store: Store,
    pub deps: Deps,
    pub worklist: VecDeque<WorkItem<K>>,
}

pub enum WorkItem<K> {
    Step(K),
    Resume { parent: K, child: K },
}
```

`deps` is not a semantic object. It is a reverse dependency index used by the
worklist: when one node summary changes, it tells the driver which other work
items may need to run. Different abstract interpreters should define different
`Deps` types.

## Generic Frame Dependencies

The generic frame-push case can use this dependency index:

```rust
pub struct FrameDeps<K> {
    pub children_by_parent: IndexMap<K, Vec<K>>,
    pub parents_by_child: IndexMap<K, Vec<K>>,
}

impl<K> FrameDeps<K>
where
    K: Clone + Eq + Hash,
{
    pub fn register_push(&mut self, parent: K, child: K) {
        self.children_by_parent
            .entry(parent.clone())
            .or_default()
            .push(child.clone());

        self.parents_by_child
            .entry(child)
            .or_default()
            .push(parent);
    }
}
```

The two maps have different meanings:

- `children_by_parent[parent]`: if the parent summary changes, recompute the
  input summary of each child reached from that parent.
- `parents_by_child[child]`: if the child summary changes, resume each waiting
  parent from the child's current abstract completion.

The abstract driver uses it like this:

```rust
fn schedule_dependents<K, F, S, Store>(
    state: &mut AbstractState<K, F, S, Store, FrameDeps<K>>,
    changed: K,
) where
    K: Clone + Eq + Hash,
{
    for child in state
        .deps
        .children_by_parent
        .get(&changed)
        .into_iter()
        .flatten()
    {
        state.worklist.push_back(WorkItem::Step(child.clone()));
    }

    for parent in state
        .deps
        .parents_by_child
        .get(&changed)
        .into_iter()
        .flatten()
    {
        state.worklist.push_back(WorkItem::Resume {
            parent: parent.clone(),
            child: changed.clone(),
        });
    }
}
```

Applying `FrameEffect::Push { parent, child }` updates the table and installs
both dependency directions:

```text
apply_push(current, parent, child):
    child_key = allocate_child_key(current, child.location)

    frames[current] = parent
    frames[child_key] = join_or_replace_frame(child_key, child)

    deps.register_push(current, child_key)

    if merge_child_input(child_key, summary_from_parent(current)):
        worklist.push(Step(child_key))
```

When the child summary later changes, `parents_by_child` schedules:

```text
Resume { parent: current, child: child_key }
```

The driver then derives an abstract completion from the child summary and calls
the stored parent frame's `resume` method:

```text
process(Resume { parent, child }):
    completion = completion_from_summary(summaries[child])
    effect = frames[parent].resume(completion, interpreter_for(parent))
    apply_effect(parent, effect)
```

This is still an abstract frame-transition graph conceptually, but the actual
data structure is a set of tables and reverse indexes.

## Generalized Abstract Addresses

Traditional k-CFA has addresses such as `(variable, call_string)` and
`(return, call_site, call_string)`. The generalized Kirin version is
`(frame_node_key, address_slot)`.

```rust
pub struct AbstractAddress<K> {
    pub owner: K,
    pub slot: AddressSlot,
}

pub enum AddressSlot {
    Ssa(SSAValue),
    BlockArgument(BlockArgument),
    FunctionParameter(usize),
    Return,
    Yield,
    FrameLocal(FrameSlot),
}
```

A call frame, block frame, function frame, or `scf.for` frame can own addresses
under the same allocation discipline.

## Context Strategy

The node key is context-sensitive. Classic k-CFA uses the last `k` call sites
as context. Here the same idea is generalized from call context to frame-node
context: the key stores the current `Location` plus a bounded history of parent
frame locations.

```rust
pub struct KContextStrategy {
    pub k: usize,
}

pub fn child_key(
    parent: &FrameNodeKey,
    child_location: Location,
    strategy: &KContextStrategy,
) -> FrameNodeKey {
    let mut entries = parent.context.entries.clone();
    entries.push(parent.location);

    if entries.len() > strategy.k {
        entries.remove(0);
    }

    FrameNodeKey {
        location: child_location,
        context: NodeContext { entries },
    }
}
```

For `k = 0`, all contexts collapse and the analysis is context-insensitive. For
`k = 1`, nodes are distinguished by the immediate parent frame location. For
larger `k`, nodes are distinguished by longer bounded traversal histories.
Call-site k-CFA is the special case where context entries are only call
locations.

## CFG Dependency Indexes

Forward and backward analyses can use different dependency indexes without
changing `AbstractState`:

```rust
pub struct ForwardCfgDeps<K, T> {
    pub successors_by_source: IndexMap<K, Vec<(K, T)>>,
}

pub struct BackwardCfgDeps<K, T> {
    pub predecessors_by_successor: IndexMap<K, Vec<(K, T)>>,
}
```

For forward constant propagation, if source block output changes, schedule its
successors. For backward liveness, if successor live-in changes, schedule its
predecessors. The dependency index points in the information-flow direction for
that analysis.

This is why `StatementEffect::Transfer(T)` is specialized per abstract driver.
Forward abstract interpretation can use:

```rust
pub enum ForwardTransfer<V> {
    Branch(Vec<BlockTransfer<V>>),
}
```

Backward analyses can use a transfer payload whose entries describe reverse
requirements rather than forward argument values:

```rust
pub enum BackwardTransfer<D> {
    Branch(Vec<D>),
}
```

## Summary

Every frame that owns or references an env should also have a corresponding
summary type. The baseline abstract driver is a greedy worklist fixpoint:
evaluate a node, produce candidate summaries, merge those summaries into the
node summary, and reschedule dependents only when the summary changed.

```rust
pub enum FixpointPhase {
    Join,
    Widen,
    Narrow,
}

pub trait Summary {
    type Strategy;

    fn merge(
        &mut self,
        phase: FixpointPhase,
        other: Self,
        strategy: &mut Self::Strategy,
    ) -> bool;
}
```

`merge` returns `true` if the summary changed. The phase tells the summary
whether the driver is doing ordinary joins, widening, or narrowing. Concrete
execution never sees this API.

The simplest strategy is greedy join-only fixpoint:

```rust
pub struct Greedy;

impl Summary for MySummary {
    type Strategy = Greedy;

    fn merge(
        &mut self,
        _phase: FixpointPhase,
        other: MySummary,
        _strategy: &mut Greedy,
    ) -> bool {
        self.join(other)
    }
}
```

The greedy driver always passes `FixpointPhase::Join`:

```rust
while let Some(item) = worklist.pop_front() {
    match item {
        WorkItem::Step(node) => {
            let candidates = evaluate_node(node)?;

            for candidate in candidates {
                let changed = summaries[node].merge(
                    FixpointPhase::Join,
                    candidate,
                    &mut Greedy,
                );

                if changed {
                    schedule_dependents(node);
                }
            }
        }
        WorkItem::Resume { parent, child } => {
            let completion = completion_from_summary(summaries[child]);
            let candidate = resume_parent(parent, completion)?;

            if summaries[parent].merge(FixpointPhase::Join, candidate, &mut Greedy) {
                schedule_dependents(parent);
            }
        }
    }
}
```

The summary type is composed like frames and completions. A language-level
abstract interpreter may define:

```rust
pub enum MySummary<V> {
    Block(BlockSummary<V>),
    Function(FunctionSummary<V>),
    Scf(ScfSummary<V>),
}
```

Summaries should also use lift/project algebra. A generic abstract driver can
hold the total summary type, while frame-specific code projects into its local
summary variant and bubbles unknown summary shapes when needed.

## Widening And Narrowing

Widening and narrowing are strategy layers over the same worklist driver shape.
They are independent of k-context-sensitive node allocation: k-CFA controls how
many node contexts are distinguished, while widening/narrowing controls how a
node summary is updated once a candidate summary reaches that node.

```rust
pub struct WidenNarrowStrategy {
    pub widen_after: usize,
    pub narrowing_steps: usize,
}
```

The driver may run a join phase first, switch selected nodes to widening after
their strategy threshold, and optionally run bounded narrowing passes. Those
policies belong to the abstract driver and summary strategy, not to
`FrameEffect` or concrete frame execution.

## Optional Deltas

Some analyses may prefer incremental deltas because a child frame only updates
one part of a larger summary. For example, a liveness block summary may store
both `live_in` and `live_out`, while a successor only contributes a new
`live_out` fact. That can be layered on top as an optional refinement:

```rust
pub trait ApplyDelta<D>: Summary {
    fn apply_delta(
        &mut self,
        phase: FixpointPhase,
        delta: D,
        strategy: &mut Self::Strategy,
    ) -> bool;
}
```

The baseline design remains summary merging. `Summary::merge` is the required
operation because it gives the fixpoint driver one uniform convergence point:
take a full candidate summary for a node, join/widen/narrow it into the stored
summary, and report whether anything changed.

A delta is different: it is an incremental fact that can be cheaper or clearer
to produce than a full summary. Applying a delta must still have the same
semantic outcome as merging the corresponding full candidate summary. Deltas are
therefore an optimization and ergonomic tool for structured analyses, not the
core summary protocol.
