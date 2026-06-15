# Interpreter Framework

The interpreter framework (`kirin-interpreter`) supports concrete execution
and lattice-based abstract interpretation over the same dialect semantics,
including analyses that cross language boundaries in multi-stage pipelines.

The design is organized as a **two-persona contract**:

- **Dialect authors** describe what each statement *means*, once, in a small
  fixed vocabulary. They never see engines, stages, pipelines, frames, or
  fixpoints.
- **Compiler authors** compose languages into pipelines and *select*
  components: an engine, a value domain, an error type, and a linker. They
  never implement framework traits beyond derives.

Customizing *traversal* — a custom total frame type, or custom abstract
summary-key / join-widen policies — is an opt-in extension covered under
[Advanced](#advanced-custom-traversal-and-policies); it is not a separate
persona.

Every derive macro is named after the trait it implements
(`#[derive(Interpretable)]` → `trait Interpretable`), so learning the derive
is learning the trait.

## Dialect-author surface

Everything is exported from `kirin_interpreter::dialect`.

### `Interpretable<I>` — statement semantics

```rust
pub trait Interpretable<I: Interp>: Dialect {
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error>;
}
```

`I: Interp` is the interpreter context with two associated types: `I::Value`
(the value domain) and `I::Error` (the total error, `From<InterpreterError>`).
Implementations constrain only:

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

### `Effect` — the closed control algebra

```rust
pub enum Effect<V, E> {
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
write zero framework-trait impls:

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
the linker at `Effect::Call`, and the analysis lattice flows through
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
`Return`). The dialect-produced `Effect` is consumed by `ScopeFrame`, which
maps it to a `FrameEffect`. A custom `F`
([Advanced](#advanced-custom-traversal-and-policies)) replaces traversal
without touching the engine.

### `AbstractInterpreter<'ir, S, V, E, Lk, P = DefaultPolicy>`

Interprocedural fixpoint analyzer over a lattice `V: Widen + Lattice +
HasBottom`. Reads of unbound SSA values are `bottom` (unreached). The policy
`P` (`CallContext` for summary keys + `AbstractControl` for join/widen,
defaulting to the context-insensitive `DefaultPolicy`) is the customizable
seam ([Advanced](#advanced-custom-traversal-and-policies)); everything else is
engine-owned. Three nested fixpoints:

- **CFG**: each function body region is a block worklist; block parameters
  join across incoming edges and widen after `widen_after` visits — `cf`
  back-edge loops converge.
- **Scopes**: hook-driven scopes re-run with joined/widened entry arguments
  until stable — `scf.for` loops converge.
- **Functions**: each resolved call target is summarized under a key chosen by
  the `CallContext` policy (`DefaultPolicy` → `(stage, specialization)`), with an
  entry/return `Product<V>` summary. Calls join arguments into the callee's
  entry (enqueueing it on change) and read its current return summary
  (`bottom` until it converges); return-summary changes re-enqueue recorded
  callers — *including same-key (self-)recursion*, so a recursive function's
  rising return propagates back to its own call site (without this, recursion
  sees only the base case). Recursion converges by monotone iteration from
  `bottom`.

Analysis crates stay small: `kirin-constprop` is the `ConstPropValue` lattice, a
`ConstPropContext` policy (bounded arg-tuple context sensitivity), and
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

Two seams let an analysis customize *how* the engine traverses, without forking
it. Neither is visible to dialect authors.

### The frame layer — `Frame` / `FrameEffect` / `FrameBuild`

```rust
pub enum FrameEffect<F, C> { Continue(F), Push { parent: F, child: F }, Done, Complete(C) }

pub trait Frame<I: Interp>: Sized {            // implemented by the *total* frame enum
    type Completion;
    fn step(self, &mut I)        -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
    fn resume_done(self, &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
    fn resume(self, Self::Completion, &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error>;
}

pub trait FrameBuild<V, E> { fn from_scope(ScopeFrame<V, E>) -> Self; fn from_call(CallFrame<V>) -> Self; }
```

`ConcreteInterpreter` is generic over the total frame type `F` (default
`StandardFrame`). A custom analysis defines its own enum, reuses the standard
`ScopeFrame`/`CallFrame` traversal through `FrameBuild` and their `*_into`
delegating methods, adds its own observation or variants, and instantiates the
engine with that `F` — the engine is not forked. Frames consume the dialect
`Effect` and decide traversal; the engine just runs the stack. **Frame generics
never leak into `Interpretable<I>`**, so dialect authors are unaffected.

The capability surface a frame needs from its engine is `FrameDriver: Interp`
(env alloc/free, IR queries, statement dispatch, call resolution); both engines
implement it, so the same standard frames can drive both. This is also the
natural seam for a future **backward** (liveness) traversal: a new frame variant
over the same `FrameDriver`, not a new `Effect`.

(Example: a `TracingFrame { Scope(ScopeFrame), Call(CallFrame) }` that counts
call/scope visitation while running the real program correctly — see
`example/toy-lang`'s `interpreter::tests::advanced`.)

### Abstract policies — `CallContext` and `AbstractControl`

`AbstractInterpreter` is generic over a policy `P` providing two decisions:

```rust
pub trait CallContext<V>     { type Key: Eq + Hash + Clone;
                               fn key(&mut self, stage, function, args: &Product<V>) -> Self::Key; }
pub trait AbstractControl<V> { fn merge(&self, current, incoming, visits) -> Result<Product<V>, _>; }
```

`DefaultPolicy` is context-insensitive (`Key = (stage, specialization)`) and
joins-then-widens after `widen_after` visits. `kirin-constprop`'s
`ConstPropContext` keys distinct fully-constant argument tuples to distinct
summaries — bounded by a per-function budget, with overflow and non-constant
arguments collapsing to one shared `Unknown` context (joined → sound `Top`).
That is what makes recursive constant propagation precise
(`factorial(Const(5)) → Const(120)`) while staying sound and terminating on
unknown inputs.

## Design rules

1. **Dialects are engine-blind.** Undecidedness is expressed by the value
   domain (`is_truthy`/`loop_condition` returning `None`) and surfaces as the
   undecided `Effect`/`ScopeStep` variants. Never write per-engine dialect
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

- Backward dataflow analyses (liveness) are deliberately *not* expressed in
  this framework: they solve flow equations rather than execute, and belong
  in a separate direction-parametric dataflow solver sharing the lattice
  traits and use/def facts derivable from the IR model (`HasArguments`/
  `HasResults`). Planned as `kirin-dataflow`.
- Function-summary context sensitivity is a pluggable `CallContext` policy.
  `DefaultPolicy` is context-insensitive; `ConstPropContext` provides bounded
  arg-tuple keys (precise recursion; sound, terminating cap to `Unknown`).
  Unbounded call-string (k-CFA) policies remain future work — another
  `CallContext` impl, no engine change.
- The abstract engine exposes its join/widen and summary-key *policies* as
  components, but still owns the worklist *iteration* (CFG block worklist,
  scope loop, interprocedural worklist) inline rather than as `Frame`s.
  Unifying the abstract engine onto the same frame-stack driver as
  `ConcreteInterpreter` — so abstract explore/join lives in dedicated abstract
  frames reused via `FrameBuild` — is deferred.
- Inline `Scope::region(..)` bodies inside the abstract interpreter are not
  yet supported (function-entry regions are); no current dialect emits them.
- First-class function values (`Lambda`/`Bind` as values, `Callee` from an
  SSA value) are not yet supported by either engine.
