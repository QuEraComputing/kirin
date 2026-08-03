# Interpreter Framework

The interpreter framework (`kirin-interpreter`) supports concrete execution
and lattice-based abstract interpretation over the same dialect semantics,
including analyses that cross language boundaries in multi-stage pipelines.

The design is organized as a **two-persona contract**:

- **Dialect authors** describe what each statement *means*, once, in a small
  fixed vocabulary — `Interpretable<I, ForwardEval>`/`SparseForwardInterp`/`SparseForwardEffect`,
  receiving the engine directly and selecting semantics by a compile-time semantic
  marker. There is no framework
  "scope": a statement whose operation owns structured control runs a
  sub-computation by *pushing a frame the dialect owns* (`SparseForwardEffect::Push`),
  built per-engine through a small dialect dispatch capability. Ordinary
  (non-control) dialects never push frames at all.
- **Compiler authors** compose languages into pipelines and *select*
  components: an engine, a value domain, an error type, and a linker. When they
  need more control, the same compiler-author surface also includes opt-in
  traversal and analysis components: custom concrete frames
  (`ConcreteInterpreter<.., F>`), custom forward-dataflow frames
  (`SparseForwardInterpreter<.., P, F>`), and abstract policies `P`
  (`CallContext` + `WideningStrategy`). A language that uses a structured-control
  dialect composes its own total frame type embedding the standard frames plus
  that dialect's frames. Ordinary dialects never name a frame type.

Every derive macro is named after the trait it implements
(`#[derive(Interpretable)]` → `trait Interpretable`), so learning the derive
is learning the trait.

## Dialect-author surface

Everything is exported from `kirin_interpreter::dialect`.

### `Interp` and `Interpretable<I, Semantics>` — statement semantics

```rust
pub trait Interp: Sized {               // the engine-side driver — ANALYSIS-AGNOSTIC
    type Value: Clone;                  // the value domain
    type Error: From<InterpreterError>; // the total error
    type Effect;                        // analysis-specific per-statement effect
    type Semantics: SemanticKey;        // compile-time semantic key (ForwardEval, ...)
    fn stage(&self) -> CompileStage;    // the current statement's location, set by the engine
    fn statement(&self) -> Statement;   //   before each dispatch and read back by rules
    fn index(&self) -> EnvIndex;        //   (the SSA activation)
}

// SSA environment access used by forward engines.
pub trait Env: Interp {
    fn env_read(..) -> Result<Self::Value, Self::Error>;
    fn env_write(..) -> Result<(), Self::Error>;
}
```

```rust
// The single dialect trait — specialized on the engine `I` and a compile-time
// semantic key `Semantics` (NOT a runtime context object, NOT a solver shape).
pub trait Interpretable<I: Interp, Semantics>: Dialect {
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error>;
}
```

A rule receives the engine `interp: &mut I` directly. Forward rules bound
`I: SparseForwardInterp`, which provides the read/write helpers — `read`,
`read_many`, `write`, `write_results` — as **default methods** that operate on the
engine's current activation (`interp.index()`) and delegate to the engine's
[`Env`] storage access. So a forward rule calls `interp.read(..)` / `interp.write(..)`
directly. There is **no** `ValueContext`/`ForwardCtx` object: the engine *is* the
context.

`ForwardEval` is a pure compile-time **semantic key** — never instantiated — that
declares its solver shape (`SemanticKey::Shape = SparseForwardShape`; shapes name
mechanics only and are never dispatch tags) —
that selects *which* semantics an impl describes. It names *forward evaluation*
(read operands, compute a semantic/lattice value, write results), so one mode
covers concrete execution, constant propagation, and interval analysis — they
differ only in the value domain, not the rule shape. It is deliberately not
`ForwardValue`: a future forward **type inference** mode also attaches facts to
SSA values but should expose a different rule API, so the name reflects the
evaluation semantics rather than "operates on values". A forward statement rule
is `impl<I: SparseForwardInterp, ..> Interpretable<I, ForwardEval> for Op`: it
reads/writes through `interp.read`/`interp.write` and returns `I::Effect`
(= `SparseForwardEffect`). The backward analyses follow the same shape with
their own markers, engine traits, and effects — the impls coexist on the same
dialect type because the `Semantics` parameter distinguishes them (no coherence
conflict):

- `StrongDemand` (on `SparseBackwardShape`) — engine surface split in two:
  the shape-generic `SparseBackwardInterp` (`fact`/`raise_fact`/`effect` plus
  the block-topology queries) serves any sparse-backward key, and
  `DemandInterp` (pinned to `StrongDemand` via its supertrait) adds the demand
  vocabulary. `raise_fact` takes the lattice element to merge, so the shape
  moves facts without inspecting them; ⊤ is `demand`'s business, and `HasTop`
  rides on `DemandInterp`'s supertrait so rules never spell it. Rules bind `DemandInterp`: read converged facts (`is_demanded`),
  raise demands (`demand`), and end with `interp.effect()`
  (= `SparseBackwardEffect`); ordinary dialects are the one-liner
  `interp.demand_uses_if_observable(self)` (purity-aware neededness via `IsPure`).
- `ClassicLiveness` (on `DenseBackwardShape`) — likewise split: the
  shape-generic `DenseBackwardInterp` (`point_state`/`point_state_mut`, which
  hand the state over opaquely) serves any dense-backward key, and
  `ClassicLivenessInterp` adds liveness's spellings — `PointFacts` ("a state is
  a set of live values") is that key's contract, required by no engine and no
  frame. What the shape *does* need of a state is `Lattice` for merges and
  `DenseBackwardState` (`rename`/`forget`) for crossing edges and leaving
  scopes; the parameter-to-argument substitution the CFG edge transfer and
  `scf.for`'s back-edge both perform lives in those two methods, implemented
  for `LiveSet` in `kirin-liveness`. Rules bind `ClassicLivenessInterp`: `gen_live`/
  `kill_def`; ordinary dialects (and calls — purity is irrelevant to dense
  sets) are `interp.gen_uses_kill_defs(self)`; CFG terminators name their edges
  (`Edges`, in `DenseBackwardEffect`), structured dialects push dense frames
  (`Push`).

The engines/transfers are generic over the semantic key with the canonical
default (`SparseForwardTransfer<..., Sem = ForwardEval>`,
`SparseBackwardInterpreter<..., Sem = StrongDemand>`,
`DenseBackwardInterpreter<..., Sem = ClassicLiveness>`; `Sem` bounded by the
shape's *family* trait, e.g. `SparseForwardSemantic`), so a downstream key
reuses an engine by instantiating `Sem` — and `InterpDispatch<I>` always
dispatches `I::Semantics`, so a stage can never be paired with a foreign
key's rules.

Future sibling modes (not yet implemented) each get their own marker + engine
trait the same way, e.g. `ForwardType` (forward type inference) or
a key on `DenseForwardShape` (typestate); two keys may also share an existing
shape without colliding.

`Interp` is the interpreter/analysis **driver**: it exposes the value domain, the
error type, the per-statement effect, the semantic key `Semantics`, and the current
statement location (`stage()`/`statement()`/`index()`). The engine stashes the
location before dispatching each rule (`run_statement`/`enter_function`) and
restores it afterward, so a rule can read it back without a separate context
object. A rule produces `I::Effect` — the **analysis-specific** effect algebra —
not a single universal enum. (The frame type stays the engine's own `F` generic,
e.g. `ConcreteInterpreter<.., F>`, so traversal is customizable without an unused
associated type on `Interp`.) Forward rules bound `I: SparseForwardInterp`, the
flavor of `Interp` whose `Semantics = ForwardEval` and `Effect = SparseForwardEffect<I::Value,
I::Frame>`, so they build and return `SparseForwardEffect` values (which are `I::Effect`).
`I::Frame` is the engine's total frame type, re-exposed by `SparseForwardInterp`
only so a structured dialect can name the frame it pushes; ordinary dialects never
mention it (it is inferred from `I::Effect`). They constrain only:

- the value domain, with plain Rust bounds — `I::Value: Add<Output = I::Value>`
  (kirin-arith), `I::Value: BranchCondition` (kirin-cf), `I::Value:
  ForLoopValue` (kirin-scf);
- error lifting — `I::Error: From<DivisionByZero>`.

Because the impl is generic over the value domain, **one transfer rule serves
both execution and analysis**: `kirin-arith`'s `Add` rule computes `3 + 5`
under `ConcreteInterpreter<.., i64, ..>` and folds `Const(3) + Const(5)`
under constant propagation, with no analysis-specific code in the dialect.

`SparseForwardInterp` is the **forward engine** trait: it requires `Env` and
`Semantics = ForwardEval`, and exposes the SSA read/write helpers as **default
methods**, hiding environment indices and locations: `interp.read(ssa)`,
`interp.write(result, value)`, `interp.read_many(&values)`,
`interp.write_results(&results, product)`. They delegate to the engine's [`Env`]
storage access (`env_read`/`env_write`) at the engine's current activation
(`interp.index()`). A structured dialect calls its own dispatch capability
(e.g. `interp.scf_if_frame(..)`) to build the frame it pushes (see SCF below).
The backward engine traits chose their own helper APIs and effects the same way
— per-value facts for `SparseBackwardInterp` (with `DemandInterp` sugar),
point states for `DenseBackwardInterp` (with `ClassicLivenessInterp` sugar) —
without adding variants to `SparseForwardEffect`.

### `SparseForwardEffect` — the forward control algebra

This is the `Effect` for the *forward* mode (`SparseForwardInterp::Effect`). It is **one
algebra among potential several**: a future analysis defines its own `I::Effect`
rather than adding variants here.

```rust
pub enum SparseForwardEffect<V, F> {
    Next,                                          // atomic statement done
    Jump(Edge<V>),                                 // decided CFG transfer
    Branch(Vec<Edge<V>>),                          // undecided CFG transfer
    Call(CallEffect<V>),                           // function invocation (resolved by the linker)
    Yield(Product<V>),                             // terminate the innermost body block
    Return(Product<V>),                            // return from the enclosing function
    Push { frame: F, results: Product<SSAValue> }, // run a dialect-owned frame; bind its finish values
}
```

`F` is the engine's total frame type. The frame-free variants don't name it, so
ordinary dialects never see it; only a dialect whose operations own structured
traversal builds `Push` (naming the frame via `SparseForwardInterp::Frame`). The pushed
`frame` is whatever traversal the dialect decided on — there is no framework-owned
"scope", and no framework "explore alternatives" effect (a dialect frame that
needs to explore several bodies pushes them one at a time and joins itself).

`Branch` encodes the concrete/abstract split *in the value domain* for cf-style
CFG transfers: a dialect asks the value (`BranchCondition::is_truthy() ->
Option<bool>`) and emits `Jump` when decided, `Branch` when not. Concrete engines
reject `Branch` (`IndeterminateBranch`); the abstract CFG frame explores every
edge and joins. Control dialects pass the same `Option<bool>` to their own frame
(see SCF below). Dialects therefore have exactly one `Interpretable` impl and no
knowledge of which engine is running.

### Structured control flow — dialect-owned frames

The framework has no "scope" type. A dialect whose operation owns structured
traversal builds a **frame it owns** (per-engine, through a small dialect
dispatch capability) and returns it as `SparseForwardEffect::Push { frame, results }`.
SCF has two such operations:

- **`scf.if`** → `kirin_scf::ScfIfFrame` (concrete) / `AbstractScfIfFrame`
  (abstract), built via `ScfIfDispatch::scf_if_frame(.., decided)`. The rule
  reads the condition value and hands the `Option<bool>` decision to the frame;
  the **frame** picks the arm (concrete; undecided is `IndeterminateBranch`) or
  explores both arms and **joins** their finish results (abstract). It walks each
  arm by pushing the framework `BlockFrame`/`AbstractBlockFrame` building block,
  consumes the arm's `Completion::Yielded` values, and relays a bubbled
  `Completion::Returned` unchanged toward the nearest `CallFrame`.

- **`scf.for`** → `ScfForFrame` / `AbstractScfForFrame`, built via
  `ScfForDispatch`. The frame pushes a body frame each iteration, advances the
  induction variable on each yield, and decides repeat/finish in the value
  domain. The **loop-carried fixpoint lives in the abstract loop frame**: it
  joins (then widens) the entry state across iterations until stable,
  accumulating finish values across exits — so `scf.for` over a lattice
  converges, with no framework "scope hook".

The framework `BlockFrame`/`AbstractBlockFrame` (single-block body walkers,
surfacing `Yield` to their parent) are reusable **building blocks**, not
framework-owned structured semantics: the SCF frames build them to walk a
chosen body, but the structured *decision* and result binding stay in the SCF
frame. A language that
uses SCF composes a total frame type embedding the standard frames plus
`ScfIfFrame`/`ScfForFrame` (via `BuildScfIf`/`BuildScfFor` and the abstract
equivalents); see `example/toy-lang`'s `ToyFrame`/`ToyAbstractFrame`. Future
structured dialects would follow the same pattern; only the existing SCF
operations are implemented.

### `FunctionEntry<I>` — callable statements

```rust
pub trait FunctionEntry<I: Interp>: Dialect {
    fn function_entry(&self, args: Product<I::Value>, interp: &mut I)
        -> Result<CallableBody<I::Value>, I::Error>;
}
```

Like `Interpretable`, it receives the engine `interp` directly (function entry is
forward-only, so there is no `Semantics` parameter).

Statements that define function bodies (e.g. `kirin_function::Function`)
return the `CallableBody { body, args }` to enter on invocation (the
function-call entry descriptor — not a structured-control abstraction). On
language enums it is derived; `#[callable]` marks the variants that forward, all
others report `NotCallable`.

## Compiler-author surface

Everything is exported from `kirin_interpreter::engine`. Compiler authors
usually write zero framework-trait impls:

1. **Language enums** — the same `#[wraps]` enums used for parsing/printing,
   with `Interpretable` (and `FunctionEntry` + `#[callable]`) added to the
   derive list.
2. **Stage enum** — add `#[derive(InterpDispatch)]` next to `StageMeta` and
   `ParseDispatch`. Single-language pipelines (`Pipeline<StageInfo<L>>`) get a
   blanket impl.
3. **Value and error types** — plain Rust: a value type with the operator
   impls the dialects need, an error enum with `From` impls for
   `InterpreterError` and the dialect errors in use.
4. **Engine + linker** — components selected by value:

```rust
let mut interp = ConcreteInterpreter::<Stage, i64, ToyError>::new(&pipeline)
    .with_linker(CrossStageLinker);
let result = expect_single(interp.call_by_name("source", "main", [3, 5])?)?;

let mut analysis = ConstProp::<Stage, ToyError>::new(&pipeline)
    .with_linker(CrossStageLinker);
let value = expect_single(analysis.analyze_by_name("source", "abs", [Const(7)])?)?;
```

### Linkers: calling conventions as components

```rust
pub trait Linker<S: StageMeta> {
    fn resolve(&self, pipeline: &Pipeline<S>, caller_stage: CompileStage, callee: &Callee)
        -> Result<FunctionTarget, InterpreterError>;
}
```

A linker resolves `Callee::{Named, Function, Staged, Specialized}` to a
`(stage, specialization, body)` target. It is a *field of the engine*, never
a trait the user implements on the engine type — this is a deliberate
coherence rule: policies must be swappable without newtype-cloning a driver.

- `SameStageLinker` (default): resolve within the caller's stage.
- `CrossStageLinker`: prefer a live specialization at the caller's stage,
  otherwise any stage that has one.

Because the linker is shared by all engines, cross-language *analysis* is the
same one-line choice as cross-language *execution*: the abstract engine calls
the linker at `SparseForwardEffect::Call`, and the analysis lattice flows through
`Product<V>` function summaries regardless of which language the callee
belongs to.

## Engines

### `ConcreteInterpreter<'ir, S, V, E, Lk, F = StandardFrame<V, E>>`

A generic **frame-stack driver**: it pops the top frame, calls `Frame::step`,
and applies the returned `FrameEffect` (`Continue` / `Push` / `Done` /
`Complete`) — it owns *no* traversal logic itself. Traversal lives in the
frames, organized along two independent axes:

- **Body representation** (the closed `Body` vocabulary — an intentional IR
  design decision): each framework-walkable representation has one
  *representation walker* owning traversal mechanics only — `CFGFrame`
  (multi-block, follows `Jump`, rejects an undecided `Branch`), `BlockFrame`
  (one linear block; `Jump`/`Branch` are errors), and `DiGraphFrame`
  (dependency-ordered DAG walk collecting the declared yields). `UnGraph` has
  **no default walker**: an undirected graph has no inherent execution order,
  so callable-UnGraph traversal is a dialect/compiler-supplied policy
  (`FrameBuild::from_ungraph_entry`, defaulting to `NoDefaultWalker`).
- **Entry context**: the same walker serves a *callable* body (entered
  through `CallFrame`) and a *nested structured-operation* body (entered
  through a dialect frame); analysis owners are the abstract engines' third
  context. Walkers never know their role — they surface exits through the
  completion protocol (`Completion::Returned` for a function `Return`,
  `Completion::Yielded` for a structured `Yield`, `Completion::Finished` for
  natural completion such as a digraph's output yields) and the parent frame
  decides what each means.

`CallFrame` is the **call boundary**: it resolves the callee, allocates the
callee activation, selects the entry walker for the closed `Body` variant,
validates the completion kind (`Returned`, or a graph's natural `Finished`;
a structured `Yielded` is an error), frees the callee activation exactly
once, and delivers the values — into the caller's result slots, or as the
run's result for a root call (`ConcreteInterpreter::call` pushes a
`CallFrame::root`, so root and nested calls share one boundary
implementation). Representation walkers never free activations; a `Returned`
bubbles through dialect frames to the nearest `CallFrame`.

The default total frame type `StandardFrame<V, E>` bundles the three walkers
plus `CallFrame`. The dialect-produced `SparseForwardEffect` is consumed by
the walkers, which map it to a `FrameEffect` (handling `Push` by pushing the
carried frame). `StandardFrame` is structured-control-free; a custom `F`
([Custom traversal and policies](#custom-traversal-and-policies)) adds dialect
frames or replaces traversal without touching the engine.

### `SparseForwardInterpreter<'ir, S, V, E, Lk, P = ContextInsensitive, F = StandardAbstractFrame<..>>`

The **forward dataflow** engine — a lattice-based forward abstract interpreter,
and one *specialization* of the shared framework in the forward direction (it sets
`Effect = SparseForwardEffect` and `Semantics = ForwardEval`, stores SSA activations via
`Env`, and drives forward frames). The name
`AbstractInterpreter` is reserved for the shared trait implemented by
lattice-valued abstract engines. `SparseForwardInterpreter` is the forward
engine; `SparseBackwardInterpreter` (per-SSA demand / strong liveness) and
`DenseBackwardInterpreter` (classic per-point liveness) are the backward
specializations — each with its own fact store, effect, and engine-capability
capability, reusing the same framework (fixpoint driver + `*Transfer` inner
`Interp`) and also implementing `AbstractInterpreter`.

Interprocedural fixpoint analyzer over a lattice `V: Widen + Lattice +
HasBottom`. Reads of unbound SSA values are `bottom` (unreached). Like the
concrete engine, it is a generic **frame-stack driver**: the total abstract
frame type `F` (default `StandardAbstractFrame`) owns the traversal — CFG block
worklist, branch exploration, scope fixpoints, and call summarization — and the
engine just runs the stack (`run_frames`). A custom `F`
([Custom traversal and policies](#custom-traversal-and-policies))
customizes/observes abstract traversal without forking the engine. The
*orthogonal* analysis policy `P` (`CallContext` for summary keys +
`WideningStrategy` for join/widen, default `ContextInsensitive`) controls
keying and merge; the interprocedural protocol (summary tables, caller
recording) stays atomic in the engine. Three nested fixpoints, expressed as
frames:

- **CFG**: each function-body CFG is a block worklist; block parameters
  join across incoming edges and widen after `widen_after` visits — `cf`
  back-edge loops converge.
- **Pushed loop frames**: a dialect loop frame (e.g. `scf.for`'s
  `AbstractScfForFrame`) re-runs its body with joined/widened entry arguments
  until stable — `scf.for` loops converge. The fixpoint is the dialect frame's,
  using the engine's `analysis_merge`.
- **Functions**: each resolved call target is summarized under a key chosen by
  the `CallContext` strategy (`ContextInsensitive` → `(stage, specialization)`), with an
  entry/return `Product<V>` summary. Calls join arguments into the callee's
  entry (enqueueing it on change) and read its current return summary
  (`bottom` until it converges); return-summary changes re-enqueue recorded
  callers — *including same-key (self-)recursion*, so a recursive function's
  rising return propagates back to its own call site (without this, recursion
  sees only the base case). Recursion converges by monotone iteration from
  `bottom`.

Analysis crates stay small: `kirin-constprop` is the `ConstPropValue` lattice, a
`ConstPropContext` strategy (bounded arg-tuple context sensitivity), and
`pub type ConstProp<..> = SparseForwardInterpreter<.., ConstPropValue, .., ConstPropContext>`
— constant propagation is a forward dataflow / forward abstract-interpretation
specialization.

### Engine internals: stage dispatch and IR queries

Two mechanisms keep engines generic over stage enums:

- `InterpDispatch<C>` (derived) — monomorphic dispatch of statement
  interpretation and function entry to each stage's language, mirroring
  `ParseDispatch`. The engine builds its context and dispatch forwards it to the
  matching `Interpretable`/`FunctionEntry` rule.
- `StageQuery` — a bound bundle over kirin-ir's `StageDispatch`/`StageAction`
  machinery for language-independent IR facts (block parameters, statement
  order, CFG entry, specialization lookup, symbol resolution). Satisfied
  automatically by any stage enum; used by engines and linkers internally.

## Custom traversal and policies

Both engines are frame-stack drivers over one **shared protocol**. Compiler
authors can customize *how* an engine traverses (a custom frame type) or *how
precisely* an abstract analysis summarizes (a custom policy `P`), without
forking an engine. This is part of the compiler-author surface. The total frame
type `F` is the engine's generic; it is named in `Interpretable<I, ForwardEval>`
*only* by a structured dialect building `SparseForwardEffect::Push` (through
`SparseForwardInterp::Frame`) — ordinary dialects never mention it.

### Shared protocol vs. forward engine capabilities

`Frame`, `FrameEngine`, `FrameEffect`, and `drive_frames` are **shared and
direction-neutral** — they say nothing about a value domain or direction, and
the backward engines reuse them as-is. On top of that neutral protocol sit the
per-direction engine-capability surfaces: the forward **component traits**
below, composed by the `ForwardFrameEngine` / `ForwardDataflowFrameEngine`
umbrellas, and `DenseBackwardFrameEngine` for the dense backward engine
(statement dispatch, point-state access, edge absorption against the converged
summaries, per-point recording). The sparse backward engine needs no capability
surface at all — its `DemandFrame` dispatches rules directly on the transfer.

**"Engine" means three different things**, so the names are kept distinct:

| name | what it is |
|---|---|
| `drive_frames` | the **frame-stack driver** — the loop. The only thing called a driver at this layer; `ForwardDriver`/`DenseBackwardDriver` are fixpoint-driver *structs*, not capability traits. |
| `FrameEngine` | the **minimal engine contract** the generic frame stack needs: a total `Error` type, nothing more. |
| `ForwardFrameEngine` | the **full engine capability set** for the standard concrete frame universe. |
| `ForwardDataflowFrameEngine` | the capability set for the standard forward-abstract frame universe. |
| component traits | narrowly scoped **services used by individual frames**. |

#### The forward capability model

Forward capabilities are split by **what one frame needs**, not by what one
engine happens to provide. A frame's bound is then a precise statement of which
engine operations it can reach, and an engine that implements only part of the
surface still runs the frames it can support.

| trait | capability | required by |
|---|---|---|
| `StatementDispatch: Interp` | `run_statement` — dispatch to the dialect rule | every executing frame |
| `BlockQueries: Interp` | `block_params`/`first_statement`/`next_statement` | `BlockCursor`, `BlockFrame`, `AbstractBlockFrame`, dialect block walkers |
| `CFGQueries: BlockQueries` | `cfg_entry` | `CFGFrame` |
| `DiGraphQueries: Interp` | `digraph_walk_plan` (default: `NoDefaultWalker`) | `DiGraphFrame`, `AbstractDiGraphFrame` |
| `CallServices: Env` | `alloc_env`/`free_env`/`resolve_call`/`enter_function` | `CallFrame` |

**The `*Queries` traits are read-only, and only require `Interp`** — so nothing
on them can touch SSA storage, and their names cannot hide a store mutation. The
one operation that needs both a query and a write, binding a block's parameters
to incoming actuals, lives on the crate-private `BlockBinding` extension
(bounded `Env + BlockQueries`) instead. A frame that binds a block entry
therefore spells that requirement out: `BlockCursor::bind_entry` and
`::enter_block` take `Env + BlockQueries`, while `::advance` takes `BlockQueries`
alone and `::write_child_results` takes `Env` alone.

`CallServices` names *services*, not a convention: **`CallFrame` still owns the
calling convention** — the operation order, which completions are legal, and
freeing the activation exactly once — and this trait only supplies the
primitives. It is deliberately **not** split further: the standard `CallFrame`
consumes all four together, and their pairing is a safety property (an
`alloc_env` without its `free_env` leaks; a second `free_env` double-frees), so
no engine should be able to offer half a call convention.

`StatementDispatch` and `InterpDispatch` face opposite directions and are easy
to confuse. `InterpDispatch<I>` is implemented by a **stage/language** to route a
statement to the right dialect rule. `StatementDispatch` is implemented by the
**engine** and is what a *frame* calls: it stashes the current location
(`stage`/`statement`/`index`) so the rule can read it back through `Interp`, then
delegates to `InterpDispatch`.

Two umbrellas compose them, one per engine family. Use an umbrella at the
*universe* level — a total frame enum's engine must support the union of all its
variants — and the components at the *member* level:

```rust
// Full concrete surface. Adds no methods; blanket-implemented.
pub trait ForwardFrameEngine:
    StatementDispatch + CFGQueries + DiGraphQueries + CallServices {}
impl<T> ForwardFrameEngine for T
where T: StatementDispatch + CFGQueries + DiGraphQueries + CallServices {}

// Abstract dataflow: the traversal it *shares*, plus merge/summarization.
// Notably NOT CallServices, and NOT CFGQueries.
pub trait ForwardDataflowFrameEngine:
    Env + StatementDispatch + BlockQueries + DiGraphQueries
{
    type SummaryKey: Clone + Eq + Hash;
    fn analysis_merge(..); fn contribute_return(..); fn current_function_key(..);
    fn summarize_call(..); fn max_iterations(..);
}
```

An abstract engine therefore **no longer inherits the concrete call lifecycle**.
That follows the semantics: forward abstract interpretation *summarizes* a call
(`summarize_call` → `AbstractCallFrame`) rather than descending into it, and
reaches a callable body's entry block through `Owner` seeding in the fixpoint
driver rather than `cfg_entry`. Requiring it to expose `alloc_env`, `free_env`,
`enter_function`, `resolve_call`, and `cfg_entry` was demanding a call
convention it never performs. `tests/frame_engine_capabilities.rs` pins this
down with deliberately incomplete mock engines whose ability to compile *is* the
regression test.

Binding values into an **explicitly selected** activation is
`Env::bind_values(index, slots, values)`, not a method on any umbrella, so it is
no longer confusable with `SparseForwardInterp::write_results` (the
dialect-facing helper, which binds into the engine's *current* activation,
`interp.index()`). The two differ by *which activation*, not by what they do —
so neither name mentions the `Product` container it happens to accept.

```rust
pub enum FrameEffect<F, C> { Continue(F), Push { parent: F, child: F }, Done, Complete(C) }

pub trait FrameEngine { type Error; }          // direction-neutral anchor (no value domain)
impl<T: Interp> FrameEngine for T { type Error = <T as Interp>::Error; }

// ONE interface, implemented by every frame — individual walkers and total enums alike.
// The effects are over `F`, the total frame type composed into, never over `Self`.
pub trait Frame<I: FrameEngine, F>: Sized {
    type Completion;
    fn step_into(self, &mut I)        -> Result<FrameEffect<F, Self::Completion>, I::Error>;
    fn resume_done_into(self, &mut I) -> Result<FrameEffect<F, Self::Completion>, I::Error>;
    fn resume_into(self, Self::Completion, &mut I) -> Result<FrameEffect<F, Self::Completion>, I::Error>;
}

// The one shared, direction-neutral driver loop, used by every engine. The
// stack's element type must be a *universe* — `F: Frame<I, F>`.
pub fn drive_frames<I: FrameEngine, F: Frame<I, F>>(engine: &mut I, frames: &mut Vec<F>)
    -> Result<F::Completion, I::Error>;

// Forward-specific capability surface: one component trait per kind of
// traversal, plus two umbrellas — see "The forward capability model" above.
pub trait StatementDispatch: Interp  { /* run_statement */ }
pub trait BlockQueries: Interp       { /* read-only block queries */ }
pub trait CFGQueries: BlockQueries   { /* cfg_entry */ }
pub trait DiGraphQueries: Interp     { /* digraph_walk_plan */ }
pub trait CallServices: Env          { /* alloc/free env, resolve_call, enter_function */ }
pub(crate) trait BlockBinding: Env + BlockQueries { /* bind_block_args */ }
```

**Members and universes.** The `F` parameter is what lets one trait serve both
roles a frame stack needs:

- a **member** — an individual walker (`BlockFrame`, `CallFrame`, a dialect's own
  frame). It is one variant of `F` and names its successors in `F`, re-wrapping
  itself through the relevant `*FrameBuild` hook. Members are generic over `F`,
  so the same walker composes into any language's frame type.
- a **universe** — a total frame enum. It implements `Frame<I, Self>` when it is
  the stack's element type, and stays generic over `F` so it can *also* be
  embedded in a larger enum without re-enumerating its variants. `TracingFrame`
  in `toy-lang`'s tests is exactly this: a newtype wrapping `ToyFrame` whole,
  counting steps and delegating. (A leaf universe with no members, like the
  sparse backward `DemandFrame`, honestly pins `F = Self`.)

The stack must be homogeneous in *type* while heterogeneous in *kind*, which is
why `F` is a closed sum type rather than `Box<dyn Frame>`: a run holds a
`CallFrame`, a `CFGFrame`, a `BlockFrame` and a dialect frame simultaneously.

`Frame` is anchored only on `FrameEngine` (a total `Error`), **not** on the
forward value engine `Interp` — so the frame protocol is decoupled from forward
value interpretation and reusable by other analyses. Every `Interp` is a
`FrameEngine` by blanket impl. The engine owns a `Vec<F>` and calls
`drive_frames`, which pops the top frame, `step_into`s it, and applies the
returned `FrameEffect`. The forward component traits above are the richer
capability surfaces the *forward* frames call; each forward frame bounds only the
components it uses, and the concrete engine implements all of them (so it also
gets `ForwardFrameEngine` by blanket impl). The concrete and abstract standard
frames are two *implementations* of this one protocol — not parallel frameworks.

Narrowest first, the shipped member frames now require:

| frame | bound |
|---|---|
| `ScfIfFrame` | `FrameEngine<Error = E>` — decides its arm before being built, so it touches no engine capability at all |
| `ScfForFrame` | `Env<Value = V, Error = E>` — reads the loop bound/step, pushes a `BlockFrame` |
| `CallFrame` | `CallServices` |
| `BlockCursor` | per operation: `BlockQueries` (query) / `Env + BlockQueries` (bind entry) / `Env` (bind child results) |
| `DiGraphFrame::finish`, `AbstractDiGraphFrame::finish` | `Env` — the schedule is already consumed; only the yields are read |
| `BlockFrame` | `BlockQueries + StatementDispatch + SparseForwardInterp` |
| `CFGFrame` | `CFGQueries + StatementDispatch + SparseForwardInterp` |
| `DiGraphFrame` | `DiGraphQueries + StatementDispatch + SparseForwardInterp` |
| `AbstractBlockFrame`, `AbstractCallFrame`, `AbstractDiGraphFrame` | `ForwardDataflowFrameEngine` (+ `SparseForwardInterp` for the walkers) |
| `StandardFrame`, `ToyFrame`, other total concrete enums | `ForwardFrameEngine + SparseForwardInterp` — correct at the universe level |
| `StandardAbstractFrame`, other total abstract enums | `ForwardDataflowFrameEngine + SparseForwardInterp` |

### Concrete frames — `BlockFrame` / `CFGFrame` / `DiGraphFrame` / `CallFrame` / `StandardFrame`

`ConcreteInterpreter` is generic over the total frame type `F` (default
`StandardFrame`). A custom enum reuses the standard single-path traversal —
the representation walkers and the call boundary — through `FrameBuild`
(`from_block`/`from_cfg`/`from_call`/`from_digraph`) and their `*_into`
delegating methods, adds dialect frames / observation, and instantiates the
engine with that `F`. Overriding `FrameBuild::from_ungraph_entry` (default:
`NoDefaultWalker`) supplies a callable-UnGraph traversal policy without
touching the generic logic (see the workspace `tests/body_kinds.rs` policy
test). (Further examples: `example/toy-lang`'s `ToyFrame`, which adds
`kirin_scf`'s `ScfIfFrame`/`ScfForFrame` via `BuildScfIf`/`BuildScfFor`; and a
`TracingFrame` counting call/body visitation while running the real program — see
`example/toy-lang`'s `interpreter::tests::advanced`.)

### Callable-body walkers — `CallBodyFramePolicy` / `DefaultBodyFrames`

`CallFrame` bundles two separable concerns, and only the second is
configurable:

| Concern | Where | Configurable? |
|---|---|---|
| **call convention** — resolve the callee, allocate its activation, ask `FunctionEntry` for the body, suspend, validate the completion kind, free the activation *exactly once*, bind results | `CallFrame` itself | **no** — this is where double-frees would live |
| **walker choice** — which frame traverses the callee body | `CallBodyFramePolicy<V, E, F>` | **yes** |

`Body` stays a closed vocabulary, so `CallFrame::step_into` still matches it
exhaustively; only the frame each arm builds is chosen by the policy. The
default reproduces today's behaviour exactly:

| body | `DefaultBodyFrames` | a custom `MyBodyFrames` might use |
|---|---|---|
| `CFG` | `CFGFrame` | `MyCustomCFGFrame` |
| `Block` | `BlockFrame` | `BlockFrame` (delegate to the default) |
| `DiGraph` | `DiGraphFrame` | `MyScheduledGraphFrame` |
| `UnGraph` | `FrameBuild::from_ungraph_entry` → `NoDefaultWalker` unless overridden | `MyCircuitWalker` |

The policy is selected by the **compiler/language author** through the concrete
total frame type's `FrameBuild::BodyFrames`; `#[derive(FrameBuild)]` emits
`DefaultBodyFrames` unless given
`#[interpret(body_frames = MyBodyFrames)]`. `CallFrame<V>` continues to mean
`CallFrame<V, DefaultBodyFrames>`. A dialect crate may *offer* reusable walkers
or policies, but a callable dialect should not permanently fix one traversal for
every engine.

**Concrete execution only, and deliberately so.** Concrete execution descends
into a callee — `CallFrame` → body walker → completion → `CallFrame`. Forward
abstract interpretation does not: `AbstractCallFrame` *summarizes* the call while
the fixpoint engine separately maps a callable body to an `Owner::Block` or
`Owner::Graph` in `seed_entry_block`. Customizing that would be an abstract
body-entry/owner policy, not this one. The backward engines differ further —
sparse backward uses SSA values as owners and never walks callable bodies through
a call frame; dense backward uses block owners and reverse walks. If those ever
need configurable representation traversal, add engine-family-specific policies;
do not make IR owners supply walkers.

**Not consulted for nested bodies.** `scf.if`/`scf.for` enter their Blocks
through their own dialect frames (chosen per engine by `ScfIfDispatch` /
`ScfForDispatch`), which then reuse a framework `BlockFrame`. Those are *nested*
bodies — they borrow the caller's activation and exit by `Yield` — so the
callable-body policy plays no part.

### Abstract frames — `StandardAbstractFrame` / `AbstractFrameBuild` / `ForwardDataflowFrameEngine`

`SparseForwardInterpreter` is symmetrically generic over a total abstract frame type
`F` (default `StandardAbstractFrame`). The standard abstract frames
(`AbstractBlockFrame`, `AbstractCallFrame`, `AbstractDiGraphFrame`) implement the
*same*
`Frame` protocol, but their traversal is the abstract one: a CFG block worklist
that joins/widens at merge points, `Branch` exploration, single-block
body walks that complete on `Yield`, dependency-ordered graph passes, and
per-key call summarization. A custom
enum reuses them through `AbstractFrameBuild` and the `*_into` methods — exactly
mirroring the concrete pattern (see `ToyAbstractFrame`, which adds
`AbstractScfIfFrame`/`AbstractScfForFrame`, and `TracingAbstractFrame` in the
same test module).

**Executable owners and the body vocabulary.** The forward fixpoint's work items
are `Owner`s, and only *executable* owners run frames: `Owner::Block` (one CFG
block) and `Owner::Graph` (one whole graph body). `Owner::Function` is
storage-only — it accumulates a context's joined entry arguments and joined
return, and is never scheduled. `seed_entry_block` is the single place a callable
body becomes executable work, translating the closed `Body` vocabulary into an
owner: `CFG` → its entry block, `Block` → itself, `DiGraph` → an `Owner::Graph`.
A graph owner is one unit because a single dependency-ordered pass is *exact* for
a DAG — no intra-graph widening is needed, and convergence pressure comes only
from entry widening when a new call site raises the owner's entry product. On
completion a graph owner has no successor edges; its declared yields become the
function's return contribution. `UnGraph` bodies are rejected with
`NoDefaultWalker`: an undirected graph has no derivable traversal order, and
unlike the concrete engine's `FrameBuild::from_ungraph_entry` there is currently
no seam through which a compiler could supply one. Analogously,
`AbstractFrameBuild::from_digraph` defaults to rejecting, so a total abstract
frame enum that carries no `AbstractDiGraphFrame` inherits the refusal rather
than pretending to analyze a graph body.

`AbstractDiGraphFrame` differs from the concrete `DiGraphFrame` in exactly one
substantive way: a `Call` effect pushes an `AbstractCallFrame`, routing the call
through `summarize_call` instead of descending into the callee. Descending would
neither widen nor terminate on recursion.

Abstract frames need a few capabilities beyond the traversal they share with
concrete execution, on `ForwardDataflowFrameEngine: Env + StatementDispatch +
BlockQueries + DiGraphQueries` —
`analysis_merge`, `contribute_return`, and `summarize_call`. It does **not**
extend `CallServices`: `AbstractCallFrame`'s single engine requirement is
`summarize_call`, so summarizing a call needs no call convention at all. Nor
`CFGQueries`, since the entry block of a callable body arrives via `Owner`
seeding rather than `cfg_entry`. The interprocedural protocol stays **atomic in
the engine**:
`summarize_call` performs resolve → key → join-into-callee-entry → record-caller
(*including same-key recursion*) → read-return-summary in one step, so a custom
frame chooses *what to traverse* but cannot reorder the summary protocol and
break soundness.

### Abstract policies — `CallContext` and `WideningStrategy`

`SparseForwardInterpreter` is generic over an analysis parameter `P` providing two decisions:

```rust
pub trait CallContext<V>     { type Key: Eq + Hash + Clone;
                               fn key(&mut self, stage, function, args: &Product<V>) -> Self::Key; }
pub trait WideningStrategy<V> { fn merge(&self, current, incoming, visits) -> Result<Product<V>, _>; }
```

`ContextInsensitive` keys by `(stage, specialization)` — every call site of a
function shares one summary — and joins-then-widens after `widen_after` visits.
`kirin-constprop`'s
`ConstPropContext` keys distinct fully-constant argument tuples to distinct
summaries — bounded by a per-function budget, with overflow and non-constant
arguments collapsing to one shared `Unknown` context (joined → sound `Top`).
That is what makes recursive constant propagation precise on both linear
recursion (`factorial(Const(5)) → Const(120)`) and overlapping-subproblem
recursion, where per-constant summaries memoize each call so the analysis stays
precise *and* non-explosive (`fib(Const(10)) → Const(55)`) — while still sound
and terminating on unknown inputs (both fold to `Top`). Runnable as
`example/toy-lang/programs/{factorial,fibonacci}.kirin`.

## Design rules

1. **Dialects are engine-blind.** Undecidedness is expressed by the value
   domain (`is_truthy`/`loop_condition` returning `None`); for cf it surfaces as
   `SparseForwardEffect::Branch`, for control dialects it is handed to the dialect's own
   frame. One `Interpretable` impl serves every engine; a control dialect's
   *frames* may have separate concrete/abstract forms, built per-engine through a
   dialect dispatch trait.
2. **Policy is a component, not an impl.** Anything a compiler author might
   swap (linkers, widening thresholds) is a value passed to the engine.
   Blanket impls on engine types are forbidden as extension points because
   coherence makes them unoverridable.
3. **Fixpoints live in engines.** Dialects contribute one-step transfer
   relations; joins, widening, summaries, and convergence are engine code.
4. **Derives are named after traits.** A new derive name is a new concept;
   only introduce one alongside a trait of the same name.

## Status and deferred work

- Both engines are frame-parametric over the shared, direction-neutral
  `FrameEngine`/`Frame`/`drive_frames` protocol: `ConcreteInterpreter<.., F>`
  (default `StandardFrame`) and `SparseForwardInterpreter<.., P, F>`
  (default `StandardAbstractFrame`). Abstract explore/join/summarize lives in dedicated
  abstract frames reused via `AbstractFrameBuild`; there is no longer an un-framed
  abstract worklist. `Frame` is anchored on `FrameEngine` (a total error), not on
  `Interp`, so the protocol is reusable beyond forward value interpretation.
- The per-statement effect is the associated type `I::Effect`, **per analysis**
  — forward execution/abstract interpretation use the `ForwardEval` semantics
  whose `Effect` is `SparseForwardEffect`. The backward analyses are
  **implemented** on this seam, as two distinct kinds (deliberate deviations
  from the earlier sketch noted inline):
  - **Strong liveness / demand** (`StrongDemand`,
    `SparseBackwardInterpreter`): `Interp::Value` is the *per-SSA-value* demand
    fact (`kirin_liveness::Live`), not a `LiveSet` — the sparse fact anchors to
    values, mirroring the forward per-value model. Helpers are
    `is_demanded`/`demand` (on `DemandInterp`)/`effect()` plus the purity-aware `demand_uses_if_observable`
    (rather than the sketched `live_after`/`use_def` names); the effect is
    `SparseBackwardEffect::Demands`. Summary owners ARE scope-qualified SSA
    values, so the fixpoint driver's default self-dependent index is the demand
    worklist; scf converges loop-carried demand with **no frames**.
  - **Classic per-point liveness** (`ClassicLiveness`,
    `DenseBackwardInterpreter`): here `Interp::Value` *is* the point state
    (`LiveSet`), realizing the sketch's `Value = LiveSet` — with the textbook
    kill/gen transfer (`gen_uses_kill_defs`, purity-irrelevant). Block owners
    converge boundary summaries; `live_before`/`live_after` are reconstructed
    per point on demand (never persisted by the fixpoint); scf owns dense
    frames (arm-join, loop fixpoint). Classic liveness consumes finalized IR
    directly and does not require a sparse-demand pre-pass.
  Strong per-point sets are the composition `dense ∩ demanded`, not a third
  analysis. Because the `Semantics` parameter distinguishes impls, one dialect
  carries all three rules at once, as every shipped dialect demonstrates.
- Function-summary context sensitivity is a pluggable `CallContext` strategy.
  `ContextInsensitive` is the context-insensitive baseline; `ConstPropContext` provides bounded
  arg-tuple keys (precise recursion; sound, terminating cap to `Unknown`).
  Unbounded call-string (k-CFA) policies remain future work — another
  `CallContext` impl, no engine change.
- First-class function values (`Lambda`/`Bind` as values, `Callee` from an
  SSA value) are not yet supported by either engine.
