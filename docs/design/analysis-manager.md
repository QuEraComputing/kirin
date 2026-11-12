# Analysis Manager

This document designs the `AnalysisManager` and `PassRunner` in
`kirin-rewrite`: how rewrite passes get analysis results, such as constant
propagation, how those results are cached, and when they are thrown away.
The `AnalysisManager` computes and caches results; the `PassRunner` owns the
pipeline and the manager, runs each pass, and tells the manager when to drop
results.

The shape follows MLIR's `AnalysisManager` (see [Comparison with
MLIR](#comparison-with-mlir)), with one rule taken from `rewrite-engine.md`
instead of MLIR's: **any edit to the IR drops every cached result.**

## Terms

- **Analysis**: a computation that reads IR and produces facts, such as
  constant propagation ("`%b` is always 12") or demand ("nothing needs `%t`").
- **Result**: the value an analysis produces for one stage. The manager
  caches results; rules read them.
- **Pass**: the function `Pipeline::run_pass` runs to edit one stage in place
  through a `Rewriter`. A `RewriteDriver` runs inside a pass. A pass may run a
  driver once, several times, or alongside other edits, and the manager only
  sees the pass as a whole.
- **Stale**: a result computed on IR that has since been edited.
- **`S`**: the pipeline's stage type, as in `Pipeline<S>`: the enum of every
  stage the pipeline holds (e.g. `Source(StageInfo<SourceLanguage>)`,
  `Lowered(..)`).
- **`L`**: the language of one stage. A pass and a `RuleSet<L>` work on one
  `StageInfo<L>`.

## Goals

1. A rule can say which analyses it needs, and the manager computes them
   before the pass starts.
2. Results are computed once and reused by later passes as long as the IR
   has not changed.
3. A result computed before an edit is never handed to a later pass.
4. Constant propagation and demand are the first clients.

## Non-goals (deferred)

- Choosing which results an edit invalidates: no `PreservedAnalyses`, no
  per-analysis invalidation hook, no mapping from `MutationEvent`s to
  analyses. Every edit drops everything.
- Stage revisions and caches that outlive an edit.
- Computing an analysis lazily inside a pass.
- Caching per function inside a stage.
- A `Pass` trait, or a pass manager that holds a list of passes and orders
  them. The `PassRunner` runs one pass at a time, given by its caller.

## The core constraint: analyses need the pipeline, passes don't have it

The existing analyses read the whole pipeline: `analyze_demand(&pipeline, ..)`,
`analyze_dense(&pipeline, ..)` and `ConstProp::new(&pipeline)`. They need it
even when every call stays in one stage: they resolve calls through the
pipeline's function table, which `StageInfo` does not hold, and they look
stages up through the stage enum `S`. A pass, however, takes its stage *out of*
the pipeline (`run_pass`) and sees only a `Rewriter` over that one
`StageInfo<L>`; it cannot borrow the pipeline either, since `run_pass` holds
it mutably. So an analysis a pass needs must run **before** the pass starts,
and its result must be **owned** data, not a borrow of the pipeline.

This decides most of the design: results are computed outside passes, handed
to the pass read-only, and kept or dropped afterwards.
It also means the manager must know, before a pass, which analyses the pass
will read (see [Which analyses run before a
pass](#which-analyses-run-before-a-pass)).

## API

All in `kirin-rewrite`, module `analysis`.

```rust
/// Names an analysis and the type of its result. It does not mention `S`, so
/// a rule set can name an analysis and a rule can read its result.
pub trait AnalysisKey: 'static {
    type Output: 'static;
}

/// How to compute analysis `A` on a `Pipeline<S>`. `run` gets the manager, as
/// an MLIR analysis constructor gets `AnalysisManager&`, and computes the
/// analyses it depends on through it: `am.get::<Other>(pipeline, stage)?`.
pub trait Analysis<S>: AnalysisKey {
    /// What the caller chooses when registering the analysis, such as the
    /// roots to analyze from and their arguments.
    type Input: 'static;

    fn run(
        pipeline: &Pipeline<S>,
        stage: CompileStage,
        input: &Self::Input,
        am: &mut AnalysisManager<S>,
    ) -> Result<Self::Output, AnalysisError>;
}

/// The results a pass can read. Read-only.
impl Analyses {
    pub fn get_cached<A: AnalysisKey>(&self) -> Option<Rc<A::Output>>;
}

/// "This pass reads analysis `A`": its `TypeId` and name. Unlike
/// `A: Analysis<S>`, it doesn't mention the pipeline's stage type `S`, so a
/// `RuleSet<L>`, which only knows its language `L`, can hold one.
#[derive(Clone, Copy)]
pub struct Requirement { /* TypeId, &'static str */ }

impl Requirement {
    pub fn of<A: AnalysisKey>() -> Self;
}

/// Computes and caches results. It never runs passes.
pub struct AnalysisManager<S> {
    /* registered: TypeId -> (compute function for S, its input)
       cache:      CompileStage -> TypeId -> Rc<dyn Any> */
}

impl<S> AnalysisManager<S> {
    pub fn new() -> Self;

    /// Make `A` computable, from `input`: a pass can then require it, and
    /// `get` can compute it.
    pub fn register<A: Analysis<S>>(&mut self, input: A::Input) -> &mut Self;

    /// The cached result, or compute it from its registered input (and what
    /// it depends on) and cache it.
    pub fn get<A: Analysis<S>>(&mut self, pipeline: &Pipeline<S>, stage: CompileStage)
        -> Result<Rc<A::Output>, AnalysisError>;

    /// The cached result, never computing.
    pub fn get_cached<A: AnalysisKey>(&self, stage: CompileStage) -> Option<Rc<A::Output>>;

    // Dropping every result is crate-private: only the runner, which sees
    // every edit, decides when.
}

/// Owns the pipeline and its analysis manager, so every edit to the
/// pipeline goes through it and no cached result can outlive an edit.
pub struct PassRunner<S> {
    /* pipeline: Pipeline<S>, analyses: AnalysisManager<S> */
}

impl<S> PassRunner<S> {
    pub fn new(pipeline: Pipeline<S>, analyses: AnalysisManager<S>) -> Self;

    pub fn pipeline(&self) -> &Pipeline<S>;
    /// For edits that are not passes, such as parsing more functions in.
    /// Drops every cached result, since the runner cannot see what changes.
    pub fn pipeline_mut(&mut self) -> &mut Pipeline<S>;
    pub fn into_pipeline(self) -> Pipeline<S>;

    pub fn analyses(&self) -> &AnalysisManager<S>;
    /// `AnalysisManager::get` on the runner's own pipeline.
    pub fn get<A: Analysis<S>>(&mut self, stage: CompileStage)
        -> Result<Rc<A::Output>, AnalysisError>;

    /// Compute every analysis in `requires` that is not cached, then run
    /// `pass` on `stage` through `Pipeline::run_pass`, handing it the
    /// stage's results; drop every result if the pass edited the IR.
    pub fn run_pass<L, T, E>(
        &mut self,
        stage: CompileStage,
        requires: impl IntoIterator<Item = Requirement>,
        pass: impl FnOnce(&mut Rewriter<L>, &Analyses) -> Result<T, E>,
    ) -> Result<T, RunPassError>;
}

pub enum AnalysisError {
    /// The analysis itself failed.
    Failed { analysis: &'static str, source: Box<dyn Error + Send + Sync> },
    /// Computing `analysis` needed `analysis` again.
    Cycle { analysis: &'static str },
    /// `analysis` was required or asked for, but never registered.
    Unregistered { analysis: &'static str },
    UnknownStage,
}

/// Why `PassRunner::run_pass` failed.
pub enum RunPassError {
    /// A required analysis could not be computed. The pass did not run and
    /// the stage is untouched.
    Analysis(AnalysisError),
    /// `Pipeline::run_pass` failed.
    Pass(StagePassError),
}
```

An analysis is named by a **key** type separate from its result, as in LLVM's
new pass manager (`DominatorTreeAnalysis::Result = DominatorTree`). Two
analyses can produce the same result type: two forward analyses over one
lattice both produce the interpreter's `Analyzed<K, SSAValue, V>`. The cache
tells them apart by key. A key is usually an empty struct, e.g.
`struct ConstPropFacts;`.

Results are shared as `Rc` (`std::rc::Rc`, a reference-counted pointer: several
owners share one value without copying it, and it is freed when the last owner
drops it). So a rule can keep a result without borrowing the manager, and a
result the manager drops after an edit lives on until the rules holding it are
dropped. This matches the `Rc` the crate already uses for native functions.

The runner has its own `get` because computing needs the manager mutably and
the pipeline at once, and both live in the runner; a caller could not borrow
them separately, so the runner splits the borrow itself. Analyses are
registered before the manager goes into the runner, so the runner exposes the
manager read-only.

The split follows MLIR, where the `AnalysisManager` caches results and the
`PassManager` runs passes and tells it what to invalidate. Kirin's runner
also owns the pipeline, which MLIR's pass manager does not: that is what
guarantees no edit goes unseen.

### Example

```rust
let mut analyses = AnalysisManager::new();
analyses
    .register::<ConstPropFacts>(vec![(main, vec![Top])])
    .register::<DemandFacts>(vec![(main, vec![Demand::Live])]);
let mut runner = PassRunner::new(pipeline, analyses);

// The rule set says which analyses its rules read; the runner computes the
// missing ones, then runs the pass. Afterwards it drops them if anything
// changed.
runner.run_pass(stage, rules.required(), |rewriter, facts| driver.run(rewriter, facts))?;

// The fold edited the IR, so this pass gets a fresh constprop result.
runner.run_pass(stage, rules.required(), |rewriter, facts| driver.run(rewriter, facts))?;

let pipeline = runner.into_pipeline();
```

## Which analyses run before a pass

A pass can only read what was computed before it started, so the runner has
to know beforehand which analyses the pass will read. MLIR never faces this,
because its analyses run lazily inside the pass.

Rules already say what they read (`.requires::<A>()`, see [How rules get
results](#how-rules-get-results)), so the runner takes the list from the rule
set: `rules.required()`. One type problem stands in the way. A `RuleSet<L>`
knows the language `L`, not the pipeline's stage type `S`, so it can record
*which* analysis a rule needs, as a `Requirement` (a `TypeId` and a name), but
it cannot compute one: computing needs `A: Analysis<S>`.

The manager therefore keeps a **registry**, as LLVM's new pass manager does
(`FunctionAnalysisManager::registerPass`): `am.register::<A>(input)` stores,
under `A`'s `TypeId`, a function computing `A` for `S` and the input to
compute it from. Before a pass, `PassRunner::run_pass` looks each requirement
up and computes the ones not cached.

- A requirement that was never registered fails with
  `AnalysisError::Unregistered` **before** the stage leaves the pipeline, so
  the stage is untouched.
- A hand-written pass without a rule set names its needs directly:
  `[Requirement::of::<ConstPropFacts>()]`, or `[]` for none.
- `get::<A>()`, including inside another analysis's `run`, also needs `A`
  registered, since registration is where its input comes from. LLVM
  requires the same.

## Invalidation

After a pass, `run_pass` checks whether the pass's `Rewriter` recorded any
`MutationEvent`. A `Rewriter` is new for each pass, so any event means this
pass edited the IR.

| What happened | Cache afterwards |
|---|---|
| The pass recorded no event | kept, for every stage |
| The pass recorded an event | dropped, for **every** stage |
| The pass failed (stage quarantined) | dropped, for every stage |
| Nothing ran (`UnknownStage`, `DialectMismatch`, ...) | kept |

All stages are dropped, not only the edited one, because a result for stage A
can depend on stage B through cross-stage calls. Telling the two apart is a
later refinement.

A pass that edits nothing keeps every result automatically.

**No edit goes unseen.** The runner owns the pipeline, so code edits it in
one of three ways, and each keeps the cache honest:

- `run_pass`: dropped as in the table above.
- `pipeline_mut()`: drops everything at once, since the runner cannot see
  what the caller changes, e.g. parsing more functions in. This is coarse:
  adding a function drops results nothing invalidated.
- `into_pipeline()`: gives the pipeline back and discards the manager with it.

## Facts during a pass

A pass reads the results computed before it started, for its whole run, even
after its first edit. It never gets a recomputed result: recomputing needs the
pipeline, which the pass does not have. This matches `rewrite-engine.md`:
the manager never reruns an analysis during the mutation phase.

So inside a pass, results are a **snapshot** of the IR before the pass.
This holds across everything the pass does: in a pass that inlines calls and
then runs a driver, the driver reads the results from before the inlining,
since nothing drops them until the pass ends. Keeping their use correct is the rule author's job, as in MLIR. Two properties make that practical:

1. **Ids created during the pass have no facts.** A rule must treat a missing
   fact as "unknown".
2. **Many facts stay true under correct rewrites.** For example:

   ```text
   %a = add 1, 2        // constprop: %a = 3
   %b = mul %a, 4       // constprop: %b = 12
   ```

   Folding `%a` to a constant keeps "`%b` is 12" true, because the rewrite
   preserves meaning. So one pass can fold the whole chain with a single
   constprop run, instead of one run per fold.

Demand-based dead-code elimination is also safe on a snapshot, because
`Rewriter::erase_statement` refuses a statement whose results are still used.
A statement the snapshot calls undemanded, but which gained a use during the
pass, is therefore kept.

## How rules get results

As in MLIR, where a pattern receives analysis results in its constructor, a
native rule receives them in its **recipe**: the function the rule set calls
to build a fresh rule for each sweep (or each block). The rule keeps the
`Rc`s it needs. A rule also declares what it requires:

```rust
rules
    .add_statement_rule("fold", |facts: &Analyses| {
        ConstFold::new(facts.get_cached::<ConstPropFacts>().expect("required"))
    })
    .requires::<ConstPropFacts>();
```

`rules.required()` lists every rule's requirements, each once, for
`PassRunner::run_pass`. As a safety net, before its first sweep the
driver also checks every requirement against the `Analyses` it was given and
fails with `DriverError::MissingAnalysis` naming the rule and the analysis.
That catches a driver run through a plain `Pipeline::run_pass`, or a pass
handing the runner a different list. A rule's `apply` signature does not
change.

In M3 only native rules read results. Pattern rules can use the same recipe
approach later, if a client needs it.

The details (the exact recipe signature, and how the driver receives
`Analyses`) are settled in step 2.

## Roots and reads

The interpreter's results have a query-shaped API
([`interpreter/index.md`](interpreter/index.md#reading-results--analyzed-and-analysisresults)):
the caller gives the roots, and the result answers reads by key. No
memoization or query engine is involved; the shape only keeps the API
compatible with one later.

**The caller gives the roots.** An analysis runs from a callable, as
`analyze(stage, callee, args)` does, with its arguments (constprop) or its
demanded outputs (demand). The analysis does not pick roots itself. A root is
a callable, not an arbitrary statement: one can root at any function, but not
at a lone `scf.if`. Analyzing from `main` also reaches what `main` calls.

**Reads are keyed, at any statement the analysis reached.** That includes
statements in nested bodies, such as the arms of an `scf.if`, and in every
callee reached from a root. What a read is keyed by depends on where the
analysis attaches its facts:

| Analysis | Facts attach to | Reading at statement `s` |
|---|---|---|
| constprop, demand (sparse) | `SSAValue` | the fact on one of `s`'s results or operands |
| liveness (dense) | `ProgramPoint` | `Before(s)`, `After(s)`, `BlockEntry(b)`, `BlockExit(b)` |

**Joined reads cover every context of a callable.** An edit changes the one
body every context of a callable shares, so a rewrite needs a fact that holds
in all of them: the join over the callable's contexts, as
`AnalysisResults::joined_fact` computes. Per-context reads are also
available, for rules that need them. As the interpreter puts it, joined
answers claim nothing about callers the analysis did not visit.

M3 uses this API as follows.

**An analysis's `Input` lists its roots,** each with its arguments or
demanded outputs, as a plain `Vec<(Callee, Vec<V>)>`.

**One result covers every root.** A result is cached per stage and analysis,
not per root: since analyzing from `main` already reaches what `main` calls,
a callee needs no result of its own. With several roots, the result holds
one `Analyzed` per root, and a joined read joins across them.

**What no root reached has no facts.** A rule treats a missing fact as
unknown, the same as for ids created during a pass. Choosing roots that
cover what a pass rewrites is the caller's job.

Two parts rely on the [interpreter changes](#interpreter-changes-for-review).
The result is the interpreter's own `Analyzed`, in its owned form, so the
manager can keep it across passes. And a rule reads a joined fact **by value
alone**: every SSA value belongs to exactly one callable's body, so the join
over a callable's contexts is a join per value, and a rule need not first
work out which callable its statement sits in.

## First clients: constprop and demand

The pieces sit where the interpreter puts its own:

| Piece | Crate |
|---|---|
| `AnalysisKey`, `Analysis`, the manager, the runner | `kirin-rewrite` |
| The result type: the owned `Analyzed` | `kirin-interpreter` |
| Lattices and engine aliases | `kirin-constprop`, `kirin-demand`, unchanged |
| The keys (`ConstPropFacts`, `DemandFacts`) and their `Analysis<Stage>` impls | the language crate (`toy-lang`; the test languages for the tests) |

The impls go in the language crate because only it knows how to run an
analysis for its stages: the linker, the error type and its frame composition
(`ToyAbstractFrame`), as `toy-lang`'s `analyze_constprop` does today. Each
impl runs the analysis from every root in its input and keeps the resulting
`Analyzed`s. The analysis crates gain nothing, and `kirin-rewrite` depends on
none of them.

## Comparison with MLIR

| | MLIR | Kirin M3 |
|---|---|---|
| An analysis is | a type constructed from `Operation*` | a key type naming its result (`AnalysisKey`) and how to compute it (`Analysis::run`), as in LLVM |
| Dependencies | constructor takes `AnalysisManager&` | `run` takes `&mut AnalysisManager` |
| Runs passes and triggers invalidation | `PassManager` | `PassRunner` |
| Owns the IR | no | the runner owns the pipeline |
| Inputs | the op it is constructed from | caller-given roots, at registration |
| When computed | lazily, inside the pass | before the pass (needs the pipeline) |
| Knowing what to compute | not needed (lazy) | rules' requirements + a registry, as in LLVM |
| Reading inside a pass | `getAnalysis`, `getCachedParentAnalysis` | `get_cached` only, like a parent analysis |
| Scope | per op, nested | per stage |
| After an editing pass | drop unless marked preserved | drop everything |
| After a pass with no edits | drop unless marked preserved | keep everything |
| Facts after an edit in the pass | pass author's job | pass author's job (snapshot) |
| Patterns get facts | through their constructor | through their recipe |

## Implementation steps

Each step is reviewed before the next starts.

1. **Core types.** `AnalysisKey`, `Analysis`, `Analyses`, `Requirement`,
   `AnalysisManager` (with `register`), `PassRunner`, `AnalysisError`,
   `RunPassError`, with cycle detection. Tests in `crates/kirin-rewrite/tests/analysis_manager.rs`
   with test analyses counting their runs: compute once then reuse;
   `get_cached` before `get`; dependencies through the manager; cycles;
   `run` receives its registered input; `run_pass` computes registered
   requirements; an unregistered requirement fails before the pass and leaves
   the stage untouched; `get` on an unregistered analysis fails; a pass with no edits
   keeps results; a pass with an edit drops all stages; a failed pass drops
   everything; `pipeline_mut()` drops everything; a pass reads the results
   computed before it.
2. **Rules read results.** Recipes receive `&Analyses`; `.requires::<A>()`;
   `RuleSet::required()`; the driver checks requirements and fails with
   `MissingAnalysis`.
3. **Constprop client.** Needs the [interpreter
   changes](#interpreter-changes-for-review). The `ConstPropFacts` key and its
   `Analysis` impl for a test language; a test-only constant-folding rule.
4. **Demand client.** The `DemandFacts` key and its `Analysis` impl for a
   test language; a test-only demand-based dead-code rule.
5. **Docs.** Update the M3, `AnalysisManager` and `NativeRule` sections of
   `rewrite-engine.md`.

## Interpreter changes (for review)

M3 caches the interpreter's analysis results, which needs one change in
`kirin-interpreter`.

**What.** `analyze` returns an owned `Analyzed`: plain data holding the fact
store, the callable and response of each reached context, the root, and the
callable → contexts index. Today `Analyzed<I>` wraps the converged engine `I`.

- One `AnalysisResults` impl on the owned `Analyzed` replaces the three
  per-engine impls. Its methods stay the same.
- Demand's `response`, which reads the parameters from the IR on each call,
  is computed once when `analyze` finishes. `is_demanded` moves onto the
  owned `Analyzed`.
- A per-value joined read is added to `AnalysisResults`: the join of a value's
  fact over every context that has it, so a reader need not know the value's
  callable.

**Why.** The engine borrows the pipeline, so today's `Analyzed` does too.
While it is alive, nothing can edit the pipeline, so the analysis manager
cannot keep it across passes. An owned result lets the manager cache the
interpreter's own result, giving one result type for every analysis, current
and downstream, instead of a copy type per analysis on the rewrite side. Every
client reads it through the same `AnalysisResults` API. LLVM's and MLIR's
analysis results are owned objects in the same way.

**What does not change.** How analyses compute (engines, worklists, frames,
dialect rules); the `AnalysisResults` methods; callers that let the type be
inferred, which is nearly all of them; the analysis crates. `toy-lang` names
`Analyzed<..>` in three signatures, which change. Roughly 150 lines go and 155
come in `kirin-interpreter`, as the three impls collapse into one.

**Tests.** The existing tests reading through `AnalysisResults` must pass
unchanged. One new test analyzes, edits the pipeline, then reads the result,
which compiles only if the result no longer borrows the pipeline.

## Open question: results per stage or per pipeline

Is "which stage" part of an analysis itself, or part of how its result is
looked up?

- **Part of the lookup (this design).** The cache key is the stage and the
  analysis, and `run`, `get` and `get_cached` take a stage. The same
  analysis type has a separate result for each stage. But a root at one
  stage can reach callables at others through cross-stage calls, so "the
  result for stage X" may hold facts about other stages too.
- **Part of the analysis.** An analysis is a value naming its roots, each at
  its stage, e.g. `register(ConstPropFacts { roots })`, as an LLVM analysis
  is a constructed object with a `run` method. There is one result per
  analysis type for the whole pipeline; `run`, `get` and `get_cached` take
  no stage, and the separate `Input` type goes away. Two results of one
  analysis, such as one per stage, need two types.

Invalidation is the same either way: any edit drops every result.
