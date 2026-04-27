# Simple Owner-Local Fixpoint

This document specializes the abstract framework in
[framework.md](framework.md) to a simple owner-local fixpoint interpreter.

This is the smallest useful shape: the driver rebuilds a root frame for one
summary owner, evaluates that owner with a local concrete-like frame stack, and
merges the resulting candidate summary. It does not use the generalized
continuation store.

The goal is to show a baseline abstract interpreter that reuses:

- `Frame`
- `FrameEffect`
- `StatementEffect`
- `StatementDispatch`
- `Interpretable::interpret`
- `Env`

The important distinction is:

```text
frame          = traversal state
summary owner  = semantic convergence boundary
summary        = abstract state stored for that boundary
```

There is no required one-to-one mapping from frame to summary. A block frame can
exist without a block summary. A region frame can traverse many blocks while all
of those blocks share the same function summary. A dialect frame, such as
`scf.for`, can introduce its own summary when that statement is the right
convergence boundary.

## Shape

The abstract interpreter is a fixpoint shell. It repeatedly analyzes summary
owners, merges candidate summaries, and schedules owners whose summary changed.

```rust
pub struct SimpleFixpointInterpreter<'ir, Stage, F, C, E, V, T> {
    pub pipeline: &'ir Pipeline<Stage>,
    pub summaries: IndexMap<SummaryKey, EnvSummary<V>>,
    pub envs: AbstractEnvArena<V>,
    pub worklist: VecDeque<WorkItem>,
    pub phase: FixpointPhase,
    pub strategy: WidenNarrowStrategy,
    pub frame: PhantomData<F>,
    pub completion: PhantomData<C>,
    pub error: PhantomData<E>,
    pub transfer: PhantomData<T>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SummaryKey {
    pub location: Location,
}

pub enum WorkItem {
    Analyze(SummaryKey),
}
```

`SummaryKey` is not "the current frame location." It is the location of the
semantic owner whose summary should converge. The owner is chosen by the parent
statement/frame semantics.

Examples:

| Program shape | Summary owner | Frames that may run inside it |
| --- | --- | --- |
| function with block body | specialized function location | function frame, block frame, statement frames |
| function with CFG region body | specialized function location | function frame, region frame, many block frames |
| `scf.for` with block body | `scf.for` statement location | for frame, body block frame, statement frames |
| graph body | function, statement, or graph location chosen by semantics | graph frame plus node/body frames |

This is why the summary table is not a frame table. Frames define traversal.
Summary owners define convergence.

## Abstract Domain

The value type `V` is an abstract domain element.

```rust
pub trait AbstractValue: Clone + PartialEq {
    fn bottom() -> Self;

    fn join(&mut self, other: Self) -> bool;

    fn widen(&mut self, other: Self) -> bool {
        self.join(other)
    }

    fn narrow(&mut self, other: Self) -> bool {
        let _ = other;
        false
    }
}
```

`join`, `widen`, and `narrow` mutate `self` and return whether it changed.
Simple finite domains can use the default `widen` and `narrow`. Infinite-height
domains, such as intervals, override widening and narrowing.

## Abstract Env

The generic `Env<V>` trait still works, but env allocation in the abstract
interpreter has two different meanings:

- semantic activation allocation, which happens at call/function boundaries or
  explicit scope-introducing statements,
- scratch env allocation, which creates a temporary working copy of a summary
  while analyzing one owner.

The scratch env is an implementation detail of local transfer evaluation. It
does not mean that each block creates a new semantic activation.

```rust
#[derive(Clone, PartialEq, Eq)]
pub struct AbstractEnv<V> {
    pub values: IndexMap<SSAValue, V>,
}

impl<V> AbstractEnv<V>
where
    V: AbstractValue,
{
    pub fn bottom() -> Self {
        Self {
            values: IndexMap::new(),
        }
    }

    pub fn read(&self, value: SSAValue) -> V {
        self.values
            .get(&value)
            .cloned()
            .unwrap_or_else(V::bottom)
    }

    pub fn write(&mut self, value: SSAValue, data: V) {
        self.values.insert(value, data);
    }
}

pub struct AbstractEnvArena<V> {
    pub envs: SlotMap<EnvIndex, AbstractEnv<V>>,
}
```

Within one owner analysis, `write` is assignment into the scratch env. Merging
happens when a boundary produces a candidate for a summary owner.

For a function body, blocks normally share the same semantic activation. If the
body is a single block, only the function owner needs a summary. If the body is
a CFG region, all block frames in that region still share the function owner
summary unless the function/region semantics intentionally choose finer summary
owners.

## Summary

The simplest summary stores one abstract env for the semantic owner.

```rust
pub struct EnvSummary<V> {
    pub env: AbstractEnv<V>,
    pub visits: usize,
    pub narrow_visits: usize,
}
```

`env` is the joined abstract state at the owner boundary. What that means is
defined by the owner:

- for a function owner, it is the function activation summary,
- for an `scf.for` owner, it is the loop-carried/body-entry summary chosen by
  the SCF frame semantics,
- for a graph owner, it may summarize the graph traversal state chosen by the
  graph frame semantics.

The first version can use `EnvSummary<V>`. Later summaries can be richer than a
single env, for example a graph summary with per-node facts or a region summary
with internal block facts. That is an extension of the summary type, not a
change to the frame protocol.

In the full framework, `Summary::merge` returns `Option<Change>`. This simple
driver can collapse that to `bool` by using `Change = ()`.

## Merge Policy

Widening and narrowing are merge policies over summaries. They are not frame
effects.

```rust
pub enum FixpointPhase {
    Join,
    Widen,
    Narrow,
}

pub struct WidenNarrowStrategy {
    pub widen_after: usize,
    pub narrow_iterations: usize,
}
```

For the simple interpreter, apply widening at every summary owner after a visit
threshold. This is conservative and easy to implement. Later we can restrict
widening to loop-like owners such as function recursion points, CFG loop
headers, or `scf.for`.

```rust
fn merge_env_summary<V>(
    summary: &mut EnvSummary<V>,
    candidate: AbstractEnv<V>,
    phase: FixpointPhase,
    strategy: &WidenNarrowStrategy,
) -> bool
where
    V: AbstractValue,
{
    let use_widening = matches!(phase, FixpointPhase::Widen)
        && summary.visits >= strategy.widen_after;

    let mut changed = false;

    for (value, candidate_value) in candidate.values {
        let current = summary
            .env
            .values
            .entry(value)
            .or_insert_with(V::bottom);

        let value_changed = match phase {
            FixpointPhase::Join => current.join(candidate_value),
            FixpointPhase::Widen if use_widening => {
                current.widen(candidate_value)
            }
            FixpointPhase::Widen => current.join(candidate_value),
            FixpointPhase::Narrow => current.narrow(candidate_value),
        };

        changed |= value_changed;
    }

    changed
}
```

## Driver

Processing a summary owner constructs the root frame defined by that owner's
semantics, runs it with abstract values, and lets frames produce candidates for
summary owners.

```rust
impl<'ir, Stage, F, C, E, V, T>
    SimpleFixpointInterpreter<'ir, Stage, F, C, E, V, T>
where
    F: Frame<Self, F, C, E>,
    V: AbstractValue,
{
    pub fn solve(
        &mut self,
        entry: SummaryKey,
        input: AbstractEnv<V>,
    ) -> Result<(), E> {
        self.merge_summary_env(entry.clone(), input)?;

        self.phase = FixpointPhase::Widen;
        self.drain_worklist()?;

        if self.strategy.narrow_iterations > 0 {
            self.phase = FixpointPhase::Narrow;
            self.seed_narrowing_worklist();
            self.drain_bounded_narrowing()?;
        }

        Ok(())
    }

    fn drain_worklist(&mut self) -> Result<(), E> {
        while let Some(WorkItem::Analyze(owner)) = self.worklist.pop_front() {
            self.analyze_owner(owner)?;
        }

        Ok(())
    }
}
```

`FixpointPhase::Widen` includes ordinary joins before the threshold. This avoids
needing a separate pre-widening pass for the first version.

## Running Frames For An Owner

The fixpoint driver does not keep a global frame stack. Instead, each work item
builds a fresh root frame for one summary owner and runs it locally using the
same `Frame` protocol as the concrete interpreter.

```rust
fn analyze_owner(&mut self, owner: SummaryKey) -> Result<(), E>
where
    F: Frame<Self, F, C, E>,
{
    let summary_env = self.summaries[&owner].env.clone();
    let scratch_env = self.envs.alloc_scratch_from(summary_env);
    let root = self.frame_for_summary_owner(owner.clone(), scratch_env)?;

    let completion = self.run_local_frame(root)?;
    self.handle_owner_completion(owner.clone(), completion)?;
    self.summaries[&owner].visits += 1;
    self.envs.free(scratch_env)?;

    Ok(())
}

fn run_local_frame(&mut self, root: F) -> Result<C, E>
where
    F: Frame<Self, F, C, E>,
{
    let mut stack = vec![root];

    loop {
        let frame = stack.pop().ok_or(InterpreterError::EmptyFrameStack)?;
        let effect = frame.step(self)?;

        match effect {
            FrameEffect::Continue(frame) => stack.push(frame),
            FrameEffect::Push { parent, child } => {
                stack.push(parent);
                stack.push(child);
            }
            FrameEffect::Complete(completion) => match stack.pop() {
                Some(parent) => {
                    let effect = parent.resume(completion, self)?;
                    apply_local_effect(&mut stack, effect)?;
                }
                None => return Ok(completion),
            },
        }
    }
}
```

The local stack is an implementation detail of evaluating one owner. It is not
the abstract program state. The abstract program state is the summary table.

`frame_for_summary_owner` is semantic-specific:

- a function owner with a block body may build a function frame that pushes a
  block frame;
- a function owner with a CFG region body may build a function frame that
  pushes a region frame, and the region frame can traverse many blocks using the
  same scratch env;
- an `scf.for` owner may build a loop frame whose body traversal updates the
  loop summary;
- a graph owner may build a graph frame with graph-specific traversal order.

## Statement Dispatch

The shell implements `StatementDispatch` exactly like the concrete interpreter:
resolve the active statement and call dialect `Interpretable::interpret`.

```rust
impl<'ir, Stage, F, C, E, V, T> StatementDispatch<F, C, E, T>
    for SimpleFixpointInterpreter<'ir, Stage, F, C, E, V, T>
{
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E> {
        let statement = location.active_statement()?;
        let definition = self.definition_at(location.stage, statement)?;
        definition.interpret(self, location, env)
    }
}
```

Dialect code does not need a separate abstract trait. It specializes
`Interpretable` for this interpreter type and returns abstract transfers.

## Local Traversal Vs Summary Merge

`StatementEffect::Transfer(T)` is consumed by the active traversal frame. A CFG
branch does not automatically mean "merge into another summary node." The frame
that owns traversal decides what the transfer means.

For a function with a CFG region body, block transfers are local to the function
owner:

```rust
match interp.dispatch_statement(location, env)? {
    StatementEffect::Done => advance_to_next_statement_or_exit(),
    StatementEffect::Push(child) => push_statement_frame_with_child(child),
    StatementEffect::Complete(completion) => FrameEffect::Complete(completion),
    StatementEffect::Transfer(ForwardTransfer::Branch(edges)) => {
        for edge in edges {
            bind_block_args(env, edge.target, edge.args);
            schedule_or_enter_local_block(edge.target, env);
        }

        continue_region_traversal()
    }
}
```

No block summary is required here. The `RegionFrame` defines the abstract
traversal order for the CFG and reuses the same scratch env for the function
owner. If the region frame needs internal chaotic iteration over blocks, that is
region-frame state, not a requirement that every block become a top-level
summary owner.

For a statement that introduces its own convergence boundary, such as `scf.for`,
the statement/frame can merge into a separate summary owner:

```rust
let loop_owner = SummaryKey {
    location: Location {
        stage,
        position: Position::Statement {
            statement: for_statement,
        },
    },
};

let candidate = interp.envs.snapshot_loop_entry(env, loop_args);
interp.merge_summary_env(loop_owner, candidate)?;
```

This is the general rule:

```text
local transfer within current owner
    -> handled by the current traversal frame

transfer/call/loop/graph boundary that has its own summary
    -> merge candidate into that summary owner
```

## Scheduling

`merge_summary_env` is the only place that schedules new outer fixpoint work
for this simple design:

```rust
fn merge_summary_env(
    &mut self,
    owner: SummaryKey,
    candidate: AbstractEnv<V>,
) -> Result<(), E>
where
    V: AbstractValue,
{
    let summary = self
        .summaries
        .entry(owner.clone())
        .or_insert_with(EnvSummary::bottom);

    let changed = merge_env_summary(
        summary,
        candidate,
        self.phase,
        &self.strategy,
    );

    if changed {
        self.worklist.push_back(WorkItem::Analyze(owner));
    }

    Ok(())
}
```

This is intentionally simpler than the dependency-index design in the full
framework. The semantics of the parent statement/frame decide which owners
receive summary candidates. More precise dependency indexes can be used by the
k-CFA and dataflow shapes when interprocedural summaries, context sensitivity,
or incremental recomputation need tighter scheduling.

## Narrowing

Narrowing reuses the same worklist but changes the merge operator.

```rust
fn seed_narrowing_worklist(&mut self) {
    for owner in self.summaries.keys().cloned() {
        self.worklist.push_back(WorkItem::Analyze(owner));
    }
}

fn drain_bounded_narrowing(&mut self) -> Result<(), E> {
    for _ in 0..self.strategy.narrow_iterations {
        if self.worklist.is_empty() {
            break;
        }

        self.drain_worklist()?;
    }

    Ok(())
}
```

In the narrowing phase, `merge_env_summary` calls `V::narrow`. For interval
analysis, this can refine widened bounds after the widening phase reaches a
post-fixpoint. For finite domains, `narrow` can be the default no-op.

## How This Uses The Framework

The simple fixpoint interpreter does not need new abstract-specific traits.

- `Frame` is still the stepping/resume protocol.
- `FrameEffect` still describes local frame structure while evaluating one
  summary owner.
- `Env<V>` works because `V` is an abstract value.
- `StatementDispatch` still resolves statements and delegates to
  `Interpretable::interpret`.
- `StatementEffect::Transfer(T)` is where abstract control flow enters the
  traversal frame.
- Widening/narrowing live in `merge_summary_env`, not in dialect statement
  semantics.

This keeps the first abstract interpreter concrete and understandable. More
advanced designs, such as k-CFA, interprocedural summary reuse, or backward
analyses, can use the continuation store and dependency-index pieces from
[framework.md](framework.md) without changing the generic interpreter traits.
