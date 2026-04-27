# k-CFA Specialization

This document specializes the abstract framework in
[framework.md](framework.md) to traditional k-CFA. The important point is that
k-CFA does not require a public graph data structure. It is implemented as
finite maps keyed by bounded call strings, plus dependency indexes and
continuation entries.

## Traditional Shape

Traditional k-CFA bakes bounded context into allocation keys:

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CallString {
    pub calls: Vec<CallSite>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSite(pub Statement);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub usize);
```

Function summaries are keyed by function plus call string:

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FunctionContext {
    pub function: FunctionId,
    pub context: CallString,
}

pub struct FunctionSummary<V> {
    pub params: Vec<V>,
    pub returns: Vec<V>,
}
```

The transition rule for a call site pushes the call site into the bounded call
string:

```rust
fn enter_call(
    current_context: &CallString,
    call_site: CallSite,
    callee: FunctionId,
    k: usize,
) -> FunctionContext {
    let mut calls = current_context.calls.clone();
    calls.push(call_site);

    if calls.len() > k {
        calls.remove(0);
    }

    FunctionContext {
        function: callee,
        context: CallString { calls },
    }
}
```

With `k = 1`:

```text
call c1 -> id  gives FunctionContext { id, [c1] }
call c2 -> id  gives FunctionContext { id, [c2] }
```

With `k = 0`:

```text
call c1 -> id  gives FunctionContext { id, [] }
call c2 -> id  gives FunctionContext { id, [] }
```

## Framework Specialization

In Kirin, k-CFA is a specialization of the abstract framework:

```rust
pub type K = FunctionOwner;
pub type Token = Statement;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FunctionOwner {
    pub function: SpecializedFunction,
    pub context: NodeContext<Statement>,
}
```

The summary is function-shaped:

```rust
pub struct FunctionSummary<V> {
    pub params: Vec<V>,
    pub returns: Vec<V>,
}

pub enum KCfaSummary<V> {
    Function(FunctionSummary<V>),
}
```

The continuation store represents calls waiting for callees. A call push stores
the caller frame as a continuation under the callee owner:

```text
KontAddr {
    owner: FunctionOwner { callee, context: pushed_call_string },
    location: call_site_location,
    context: pushed_call_string,
}
```

## Push Policy

The k-CFA push policy handles function-call pushes.

```rust
pub enum KCfaResumeKind {
    ReturnToCall {
        call_site: Statement,
        results: Vec<SSAValue>,
    },
}
```

For a call from owner `caller` at call site `c1` into callee `id`, the policy
returns:

```rust
PushTransition {
    owner: FunctionOwner {
        function: id,
        context: push_context(&caller.context, c1, strategy),
    },
    token: c1,
    entry_candidate: Some(KCfaSummary::Function(FunctionSummary {
        params: abstract_argument_values,
        returns: bottom_return_values,
    })),
    resume_kind: Some(KCfaResumeKind::ReturnToCall {
        call_site: c1,
        results: call_result_slots,
    }),
}
```

Those four fields must be derived together. Splitting them across unrelated
policies risks making the callee owner context, continuation context, parameter
candidate, and return continuation disagree.

## Owner Entry

When a callee owner needs analysis, `OwnerSemantics::entry_frame` turns the
current function summary into a root frame:

```rust
impl OwnerSemantics<FunctionOwner, KCfaSummary<V>, F, C, KCfaResumeKind, E>
    for KCfaSemantics
{
    fn bottom_summary(
        &mut self,
        owner: &FunctionOwner,
    ) -> Result<KCfaSummary<V>, E> {
        // Create a function summary with the callee's parameter/result shape.
    }

    fn entry_frame(
        &mut self,
        owner: &FunctionOwner,
        summary: &KCfaSummary<V>,
    ) -> Result<F, E> {
        // Build the standard specialized-function frame using summary.params.
    }

    fn complete_owner(
        &mut self,
        owner: FunctionOwner,
        completion: C,
    ) -> Result<SummaryEffect<FunctionOwner, KCfaSummary<V>>, E> {
        // Project the root completion into an updated function summary.
    }

    fn completion_from_summary(
        &mut self,
        owner: &FunctionOwner,
        summary: &KCfaSummary<V>,
        kind: KCfaResumeKind,
    ) -> Result<C, E> {
        // Project summary.returns into a FunctionReturned completion.
    }
}
```

This frame creates or uses the function activation env according to function
semantics, then traverses the body with the standard block, region, graph, or
dialect-defined body frame.

Blocks inside the function do not automatically get their own summaries. They
share the function owner unless the analysis intentionally chooses finer
owners.

## Return Summary And Resume

When the callee owner reaches its root continuation, owner finalization returns
a candidate function summary:

```rust
SummaryEffect::Update {
    owner: callee_owner,
    candidate: KCfaSummary::Function(FunctionSummary {
        params,
        returns,
    }),
}
```

If merging that candidate changes the callee summary, the dependency index
wakes waiting continuations:

```rust
SummaryDependency::Resume {
    kont: caller_continuation,
    kind: KCfaResumeKind::ReturnToCall { ... },
}
```

`OwnerSemantics::completion_from_summary` turns the function summary into the
completion expected by the call frame.

The stored call frame resumes with that completion and writes abstract return
facts into the caller's activation summary.

## Dependency Index

The standard owner-dependency index is enough for the return direction:

```rust
pub struct OwnerDeps<K, Token, ResumeKind> {
    pub deps: IndexMap<K, IndexSet<SummaryDependency<K, Token, ResumeKind>>>,
}
```

When a call enters a callee owner, the driver registers:

```rust
deps.register(
    &callee_owner,
    SummaryDependency::Resume {
        kont: caller_continuation,
        kind: KCfaResumeKind::ReturnToCall { ... },
    },
)?;
```

When the callee summary changes, the dependency index emits resume
dependencies.

If the analysis also needs to rescan callers when argument summaries change,
that is represented as summary edges:

```rust
deps.register(
    &caller_owner,
    SummaryDependency::Reanalyze(callee_owner),
)?;
```

Those edges can be handled by a `ForwardSummaryDeps` index or a composite
dependency index.

## Address Allocation

Traditional k-CFA also allocates addresses under context:

```rust
pub enum KCfaAddress {
    Ssa {
        owner: FunctionOwner,
        value: SSAValue,
    },
    Return {
        owner: FunctionOwner,
        call_site: Statement,
    },
}
```

Kirin generalizes this from call contexts to arbitrary summary owners. The
owner can be a function, loop, graph, region, or any semantic boundary selected
by the analysis.

## Precision Tradeoff

For `k = 0`, all callers of a function share one function owner:

```text
FunctionOwner { id, [] }
```

For `k = 1`, each immediate call site gets a separate function owner:

```text
FunctionOwner { id, [c1] }
FunctionOwner { id, [c2] }
```

Larger `k` distinguishes longer call histories. This improves precision but
increases the number of summary owners and continuation entries.

The framework does not hard-code call-site context. `Token = Statement` gives
traditional k-CFA, while other analyses can use location, graph node, loop
owner, or custom semantic tokens.
