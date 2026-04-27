# k-CFA Example

This file shows how the abstract interpreter state specializes to traditional
k-CFA. The important point is that k-CFA does not require a public graph data
structure. It is usually implemented as finite maps keyed by bounded call
strings, plus dependency indexes that decide what to revisit when summaries
change.

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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub usize);

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Addr {
    Var {
        var: VarId,
        context: CallString,
    },
    Ret {
        call_site: CallSite,
        context: CallString,
    },
}
```

The abstract interpreter stores joined facts:

```rust
pub struct KCfaStore<V> {
    pub values: HashMap<Addr, HashSet<V>>,
}

pub struct KCfaEnv {
    pub vars: HashMap<VarId, Addr>,
}
```

For a function-oriented version, summaries are keyed by function plus call
string:

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FunctionContext {
    pub function: FunctionId,
    pub context: CallString,
}

pub struct FunctionSummary<V> {
    pub params: Vec<HashSet<V>>,
    pub returns: HashSet<V>,
}

pub struct KCfa<V> {
    pub functions: HashMap<FunctionContext, FunctionSummary<V>>,
    pub calls: HashMap<(CallSite, CallString), HashSet<FunctionId>>,
    pub store: KCfaStore<V>,
    pub worklist: VecDeque<FunctionContext>,
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

The graph is implicit in the maps and worklist. If a summary changes, the
driver either rescans affected call sites or maintains dependency maps to know
what to revisit.

## Kirin Specialization

In the new interpreter design, k-CFA is a specialization of `AbstractState`.
`Deps` is call-specific instead of generic:

```rust
pub struct CallString {
    pub calls: Vec<Statement>,
}

pub struct CallNode {
    pub call_site: Statement,
    pub context: CallString,
}

pub struct FunctionNode {
    pub function: SpecializedFunction,
    pub context: CallString,
}

pub enum KCfaNode {
    Call(CallNode),
    Function(FunctionNode),
}

pub enum KCfaFrame<V> {
    Call(CallFrame<V>),
    Function(FunctionFrame<V>),
}

pub enum KCfaSummary<V> {
    Call(CallSummary<V>),
    Function(FunctionSummary<V>),
}

pub struct CallSummary<V> {
    pub args: Vec<V>,
    pub results: Vec<V>,
}

pub struct FunctionSummary<V> {
    pub params: Vec<V>,
    pub returns: Vec<V>,
}

pub struct KCfaDeps {
    pub callees_by_call: IndexMap<CallNode, Vec<FunctionNode>>,
    pub callers_by_function: IndexMap<FunctionNode, Vec<CallNode>>,
}

pub type KCfaState<V, Store> =
    AbstractState<KCfaNode, KCfaFrame<V>, KCfaSummary<V>, Store, KCfaDeps>;
```

`callees_by_call` means that if the call node's argument summary changes, each
callee function input summary may change. `callers_by_function` means that if
the function return summary changes, each call continuation may need to resume.

Processing a call installs both indexes:

```text
process_call(call, callee):
    function = FunctionNode {
        function: callee,
        context: push_call_string(call.context, call.call_site, k),
    }

    deps.callees_by_call[call].push(function)
    deps.callers_by_function[function].push(call)

    if merge_function_params(function, summaries[call].args):
        worklist.push(Step(KCfaNode::Function(function)))
```

When a call summary changes:

```text
on_call_changed(call):
    for function in deps.callees_by_call[call]:
        if merge_function_params(function, summaries[call].args):
            worklist.push(Step(KCfaNode::Function(function)))
```

When a function summary changes:

```text
on_function_changed(function):
    for call in deps.callers_by_function[function]:
        worklist.push(Resume {
            parent: KCfaNode::Call(call),
            child: KCfaNode::Function(function),
        })
```

Then the resume work item updates the call result summary:

```text
process(Resume { parent: Call(call), child: Function(function) }):
    returns = summaries[function].returns

    if merge_call_results(call, returns):
        schedule_dependents(KCfaNode::Call(call))
```

## Bounded Context

The context update is the usual k-CFA bounded call string:

```rust
pub fn push_call_string(
    context: &CallString,
    call_site: Statement,
    k: usize,
) -> CallString {
    let mut calls = context.calls.clone();
    calls.push(call_site);

    if calls.len() > k {
        calls.remove(0);
    }

    CallString { calls }
}
```

For `k = 1`, two calls into the same function get different function nodes:

```text
CallNode { c1, [] } -> FunctionNode { id, [c1] }
CallNode { c2, [] } -> FunctionNode { id, [c2] }
```

For `k = 0`, they share one function node:

```text
CallNode { c1, [] } -> FunctionNode { id, [] }
CallNode { c2, [] } -> FunctionNode { id, [] }
```

That is the usual k-CFA precision tradeoff. With `k = 0`, all callers of `id`
share one function summary. With `k = 1`, `id` gets one summary per immediate
call site. Larger `k` distinguishes longer call histories.

## Generalized Address Allocation

The Kirin generalization is to replace call-specific keys and addresses with
frame-node keys and frame-owned address slots:

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

Traditional k-CFA addresses such as `(variable, call_string)` and
`(return, call_site, call_string)` become `(frame_node_key, address_slot)`.

The k-CFA specialization uses call and function nodes as the frame-node keys.
The general interpreter can use the same allocation discipline for block
frames, region frames, call frames, function frames, `scf.for` frames, or any
dialect-defined frame with a meaningful location and summary.
