# Interpreter Framework

The interpreter framework (`kirin-interpreter`) supports concrete execution
and lattice-based abstract interpretation over the same dialect semantics,
including analyses that cross language boundaries in multi-stage pipelines.

The design is organized as a **two-persona contract**:

- **Dialect authors** describe what each statement *means*, once, in a small
  fixed vocabulary — `Interpretable`/`Ctx`/`ForwardEffect`, plus `Scope`/`ScopeHook`
  for structured control. They never see engines, stages, pipelines, frames, or
  fixpoints.
- **Compiler authors** compose languages into pipelines and *select*
  components: an engine, a value domain, an error type, and a linker. When they
  need more control, the same compiler-author surface also includes opt-in
  traversal and analysis components: custom concrete frames
  (`ConcreteInterpreter<.., F>`), custom abstract frames
  (`AbstractInterpreter<.., P, F>`), and abstract policies `P`
  (`CallContext` + `WideningStrategy`). These extensions do not change the
  dialect contract — frames never appear in `Interpretable`.

Every derive macro is named after the trait it implements
(`#[derive(Interpretable)]` → `trait Interpretable`), so learning the derive
is learning the trait.

## Dialect-author surface

Everything is exported from `kirin_interpreter::dialect`.

### `Interp` and `Interpretable<I>` — statement semantics

```rust
pub trait Interp: Sized {
    type Value: Clone;                  // the value domain
    type Error: From<InterpreterError>; // the total error
    type Effect;                        // analysis-specific per-statement effect
    fn env_read(..); fn env_write(..);
}

pub trait Interpretable<I: Interp>: Dialect {
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error>;
}
```

`Interp` is the interpreter/analysis **driver**: it exposes the value domain, the
error type, and the per-statement effect — replacing the old
`Interpretable<L, I, F, C, E, T>` parameter soup. A rule produces `I::Effect` —
the **analysis-specific** effect algebra — not a single universal enum. (The
frame type stays the engine's own `F` generic, e.g. `ConcreteInterpreter<.., F>`,
so traversal is customizable without an unused associated type on `Interp`.)
Forward rules bound `I: ForwardInterp`, the flavor of `Interp` whose
`Effect = ForwardEffect<I::Value, I::Error>`, so they build and return
`ForwardEffect` values (which are `I::Effect`). They constrain only:

- the value domain, with plain Rust bounds — `I::Value: Add<Output = I::Value>`
  (kirin-arith), `I::Value: BranchCondition` (kirin-cf), `I::Value:
  ForLoopValue` (kirin-scf);
- error lifting — `I::Error: From<DivisionByZero>`.

Because the impl is generic over the value domain, **one transfer rule serves
both execution and analysis**: `kirin-arith`'s `Add` rule computes `3 + 5`
under `ConcreteInterpreter<.., i64, ..>` and folds `Const(3) + Const(5)`
under constant propagation, with no analysis-specific code in the dialect.

`Ctx` hides environment indices and locations: `ctx.read(ssa)`,
`ctx.write(result, value)`, `ctx.read_many(&values)`,
`ctx.write_results(&results, product)`.

### `ForwardEffect` — the forward control algebra

This is the `Effect` for the *forward* mode (`ForwardInterp::Effect`). It is **one
algebra among potential several**: a future analysis defines its own `I::Effect`
rather than adding variants here.

```rust
pub enum ForwardEffect<V, E> {
    Next,                       // atomic statement done
    Jump(Edge<V>),              // decided CFG transfer
    Branch(Vec<Edge<V>>),       // undecided CFG transfer
    Call(CallEffect<V>),        // function invocation (resolved by the linker)
    Yield(Product<V>),          // terminate the innermost scope body
    Return(Product<V>),         // return from the enclosing function
    Enter(Scope<V, E>),         // run a structured sub-computation
    EnterAny(Vec<Scope<V, E>>), // undecided structured branch
}
```

The undecided variants encode the concrete/abstract split *in the value
domain*, not in the dialect: a dialect asks the value
(`BranchCondition::is_truthy() -> Option<bool>`) and emits the decided form
when it gets an answer, the undecided form when it does not. Concrete engines
reject undecided effects (`IndeterminateBranch`); abstract engines explore
every alternative and join. Dialects therefore have exactly one impl and no
knowledge of which engine is running.

### `Scope` and `ScopeHook` — structured control flow

A `Scope` is a body (`Block` for scf-style operations, `Region` for function
bodies, or `Immediate` for "skip" alternatives), entry arguments, result
bindings, and an optional hook:

```rust
Scope::block(self.then_body).bind(self.results.iter().copied())          // scf.if arm
Scope::block(self.body).args(...).bind(...).on_yield(ForHook { ... })    // scf.for
Scope::immediate(inits).bind(...)                                        // for with 0 iterations
Scope::region(self.body).args(args)                                      // function entry
```

Without a hook, the first `Yield` finishes the scope (the `if` shape). With a
hook, the dialect decides on each yield:

```rust
pub trait ScopeHook<V, E> {
    fn on_yield(self: Box<Self>, entry: &Product<V>, yielded: Product<V>,
                env: &mut dyn EnvOps<V, E>) -> Result<ScopeStep<V, E>, E>;
}

pub enum ScopeStep<V, E> {
    Finish(Product<V>),
    Repeat { args: Product<V>, hook: Box<dyn ScopeHook<V, E>> },
    RepeatOrFinish { args: Product<V>, results: Product<V>, hook: Box<dyn ScopeHook<V, E>> },
}
```

`entry` is the product currently bound to the body parameters. Under abstract
interpretation it is the *joined* entry state, so hooks must derive iteration
state from it (e.g. `scf.for` reads the induction variable from `entry[0]`),
never from captured per-iteration values. `RepeatOrFinish` is the
loop-condition analogue of `Branch`: undecided in the value domain.

Crucially, **the engine owns the loop fixpoint**. The concrete engine re-binds
and re-runs the body while the hook says `Repeat`; the abstract engine joins
(then widens) the entry arguments across `Repeat`s and re-runs until the entry
state is stable, accumulating `Finish` results. The dialect contributes only
the one-step relation.

### `FunctionEntry<I>` — callable statements

```rust
pub trait FunctionEntry<I: Interp>: Dialect {
    fn function_entry(&self, args: Product<I::Value>, ctx: &mut Ctx<'_, I>)
        -> Result<Scope<I::Value, I::Error>, I::Error>;
}
```

Statements that define function bodies (e.g. `kirin_function::Function`)
return the scope to enter on invocation. On language enums it is derived;
`#[callable]` marks the variants that forward, all others report
`NotCallable`.

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
`ScopeFrame` (block/region/hook-driven scope traversal — `Jump` retargets it,
`Yield`/`Return` complete it) and `CallFrame` (dispatch a callee, await its
`Return`). The dialect-produced `ForwardEffect` is consumed by `ScopeFrame`, which
maps it to a `FrameEffect`. A custom `F`
([Custom traversal and policies](#custom-traversal-and-policies)) replaces
traversal without touching the engine.

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
- **Scopes**: hook-driven scopes re-run with joined/widened entry arguments
  until stable — `scf.for` loops converge.
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

Two mechanisms keep engines generic over stage enums without HRTBs:

- `InterpDispatch<I>` (derived) — monomorphic dispatch of statement
  interpretation and function entry to each stage's language, mirroring
  `ParseDispatch`.
- `StageQuery` — a bound bundle over kirin-ir's `StageDispatch`/`StageAction`
  machinery for language-independent IR facts (block parameters, statement
  order, region entry, specialization lookup, symbol resolution). Satisfied
  automatically by any stage enum; used by engines and linkers internally.

## Custom traversal and policies

Both engines are frame-stack drivers over one **shared protocol**. Compiler
authors can customize *how* an engine traverses (a custom frame type) or *how
precisely* an abstract analysis summarizes (a custom policy `P`), without
forking an engine. This is part of the compiler-author surface. None of it is
visible to dialect authors — **frame generics never leak into
`Interpretable<I>`**.

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

pub trait FrameDriver: Interp { /* env alloc/free, IR queries, statement dispatch, call resolution */ }
```

`Frame` is anchored only on `FrameEngine` (a total `Error`), **not** on the
forward value engine `Interp` — so the frame protocol is decoupled from forward
value interpretation and reusable by other analyses. Every `Interp` is a
`FrameEngine` by blanket impl. The engine owns a `Vec<F>` and calls
`drive_frames`, which pops the top frame, `step`s it, and applies the returned
`FrameEffect`. `FrameDriver: Interp` is the richer capability surface the
*forward* frames call; **both forward engines implement it**. The concrete and
abstract standard frames are two *implementations* of this one protocol — not
parallel frameworks.

### Concrete frames — `ScopeFrame` / `CallFrame` / `StandardFrame`

`ConcreteInterpreter` is generic over the total frame type `F` (default
`StandardFrame`). A custom enum reuses the standard `ScopeFrame`/`CallFrame`
single-path traversal through `FrameBuild` (`from_scope`/`from_call`) and their
`*_into` delegating methods, adds observation or variants, and instantiates the
engine with that `F`. (Example: a `TracingFrame { Scope(ScopeFrame), Call(CallFrame) }`
counting call/scope visitation while running the real program — see
`example/toy-lang`'s `interpreter::tests::advanced`.)

### Abstract frames — `StandardAbstractFrame` / `AbstractFrameBuild` / `AbstractFrameDriver`

`AbstractInterpreter` is symmetrically generic over a total abstract frame type
`F` (default `StandardAbstractFrame`). The standard abstract frames
(`AbstractFunctionFrame`, `AbstractCfgFrame`, `AbstractScopeFrame`,
`AbstractScopeAlternativesFrame`, `AbstractCallFrame`) implement the *same*
`Frame` protocol, but their traversal is the abstract one: a CFG block worklist
that joins/widens at merge points, `Branch`/`EnterAny` exploration, scope
fixpoints to stability, and per-key call summarization. A custom enum reuses
them through `AbstractFrameBuild` and the `*_into` methods — exactly mirroring
the concrete pattern (see `TracingAbstractFrame` in the same test module).

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
   domain (`is_truthy`/`loop_condition` returning `None`) and surfaces as the
   undecided `ForwardEffect`/`ScopeStep` variants. Never write per-engine dialect
   impls or engine-specific dispatch traits.
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
- The per-statement effect is the associated type `I::Effect`, **per analysis**
  — forward execution/abstract interpretation use `ForwardEffect`. A future
  analysis (e.g. backward liveness) defines its **own** `Interp` flavor and its
  **own** `Effect` algebra (reusing the same frame protocol), rather than
  widening `ForwardEffect` or routing through it. Implementing such an analysis
  is deliberately **out of scope** here; this pass only consolidated the trait
  surface and *prepared* the seam (associated `Effect`, frames decoupled from
  `Interp`).
- Function-summary context sensitivity is a pluggable `CallContext` strategy.
  `ContextInsensitive` is the context-insensitive baseline; `ConstPropContext` provides bounded
  arg-tuple keys (precise recursion; sound, terminating cap to `Unknown`).
  Unbounded call-string (k-CFA) policies remain future work — another
  `CallContext` impl, no engine change.
- Inline `Scope::region(..)` bodies inside the abstract interpreter are not
  yet supported (function-entry regions are); no current dialect emits them.
- First-class function values (`Lambda`/`Bind` as values, `Callee` from an
  SSA value) are not yet supported by either engine.
