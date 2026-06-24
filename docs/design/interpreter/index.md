# Interpreter Framework

The interpreter framework (`kirin-interpreter`) supports concrete execution
and lattice-based abstract interpretation over the same dialect semantics,
including analyses that cross language boundaries in multi-stage pipelines.

The design is organized as a **two-persona contract**:

- **Dialect authors** describe what each statement *means*, once, in a small
  fixed vocabulary — `Interpretable<ForwardContext<'_, I>>`/`ForwardContext`/`ForwardEffect`,
  specializing on the forward *context* type rather than the engine. There is no framework
  "scope": a statement whose operation owns structured control runs a
  sub-computation by *pushing a frame the dialect owns* (`ForwardEffect::Push`),
  built per-engine through a small dialect dispatch capability. Ordinary
  (non-control) dialects never push frames at all.
- **Compiler authors** compose languages into pipelines and *select*
  components: an engine, a value domain, an error type, and a linker. When they
  need more control, the same compiler-author surface also includes opt-in
  traversal and analysis components: custom concrete frames
  (`ConcreteInterpreter<.., F>`), custom abstract frames
  (`AbstractInterpreter<.., P, F>`), and abstract policies `P`
  (`CallContext` + `WideningStrategy`). A language that uses a structured-control
  dialect composes its own total frame type embedding the standard frames plus
  that dialect's frames. Ordinary dialects never name a frame type.

Every derive macro is named after the trait it implements
(`#[derive(Interpretable)]` → `trait Interpretable`), so learning the derive
is learning the trait.

## Dialect-author surface

Everything is exported from `kirin_interpreter::dialect`.

### `Interp`, `InterpretCtx`, and `Interpretable<C>` — statement semantics

```rust
pub trait Interp: Sized {               // the engine-side driver — ANALYSIS-AGNOSTIC
    type Value: Clone;                  // the value domain
    type Error: From<InterpreterError>; // the total error
    type Effect;                        // analysis-specific per-statement effect
    type Context<'a>: InterpretCtx<Value = Self::Value, Error = Self::Error, Effect = Self::Effect>
    where Self: 'a;                     // the engine's *own* dialect-facing context type
    fn context<'a>(&'a mut self, ..) -> Self::Context<'a>; // build it per statement
}

// FORWARD value-interpretation storage access — split out of `Interp` so the base
// trait stays analysis-agnostic. A backward analysis would NOT implement this.
pub trait ForwardEnv: Interp {
    fn env_read(..) -> Result<Self::Value, Self::Error>;
    fn env_write(..) -> Result<(), Self::Error>;
}

// What every interpretation *context* exposes — the dialect-impl specialization boundary.
pub trait InterpretCtx { type Value: Clone; type Error: From<InterpreterError>; type Effect; }

// The single dialect trait — specialized on the CONTEXT type `C`, not the engine `I`.
pub trait Interpretable<C: InterpretCtx>: Dialect {
    fn interpret(&self, ctx: &mut C) -> Result<C::Effect, C::Error>;
}
```

`ForwardContext<'_, I>` exposes the forward read/write helpers — `read`, `read_many`,
`write`, `write_results` — as **inherent methods** (they delegate to the engine's
[`ForwardEnv`] storage access), so dialect rules call `ctx.read(..)` / `ctx.write(..)`
**without importing any trait**. (There is no `ForwardCtx` trait — the helpers are
inherent.) A future liveness context would expose its own inherent helpers
(`live_after`/`use_def`/`transfer`) instead.

A forward statement rule is `impl<I: ForwardInterp, ..> Interpretable<ForwardContext<'_, I>>
for Op`: it specializes on the concrete forward context `ForwardContext<'_, I>`,
reads/writes through that context's inherent `ctx.read`/`ctx.write` helpers, and returns `I::Effect`
(= `ForwardEffect`). **The context type is the specialization boundary, not the
engine.** A future backward analysis (liveness) implements
`Interpretable<LivenessContext<'_, I>>` for its *own distinct* context type, so its
dialect impls never overlap the forward ones (no `E0119`) — even though both are
generic over the engine `I`. (Two impls keyed on `I` alone, differing only in a
`where I: ForwardInterp` vs `where I: LiveInterp` bound, *do* overlap, because
coherence ignores those bounds — which is exactly why the distinction is carried by
the context *type*, a different type constructor per analysis, rather than an engine
bound.)

`Interp` is the interpreter/analysis **driver**: it exposes the value domain, the
error type, the per-statement effect, **and its own dialect-facing context type**
([`Interp::Context<'a>`], built per statement by [`Interp::context`]) — replacing the
old `Interpretable<L, I, F, C, E, T>` parameter soup. **The engine owns its context
type**: the context is the *short-lived, dialect-facing* half of the pair (it borrows
the engine for one statement), while the engine is the *long-lived,
compiler-author/internal* half (env store, frame stack, summaries). The forward
engines set `type Context<'a> = ForwardContext<'a, Self>`; dispatch never names a
concrete context — it asks the engine to build `I::Context<'_>`. A rule produces
`C::Effect` (= `I::Effect` for the forward context) — the **analysis-specific** effect
algebra — not a single universal enum. (The frame type stays the engine's own `F`
generic, e.g. `ConcreteInterpreter<.., F>`, so traversal is customizable without an
unused associated type on `Interp`.) Forward rules bound `I: ForwardInterp`, the flavor of
`Interp` whose `Effect = ForwardEffect<I::Value, I::Frame>`, so they build and return
`ForwardEffect` values (which are `I::Effect`). `I::Frame` is the engine's total
frame type, re-exposed by `ForwardInterp` only so a structured dialect can name
the frame it pushes; ordinary dialects never mention it (it is inferred from
`I::Effect`). They constrain only:

- the value domain, with plain Rust bounds — `I::Value: Add<Output = I::Value>`
  (kirin-arith), `I::Value: BranchCondition` (kirin-cf), `I::Value:
  ForLoopValue` (kirin-scf);
- error lifting — `I::Error: From<DivisionByZero>`.

Because the impl is generic over the value domain, **one transfer rule serves
both execution and analysis**: `kirin-arith`'s `Add` rule computes `3 + 5`
under `ConcreteInterpreter<.., i64, ..>` and folds `Const(3) + Const(5)`
under constant propagation, with no analysis-specific code in the dialect.

`ForwardContext<'_, I>` is the **forward context** type: it implements `InterpretCtx`
(carrying the engine's `Value`/`Error`/`Effect`) and exposes the SSA read/write
helpers as **inherent methods**, hiding environment indices and locations:
`ctx.read(ssa)`, `ctx.write(result, value)`, `ctx.read_many(&values)`,
`ctx.write_results(&results, product)` — all callable without importing any trait.
They delegate to the engine's [`ForwardEnv`] storage access (`env_read`/`env_write`),
which lives on the forward-only `ForwardEnv` trait rather than base `Interp` so the
base trait stays analysis-agnostic. A structured dialect reaches the engine through
`ctx.interp()` to build the frame it pushes (see SCF below). A future liveness context
would be a *different* type exposing *different* helpers (e.g.
`live_after`/`use_def`/`transfer`) and returning its own effect — never these
forward read/write helpers, and its engine would not implement `ForwardEnv`.

### `ForwardEffect` — the forward control algebra

This is the `Effect` for the *forward* mode (`ForwardInterp::Effect`). It is **one
algebra among potential several**: a future analysis defines its own `I::Effect`
rather than adding variants here.

```rust
pub enum ForwardEffect<V, F> {
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
traversal builds `Push` (naming the frame via `ForwardInterp::Frame`). The pushed
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
dispatch capability) and returns it as `ForwardEffect::Push { frame, results }`.
SCF has two such operations:

- **`scf.if`** → `kirin_scf::ScfIfFrame` (concrete) / `AbstractScfIfFrame`
  (abstract), built via `ScfIfDispatch::scf_if_frame(.., decided)`. The rule
  reads the condition value and hands the `Option<bool>` decision to the frame;
  the **frame** picks the arm (concrete; undecided is `IndeterminateBranch`) or
  explores both arms and **joins** their finish results (abstract). It walks each
  arm by pushing the framework `BodyFrame`/`AbstractBlockFrame` building block.

- **`scf.for`** → `ScfForFrame` / `AbstractScfForFrame`, built via
  `ScfForDispatch`. The frame pushes a body frame each iteration, advances the
  induction variable on each yield, and decides repeat/finish in the value
  domain. The **loop-carried fixpoint lives in the abstract loop frame**: it
  joins (then widens) the entry state across iterations until stable,
  accumulating finish values across exits — so `scf.for` over a lattice
  converges, with no framework "scope hook".

The framework `BodyFrame`/`AbstractBlockFrame` (single-block body walkers,
completing on `Yield`) are reusable **building blocks**, not framework-owned
structured semantics: the SCF frames build them to walk a chosen body, but the
structured *decision* and result binding stay in the SCF frame. A language that
uses SCF composes a total frame type embedding the standard frames plus
`ScfIfFrame`/`ScfForFrame` (via `BuildScfIf`/`BuildScfFor` and the abstract
equivalents); see `example/toy-lang`'s `ToyFrame`/`ToyAbstractFrame`. Future
structured dialects would follow the same pattern; only the existing SCF
operations are implemented.

### `FunctionEntry<C>` — callable statements

```rust
pub trait FunctionEntry<C: InterpretCtx>: Dialect {
    fn function_entry(&self, args: Product<C::Value>, ctx: &mut C)
        -> Result<FunctionBody<C::Value>, C::Error>;
}
```

Like `Interpretable`, it is specialized on the context type `C` (the forward
context `ForwardContext<'_, I>`).

Statements that define function bodies (e.g. `kirin_function::Function`)
return the `FunctionBody { region, args }` to enter on invocation (the
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
the linker at `ForwardEffect::Call`, and the analysis lattice flows through
`Product<V>` function summaries regardless of which language the callee
belongs to.

## Engines

### `ConcreteInterpreter<'ir, S, V, E, Lk, F = StandardFrame<V, E>>`

A generic **frame-stack driver**: it pops the top frame, calls `Frame::step`,
and applies the returned `FrameEffect` (`Continue` / `Push` / `Done` /
`Complete`) — it owns *no* traversal logic itself. Traversal lives in the
frames. The default total frame type `StandardFrame<V, E>` wraps the standard
`BodyFrame` (walks a function-body region CFG, or a single body block that
completes on `Yield` — `Jump` retargets it, `Return` completes it) and
`CallFrame` (dispatch a callee, await its `Return`). The dialect-produced
`ForwardEffect` is consumed by `BodyFrame`, which maps it to a `FrameEffect`
(handling `Push` by pushing the carried frame). `StandardFrame` is
structured-control-free; a custom `F`
([Custom traversal and policies](#custom-traversal-and-policies)) adds dialect
frames or replaces traversal without touching the engine.

### `AbstractInterpreter<'ir, S, V, E, Lk, P = ContextInsensitive, F = StandardAbstractFrame<..>>`

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

- **CFG**: each function body region is a block worklist; block parameters
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
`pub type ConstProp<..> = AbstractInterpreter<.., ConstPropValue, .., ConstPropContext>`.

### Engine internals: stage dispatch and IR queries

Two mechanisms keep engines generic over stage enums:

- `InterpDispatch<C>` (derived) — monomorphic dispatch of statement
  interpretation and function entry to each stage's language, mirroring
  `ParseDispatch`. It is parameterized by the **context type** `C: InterpretCtx`,
  *not* the engine — the same context-type-is-the-boundary principle as
  `Interpretable`. The engine builds *its* context (`interp.context(..)` →
  `I::Context<'_>`) and passes the already-built `ctx` in; dispatch matches the
  statement's language and forwards `ctx` to the context-specialized
  `Interpretable`/`FunctionEntry` rule. The forward engines build
  `ForwardContext<'_, I>`, so their `FrameDriver` bound is the *concrete*
  `for<'a> InterpDispatch<ForwardContext<'a, I>>` — quantified over a concrete context
  type, **never** a GAT projection like `for<'a> Interpretable<I::Context<'a>>`, which
  would spuriously require `I: 'static` (a `for<'a>` over the GAT's `where Self: 'a`
  collapses to `'static`). A future analysis drives its own context type identically
  (`for<'a> InterpDispatch<LivenessContext<'a, I>>`), reusing this one generic dispatch
  trait without overlapping the forward path.
- `StageQuery` — a bound bundle over kirin-ir's `StageDispatch`/`StageAction`
  machinery for language-independent IR facts (block parameters, statement
  order, region entry, specialization lookup, symbol resolution). Satisfied
  automatically by any stage enum; used by engines and linkers internally.

## Custom traversal and policies

Both engines are frame-stack drivers over one **shared protocol**. Compiler
authors can customize *how* an engine traverses (a custom frame type) or *how
precisely* an abstract analysis summarizes (a custom policy `P`), without
forking an engine. This is part of the compiler-author surface. The total frame
type `F` is the engine's generic; it is named in `Interpretable<ForwardContext<'_, I>>`
*only* by a structured dialect building `ForwardEffect::Push` (through
`ForwardInterp::Frame`) — ordinary dialects never mention it.

### The shared protocol — `FrameEngine` / `Frame` / `drive_frames` / `FrameDriver`

```rust
pub enum FrameEffect<F, C> { Continue(F), Push { parent: F, child: F }, Done, Complete(C) }

pub trait FrameEngine { type Error; }          // direction-neutral anchor (no value domain)
impl<T: Interp> FrameEngine for T { type Error = <T as Interp>::Error; }

pub trait Frame<I: FrameEngine>: Sized {       // implemented by the *total* frame enum
    type Completion;
    fn step(self, &mut I)        -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
    fn resume_done(self, &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
    fn resume(self, Self::Completion, &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
}

// The one shared driver loop, used by every engine:
pub fn drive_frames<I: FrameEngine, F: Frame<I>>(engine: &mut I, frames: &mut Vec<F>)
    -> Result<F::Completion, I::Error>;

pub trait FrameDriver: ForwardEnv { /* env alloc/free, IR queries, statement dispatch, call resolution */ }
```

`Frame` is anchored only on `FrameEngine` (a total `Error`), **not** on the
forward value engine `Interp` — so the frame protocol is decoupled from forward
value interpretation and reusable by other analyses. Every `Interp` is a
`FrameEngine` by blanket impl. The engine owns a `Vec<F>` and calls
`drive_frames`, which pops the top frame, `step`s it, and applies the returned
`FrameEffect`. `FrameDriver: ForwardEnv` is the richer **forward** capability
surface the *forward* frames call (it requires `ForwardEnv` because the default
`bind_block_args`/`write_results` use `env_write`); **both forward engines implement
it**. The concrete and
abstract standard frames are two *implementations* of this one protocol — not
parallel frameworks.

### Concrete frames — `BodyFrame` / `CallFrame` / `StandardFrame`

`ConcreteInterpreter` is generic over the total frame type `F` (default
`StandardFrame`). A custom enum reuses the standard `BodyFrame`/`CallFrame`
single-path traversal through `FrameBuild` (`from_body`/`from_call`) and their
`*_into` delegating methods, adds dialect frames / observation, and instantiates
the engine with that `F`. (Examples: `example/toy-lang`'s `ToyFrame`, which adds
`kirin_scf`'s `ScfIfFrame`/`ScfForFrame` via `BuildScfIf`/`BuildScfFor`; and a
`TracingFrame` counting call/body visitation while running the real program — see
`example/toy-lang`'s `interpreter::tests::advanced`.)

### Abstract frames — `StandardAbstractFrame` / `AbstractFrameBuild` / `AbstractFrameDriver`

`AbstractInterpreter` is symmetrically generic over a total abstract frame type
`F` (default `StandardAbstractFrame`). The standard abstract frames
(`AbstractFunctionFrame`, `AbstractCfgFrame`, `AbstractBlockFrame`,
`AbstractCallFrame`) implement the *same*
`Frame` protocol, but their traversal is the abstract one: a CFG block worklist
that joins/widens at merge points, `Branch` exploration, single-block
body walks that complete on `Yield`, and per-key call summarization. A custom
enum reuses them through `AbstractFrameBuild` and the `*_into` methods — exactly
mirroring the concrete pattern (see `ToyAbstractFrame`, which adds
`AbstractScfIfFrame`/`AbstractScfForFrame`, and `TracingAbstractFrame` in the
same test module).

Abstract frames need a few capabilities beyond `FrameDriver`, on
`AbstractFrameDriver: FrameDriver` — `analysis_merge`, `contribute_return`, and
`summarize_call`. The interprocedural protocol stays **atomic in the engine**:
`summarize_call` performs resolve → key → join-into-callee-entry → record-caller
(*including same-key recursion*) → read-return-summary in one step, so a custom
frame chooses *what to traverse* but cannot reorder the summary protocol and
break soundness.

### Abstract policies — `CallContext` and `WideningStrategy`

`AbstractInterpreter` is generic over an analysis parameter `P` providing two decisions:

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
   `ForwardEffect::Branch`, for control dialects it is handed to the dialect's own
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
  (default `StandardFrame`) and `AbstractInterpreter<.., P, F>` (default
  `StandardAbstractFrame`). Abstract explore/join/summarize lives in dedicated
  abstract frames reused via `AbstractFrameBuild`; there is no longer an un-framed
  abstract worklist. `Frame` is anchored on `FrameEngine` (a total error), not on
  `Interp`, so the protocol is reusable beyond forward value interpretation.
- The per-statement effect is the associated type `C::Effect`, **per analysis**
  — forward execution/abstract interpretation use the forward context
  `ForwardContext` whose `Effect` is `ForwardEffect`. A future analysis (e.g.
  backward liveness) defines its **own** context type (`LivenessContext<'_, I>`),
  implementing `InterpretCtx` with its **own** `Effect` algebra and its own inherent
  liveness-specific helpers (e.g. `live_after`/`use_def`/`transfer`) instead of the
  forward read/write helpers. Its `Interpretable<LivenessContext<'_, I>>` dialect impls do
  **not** overlap the forward `Interpretable<ForwardContext<'_, I>>` impls — the
  **context type is the
  specialization boundary**, so the two analyses share statements without `E0119`
  even though both are generic over the engine. It reuses the same frame
  protocol. Implementing such an analysis is deliberately **out of scope** here;
  this pass only consolidated the trait surface and *prepared* the seam
  (context-typed `Interpretable`, associated `Effect`, frames decoupled from
  `Interp`).
- Function-summary context sensitivity is a pluggable `CallContext` strategy.
  `ContextInsensitive` is the context-insensitive baseline; `ConstPropContext` provides bounded
  arg-tuple keys (precise recursion; sound, terminating cap to `Unknown`).
  Unbounded call-string (k-CFA) policies remain future work — another
  `CallContext` impl, no engine change.
- First-class function values (`Lambda`/`Bind` as values, `Callee` from an
  SSA value) are not yet supported by either engine.
