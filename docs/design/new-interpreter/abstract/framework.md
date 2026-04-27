# Abstract Interpreter Framework

This document defines the abstract-interpreter concepts that sit on top of the
generic interpreter traits in
[generic-interpreter-traits.md](../generic-interpreter-traits.md).

The abstract framework reuses the concrete frame protocol:

- `Frame`
- `FrameEffect`
- `StatementEffect`
- `StatementDispatch`
- `Interpretable::interpret`
- `Env`
- lift/project composition for frames, completions, errors, and summaries

It does not require a public graph data structure. A graph is a useful mental
model, but the implementation shape is table-based: summary tables,
continuation stores, dependency indexes, and worklists.

## Core Distinction

The central distinction is:

```text
frame          = traversal state
summary owner  = semantic convergence boundary
summary        = abstract facts stored for that boundary
continuation   = stored resume state for a waiting frame
```

There is no required one-to-one mapping from frames to summaries. A block frame
can exist without a block summary. A region frame can traverse many blocks
inside one function summary. A dialect frame, such as `scf.for`, can introduce
its own summary when that statement is the right convergence boundary.

Examples:

| Program shape | Summary owner | Frames that may run inside it |
| --- | --- | --- |
| function with block body | specialized function location | function frame, block frame, statement frames |
| function with CFG region body | specialized function location | function frame, region frame, many block frames |
| `scf.for` with block body | `scf.for` statement location | for frame, body block frame, statement frames |
| graph body | function, statement, graph, or graph-node owner chosen by semantics | graph frame plus node/body frames |

## Driver Parameters

An abstract driver is generic over the same total frame, completion, error, and
transfer types as concrete interpretation, plus abstract-specific table keys:

```rust
pub struct AbstractInterpreter<
    'ir,
    Stage,
    F,
    C,
    E,
    T,
    K,
    S,
    Token,
    ResumeKind,
    Store,
    Deps,
>
where
    K: Clone + Eq + Hash,
    S: Summary,
    Deps: SummaryDependencyIndex<K, Token, S::Change, ResumeKind, E>,
{
    pub pipeline: &'ir Pipeline<Stage>,
    pub summaries: IndexMap<K, S>,
    pub store: Store,
    pub konts: KontStore<K, Token, F>,
    pub deps: Deps,
    pub worklist: VecDeque<AbstractWork<K, Token, F, C>>,
    pub transfer: PhantomData<T>,
    pub resume: PhantomData<ResumeKind>,
}
```

The important parameters are:

- `F`: total frame type.
- `C`: total completion type.
- `E`: total error type.
- `T`: transfer type used by `StatementEffect::Transfer(T)`.
- `K`: summary owner key.
- `S`: total summary type.
- `Token`: bounded context token.
- `ResumeKind`: identifies how to turn a summary into a completion for a
  waiting continuation.
- `Store`: driver-specific abstract store, if the analysis needs one.

`K`, `S`, `Token`, `ResumeKind`, and `Store` are where analyses specialize. The
generic frame protocol does not change.

## Summary Owners

The owner key `K` identifies a semantic convergence boundary.

The interpreter crate can provide a standard helper:

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SummaryKey<Token = Location> {
    pub location: Location,
    pub context: NodeContext<Token>,
}
```

`SummaryKey` is only a convenience. The driver remains generic over `K`.

Common owner choices:

- k-CFA: context-specialized function owner.
- constant propagation: function owner, or finer CFG loop-header owners if
  needed.
- liveness: function, block, or region owner depending on traversal strategy.
- SCF: `scf.for` owner for loop-carried convergence.
- graph analyses: graph or graph-node owner selected by graph semantics.

## Context Tokens

Bounded context is represented by generic tokens:

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodeContext<Token> {
    pub entries: Vec<Token>,
}

pub struct ContextStrategy {
    pub k: usize,
}

pub fn push_context<Token: Clone>(
    context: &NodeContext<Token>,
    token: Token,
    strategy: &ContextStrategy,
) -> NodeContext<Token> {
    let mut entries = context.entries.clone();
    entries.push(token);

    if entries.len() > strategy.k {
        entries.remove(0);
    }

    NodeContext { entries }
}
```

With `k = 0`, contexts collapse. With `k = 1`, the latest token distinguishes
owners or continuations. Larger `k` keeps a bounded history.

Traditional k-CFA is the special case where `Token = Statement` and tokens are
call sites. Kirin generalizes the token from call sites to arbitrary IR nodes
or semantic events.

## Abstract Stores

Some abstract interpreters implement the generic `Env<V>` trait over abstract
values. Others use summary-only state, graph-local fact tables, or a traditional
k-CFA store. The abstract framework therefore treats storage as driver-specific
state rather than requiring an env-shaped core store.

When an analysis does use an abstract env, it should still distinguish:

- semantic activation allocation, created by calls/functions or explicit
  scope-introducing statements,
- scratch env allocation, created by the driver while evaluating one owner.

Blocks and regions normally share the current function activation env. A block
summary is not created just because a block frame exists.

The store is part of analysis semantics, not frame identity. A function summary
may store one activation env. A graph summary may store per-node facts. An SCF
summary may store loop-carried facts.

## Summary Protocol

Summaries are merged at owner boundaries. Widening and narrowing are summary
merge policies; they are not frame effects.

```rust
pub enum FixpointPhase {
    Join,
    Widen,
    Narrow,
}

pub trait Summary {
    type Strategy;
    type Change;

    fn merge(
        &mut self,
        phase: FixpointPhase,
        candidate: Self,
        strategy: &mut Self::Strategy,
    ) -> Option<Self::Change>;
}
```

`None` means no observable summary change. `Some(change)` is passed to the
dependency index.

Change events are summary-specific. A total summary enum composes them with the
same lift/project style used by frames, completions, and errors:

```rust
pub enum MySummaryChange {
    Env(()),
    Function(FunctionSummaryChange),
    Scf(ScfSummaryChange),
    Graph(GraphSummaryChange),
}
```

## Owner Lifecycle

Every summary owner is initialized through one idempotent driver operation:

```rust
fn ensure_owner(&mut self, owner: K) -> Result<(), E> {
    if self.summaries.contains_key(&owner) {
        return Ok(());
    }

    let summary = self.owner_semantics.bottom_summary(&owner)?;
    self.summaries.insert(owner.clone(), summary);
    self.konts.ensure_root(owner.clone());
    self.deps.ensure_owner(&owner)?;

    Ok(())
}
```

Owner semantics creates the initial summary because `S::bottom()` is often not
enough. A function summary may need parameter and result arity. An `scf.for`
summary may need loop-carried value shape. A graph summary may need graph-local
fact layout.

```rust
pub trait OwnerSemantics<K, S, F, C, ResumeKind, E> {
    fn bottom_summary(&mut self, owner: &K) -> Result<S, E>;

    fn entry_frame(&mut self, owner: &K, summary: &S) -> Result<F, E>;

    fn complete_owner(
        &mut self,
        owner: K,
        completion: C,
    ) -> Result<SummaryEffect<K, S>, E>;

    fn completion_from_summary(
        &mut self,
        owner: &K,
        summary: &S,
        kind: ResumeKind,
    ) -> Result<C, E>;
}
```

`complete_owner` is called when execution reaches the root continuation for an
owner. `entry_frame` starts or reanalyzes an owner. `completion_from_summary`
turns an updated summary into the completion expected by a waiting
continuation.

## Continuation Store

Concrete interpretation keeps a precise stack:

```rust
Vec<F>
```

Abstract interpretation uses a finite continuation store:

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct KontAddr<K, Token> {
    pub owner: K,
    pub location: Location,
    pub context: NodeContext<Token>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct KontEntryId(usize);

pub struct KontStore<K, Token, F> {
    pub interner: KontInterner<K, Token, F>,
    pub frames: IndexMap<KontAddr<K, Token>, IndexSet<KontEntryId>>,
}

pub enum AbstractKont<K, Token, F> {
    Empty,
    Frame {
        frame: F,
        parent: KontAddr<K, Token>,
    },
}
```

Continuation identity is:

```text
summary owner + resume location + bounded continuation context
```

The store holds total frames `F`. Entries are interned so the store has set-like
behavior and does not grow forever with duplicate continuation frames.

```rust
pub trait FrameInternKey {
    type Key: Eq + Hash;

    fn frame_intern_key(&self) -> Self::Key;
}
```

The first implementation should use conservative structural keys. Later, frames
can provide smaller specialized resume keys.

Every owner has a root empty continuation:

```rust
impl<K, Token> KontAddr<K, Token> {
    pub fn root(owner: K) -> Self {
        Self {
            owner,
            location: Location::root(),
            context: NodeContext { entries: Vec::new() },
        }
    }
}
```

`AbstractKont::Empty` means completion reached the root of an owner traversal.

## Work Items

The worklist operates on frames and completions paired with continuation
addresses:

```rust
pub enum AbstractWork<K, Token, F, C> {
    Step {
        frame: F,
        kont: KontAddr<K, Token>,
    },
    Complete {
        completion: C,
        kont: KontAddr<K, Token>,
    },
}
```

The active owner is `kont.owner`.

## Push Policy

`FrameEffect::Push { parent, child }` means only:

```text
parent waits for child
```

It does not decide whether the child stays in the same summary owner or enters
a new owner. The abstract driver delegates that to one unified push policy.

```rust
pub struct PushTransition<K, Token, S, ResumeKind> {
    pub owner: K,
    pub token: Token,
    pub entry_candidate: Option<S>,
    pub resume_kind: Option<ResumeKind>,
}

pub trait AbstractPushPolicy<K, Token, F, S, ResumeKind, E> {
    fn push_transition(
        &mut self,
        current_owner: &K,
        parent: &F,
        child: &F,
    ) -> Result<PushTransition<K, Token, S, ResumeKind>, E>;
}
```

The policy is unified because k-CFA needs owner choice, call-string token,
callee entry facts, and caller resume behavior to come from the same call
event.

Push handling order:

1. ask the push policy for the transition,
2. ensure the target owner exists,
3. allocate the child continuation address,
4. insert the stored parent continuation and register a resume dependency,
5. merge any entry candidate into the target owner,
6. schedule the child directly only if the push stayed in the same owner.

Same-owner pushes may step the child directly. New-owner pushes merge an entry
candidate and let summary scheduling start the owner from its root entry frame.

## Summary Effects

Owner finalization returns summary effects. The driver applies them with the
same merge/widen/narrow policy used for all summary updates.

```rust
pub enum SummaryEffect<K, S> {
    None,
    Update {
        owner: K,
        candidate: S,
    },
    Many(Vec<(K, S)>),
}
```

This keeps convergence policy centralized in the driver.

## Dependency Index

After a summary changes, the dependency index decides all follow-up work. The
driver should not hard-code self reanalysis as a special case.

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum SummaryDependency<K, Token, ResumeKind> {
    Reanalyze(K),
    Resume {
        kont: KontAddr<K, Token>,
        kind: ResumeKind,
    },
}

pub trait SummaryDependencyIndex<K, Token, Change, ResumeKind, E> {
    fn ensure_owner(&mut self, owner: &K) -> Result<(), E>;

    fn register(
        &mut self,
        trigger_owner: &K,
        dependency: SummaryDependency<K, Token, ResumeKind>,
    ) -> Result<(), E>;

    fn on_summary_changed(
        &mut self,
        owner: &K,
        change: Change,
    ) -> Result<Vec<SummaryDependency<K, Token, ResumeKind>>, E>;
}
```

Standard dependency indexes can be provided as helpers:

- `OwnerDeps<K, Token, ResumeKind>` maps an owner to dependencies to wake.
- `ForwardSummaryDeps<K>` registers `Reanalyze(successor)` dependencies.
- `BackwardSummaryDeps<K>` registers `Reanalyze(predecessor)` dependencies.
- `CompositeDeps<K, Token, ResumeKind>` combines several indexes.

Completion creation is part of `OwnerSemantics`. For k-CFA, a callee function
summary becomes a function-return completion for the call frame waiting at the
call site. For SCF, a loop summary can become a loop completion for the frame
waiting on the loop owner.

## Statement Transfers

`StatementEffect::Transfer(T)` remains specialized to the interpreter/frame
family.

Concrete interpreters can use only `Jump`. Forward abstract interpreters can
use `Branch`, with one edge for an unconditional jump. Backward analyses can
use a transfer payload whose edges point in the backward information-flow
direction.

The transfer payload does not automatically create summary owners. The active
frame decides whether a transfer is local traversal inside the current owner or
a boundary that should merge a candidate into another summary owner.

## Design Rules

- Frames define traversal.
- Summary owners define convergence.
- Env allocation is semantic, not per-block by default.
- Summary merge owns join/widen/narrow.
- Dependency indexes own rescheduling.
- `FrameEffect` stays unchanged across concrete and abstract drivers.
- Use-case-specific precision belongs in `K`, `S`, `Token`, `ResumeKind`,
  `T`, dependency indexes, and frame families.
