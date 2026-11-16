# Analysis Manager

This document designs M3's analysis manager, the `Database` in
`kirin-rewrite`: how rewrite passes get analysis results, such as constant
propagation, how those results are cached, and when they are thrown away.
The database owns the pipeline and the results computed from it, as a salsa
database owns its inputs and its memoized queries. It computes results, runs
each pass, and drops results when a pass edits the IR.

It draws on MLIR's `AnalysisManager` and on salsa (see [Comparison with
MLIR](#comparison-with-mlir) and [Toward query-based
compilation](#toward-query-based-compilation)), with one rule taken from
`rewrite-engine.md` instead of MLIR's: **any edit to the IR drops every
cached result.**

## Terms

- **Analysis**: a computation that reads IR and produces facts, such as
  constant propagation ("`%b` is always 12") or demand ("nothing needs `%t`").
- **Key**: what one result of an analysis is for, chosen by whoever asks
  for it: a stage, or a set of roots each at its stage. Each distinct key
  gets its own result.
- **Result**: the value an analysis produces for one key. The database
  caches results; rules read them.
- **Pass**: the function `Pipeline::run_pass` runs to edit one stage in place
  through a `Rewriter`. A `RewriteDriver` runs inside a pass. A pass may run a
  driver once, several times, or alongside other edits, and the database only
  sees the pass as a whole.
- **Revision**: a counter the database bumps once for each pass that edits
  the pipeline, and on each `pipeline_mut()`. Each result records the
  revision it was computed at.
- **Stale**: a result computed at an earlier revision, that is, on IR that
  has since been edited.
- **`S`**: the pipeline's stage type, as in `Pipeline<S>`: the enum of every
  stage the pipeline holds (e.g. `Source(StageInfo<SourceLanguage>)`,
  `Lowered(..)`).
- **`L`**: the language of one stage. A pass and a `RuleSet<L>` work on one
  `StageInfo<L>`.

## Goals

1. A rule declares the queries it reads, each an analysis and a key, and the
   database answers them before the pass starts.
2. Results are computed once and reused by later passes as long as the IR
   has not changed.
3. A result computed before an edit is never handed to a later pass.
4. Constant propagation and demand are the first clients.

## Non-goals (deferred)

- Choosing which results an edit invalidates: no `PreservedAnalyses`, no
  per-analysis invalidation hook, no mapping from `MutationEvent`s to
  analyses. Every edit drops everything.
- Tracking when each function body last changed, and results that outlive
  an edit (see [Toward query-based
  compilation](#toward-query-based-compilation)).
- Updating a stale result in place rather than recomputing it.

## The core constraint: analyses need the pipeline, passes don't have it

The existing analyses read the whole pipeline: every interpreter engine is
built from it, as in `ConstProp::new(&pipeline)`, and so are `toy-lang`'s
`analyze_constprop(&pipeline, ..)` and `analyze_demand(&pipeline, ..)`. They
need it even when every call stays in one stage: they resolve calls through
the pipeline's function table, which `StageInfo` does not hold, and they look
stages up through the stage enum `S`. A pass, however, takes its stage *out
of* the pipeline (`run_pass`) and sees only a `Rewriter` over that one
`StageInfo<L>`; it cannot borrow the pipeline either, since `run_pass` holds
it mutably. So an analysis a pass needs must run **before** the pass starts,
and its result must be **owned** data, not a borrow of the pipeline.

This decides most of the design: results are computed outside passes, handed
to the pass read-only, and kept or dropped afterwards.
It also means the database must know, before a pass, which analyses the pass
will read (see [Which analyses run before a
pass](#which-analyses-run-before-a-pass)).

## API

All in `kirin-rewrite`, module `analysis`.

```rust
/// Names an analysis, the key it is asked with, and the type of its result.
/// It does not mention `S`, so a rule set can name an analysis and its key,
/// and a rule can read its result.
pub trait AnalysisKind: 'static {
    /// What one result is for, such as a stage or a set of roots.
    type Key: Clone + Eq + Hash + Send + Sync + 'static;
    type Output: Send + Sync + 'static;
}

/// How to compute analysis `A` on a `Pipeline<S>`. `run` gets the database
/// shared, as a salsa query gets `&db`: it reads the IR through
/// `db.pipeline()` and gets the analyses it depends on through
/// `db.get::<Other>(&key)?`.
pub trait Analysis<S>: AnalysisKind {
    fn run(db: &Database<S>, key: &Self::Key)
        -> Result<Self::Output, AnalysisError>;
}

/// Cached results, read-only. A pass and its rules read them.
impl AnalysisCache {
    pub fn get<A: AnalysisKind>(&self, key: &A::Key) -> Option<Arc<A::Output>>;
}

/// "This pass reads analysis `A` for `key`". Unlike `A: Analysis<S>`, it
/// doesn't mention the pipeline's stage type `S`, so a `RuleSet<L>`, which
/// only knows its language `L`, can hold one.
#[derive(Clone)]
pub struct Requirement { /* TypeId, name, the key (type-erased) */ }

impl Requirement {
    pub fn of<A: AnalysisKind>(key: A::Key) -> Self;
}

/// Owns the pipeline and the results computed from it. Every edit to the
/// pipeline goes through it, so no cached result outlives an edit.
pub struct Database<S> {
    /* pipeline:   Pipeline<S>
       registered: TypeId -> compute function for S
       revision:   u64, the pipeline's revision
       cache:      RefCell<AnalysisCache>, mapping (TypeId, key) to
                   (Arc<dyn Any + Send + Sync>, revision computed at) */
}

impl<S> Database<S> {
    pub fn new(pipeline: Pipeline<S>) -> Self;

    /// Let passes require `A`. `get` needs no registration.
    pub fn register<A: Analysis<S>>(&mut self) -> &mut Self;

    /// The cached result for `key`, or compute it (and what it depends on)
    /// and cache it. A failure is returned, not cached: the next `get` runs
    /// the analysis again.
    pub fn get<A: Analysis<S>>(&self, key: &A::Key)
        -> Result<Arc<A::Output>, AnalysisError>;

    /// The cached result for `key`, never computing.
    pub fn get_cached<A: AnalysisKind>(&self, key: &A::Key)
        -> Option<Arc<A::Output>>;

    pub fn pipeline(&self) -> &Pipeline<S>;
    /// For edits that are not passes, such as parsing more functions in.
    /// Bumps the revision, since the database cannot see what changes.
    pub fn pipeline_mut(&mut self) -> &mut Pipeline<S>;
    pub fn into_pipeline(self) -> Pipeline<S>;

    /// How many passes have edited the pipeline, plus how many times
    /// `pipeline_mut` was called, since the database was created.
    pub fn revision(&self) -> u64;

    /// Compute every requirement in `requires` that is not cached, then run
    /// `pass` on `stage` through `Pipeline::run_pass`, handing it the cached
    /// results; bump the revision if the pass edited the IR.
    pub fn run_pass<L, T, E>(
        &mut self,
        stage: CompileStage,
        requires: impl IntoIterator<Item = Requirement>,
        pass: impl FnOnce(&mut Rewriter<L>, &AnalysisCache) -> Result<T, E>,
    ) -> Result<T, RunPassError>;
}

pub enum AnalysisError {
    /// The analysis itself failed.
    Failed { analysis: &'static str, source: Box<dyn Error + Send + Sync> },
    /// Computing `analysis` for a key needed `analysis` for the same key
    /// again. The same analysis for a different key is not a cycle.
    Cycle { analysis: &'static str },
    /// A pass required `analysis`, but it was never registered.
    Unregistered { analysis: &'static str },
    /// The key names a stage the pipeline does not hold, or one a failed
    /// pass quarantined. `run` returns it.
    UnknownStage,
}

/// Why `Database::run_pass` failed.
pub enum RunPassError {
    /// A required analysis could not be computed. The pass did not run and
    /// the stage is untouched.
    Analysis(AnalysisError),
    /// `Pipeline::run_pass` failed.
    Pass(StagePassError),
}
```

An analysis is named by a type separate from its result, as in LLVM's new
pass manager (`DominatorTreeAnalysis::Result = DominatorTree`). Two analyses
can produce the same result type: two forward analyses over one lattice both
produce the interpreter's `Analyzed<K, SSAValue, V>`, and both may take
the same roots as their key. The cache tells them apart by the naming type,
which is usually an empty struct, e.g. `struct ConstPropFacts;`, and the
results of one analysis by key.

Results are shared as `Arc` (`std::sync::Arc`, a reference-counted pointer:
several owners share one value without copying it, and it is freed when the
last owner drops it). So a rule can keep a result without borrowing the
database, and a result the database drops after an edit lives on until the
rules holding it are dropped. `Arc`, unlike `Rc`, can be shared across threads,
which is why results and keys are `Send + Sync`; salsa requires the same
of the values it stores.

Computing takes the database by shared reference, as a salsa query takes
`&db`, and never mutably. The cache sits behind interior mutability, so
`get` fills it through `&self`, and an analysis can ask for the analyses it
depends on in the middle of its own `run`. Only `register`, `run_pass` and
`pipeline_mut` take `&mut`, so none of them can happen while an analysis
runs: `run` only has `&Database`. No borrow of the cache leaves the
database: `get` and `get_cached` return an `Arc`, and `get` releases its
borrow before calling `A::run`, so an analysis's own `get` calls can fill the
cache. The cell is a `RefCell` while passes run on one thread; a lock can
replace it without changing the API.

A failed computation is not cached. `AnalysisError` holds a
`Box<dyn Error>`, so one error cannot be handed to two callers; the next
`get` runs the analysis again.

Outside a pass, `Database::get_cached` reads without computing; inside one,
the pass reads the same results through the `&AnalysisCache` it is given.

One type owns both the pipeline and the results, as a salsa database owns
its inputs and memoized queries. Owning the pipeline is what guarantees no
edit goes unseen: the only ways to edit it are the database's own `&mut`
methods.

### Keys

The caller passes a key with every request, and the cache holds one result
per analysis and key. The analysis author picks the key type to fit what the
analysis is about, and the key names the stage wherever the analysis needs
one, as `analyze(stage, callee, args)` takes the stage as an input.

Which stage a call lands in is decided by the analysis's *linker*, the
interpreter component that resolves a callee to a function at some stage
([`interpreter/index.md`](interpreter/index.md#linkers-calling-conventions-as-components)).
The default, `SameStageLinker`, resolves every call at the caller's own
stage; `CrossStageLinker` resolves it at whichever stage defines the callee.

A stage-wide analysis covers every function of one stage, with
`SameStageLinker`. Its key is the stage:

```rust
impl AnalysisKind for StageConstProp {
    type Key = CompileStage;
    type Output = /* .. */;
}

db.get::<StageConstProp>(&source)?;    // one result
db.get::<StageConstProp>(&lowered)?;   // another result
```

A root-based analysis starts from given roots, each at its stage, and may
follow calls across stages with `CrossStageLinker`. Its key maps each root
function, at its stage, to its arguments:

```rust
impl AnalysisKind for ConstPropFacts {
    type Key = BTreeMap<(CompileStage, Callee), Vec<ConstPropValue<i64>>>;
    type Output = /* .. */;
}

let from_main = BTreeMap::from([((source, main), vec![Top])]);
let from_helper = BTreeMap::from([((source, helper), vec![Const(3)])]);
db.get::<ConstPropFacts>(&from_main)?;     // one result, which may span stages
db.get::<ConstPropFacts>(&from_helper)?;   // a second result
```

The key must be `Clone + Eq + Hash`, since the cache looks results up by it.
Asking with an equal key returns the cached result.

Roots have no order, so a root-based key is a map, not a list. A
`BTreeMap` keeps its entries sorted by arena id, which carries no meaning
but is deterministic, so the same roots give the same key whatever order
they are added in, and its `Eq` and `Hash` follow from that. A map also
holds one root per function, which fits entry points: each is listed once,
with conservative facts. This needs `Ord` on `CompileStage` and `Callee`,
which order by arena id; the argument values need only `Eq + Hash`.

This is the shape of a query in salsa, `fn query(db, key) -> Output`: one
result per key, asked for by key, with `run` getting the database shared.
Moving onto a query engine later then changes how the database caches and
invalidates results, not which keys analyses take or how they ask.

### Example

```rust
let mut db = Database::new(pipeline);
db.register::<ConstPropFacts>().register::<DemandFacts>();

// The rule set says which analyses its rules read; the database computes the
// missing ones, then runs the pass. Afterwards it drops them if anything
// changed.
db.run_pass(stage, rules.required(), |rewriter, cache| driver.run(rewriter, cache))?;

// The fold edited the IR, so this pass gets a fresh constprop result.
db.run_pass(stage, rules.required(), |rewriter, cache| driver.run(rewriter, cache))?;

let pipeline = db.into_pipeline();
```

## Which analyses run before a pass

A pass can only read what was computed before it started, so the database has
to know beforehand which analyses the pass will read. MLIR never faces this,
because its analyses run lazily inside the pass.

Rules already say what they read (`.requires::<A>(key)`, see [How rules get
results](#how-rules-get-results)), so the database takes the list from the
rule set: `rules.required()`. One type problem stands in the way. A
`RuleSet<L>` knows the language `L`, not the pipeline's stage type `S`, so it
can record *which* analysis a rule needs and for which key, as a
`Requirement`, but it cannot compute one: computing needs `A: Analysis<S>`.

The database therefore keeps a **registry**, as LLVM's new pass manager does
(`FunctionAnalysisManager::registerPass`): `db.register::<A>()` stores, under
`A`'s `TypeId`, a function computing `A` for `S` from a key. Before a pass,
`Database::run_pass` looks each requirement up and computes the ones not
cached. A pass can require several analyses, and one analysis for several
keys.

- A requirement that was never registered fails with
  `AnalysisError::Unregistered` **before** the stage leaves the pipeline, so
  the stage is untouched.
- A hand-written pass without a rule set names its needs directly:
  `[Requirement::of::<ConstPropFacts>(roots)]`, or `[]` for none.
- `get::<A>(&key)`, including inside another analysis's `run`, needs no
  registration: it names `A` by type, so it calls `A::run` directly, as
  calling a salsa query runs it. Only a requirement, which has lost `A`'s
  type, goes through the registry.

## Invalidation

After a pass, `run_pass` checks whether the pass's `Rewriter` edited the IR.
It reads the rewriter's `edited` flag, which every mutation sets and nothing
clears, rather than its event log, which a pass can empty with
`drain_events`. A `Rewriter` is new for each pass, so the flag says whether
this pass edited. The flag is a small addition to `kirin-ir`:
`Rewriter::edited()`.

| What happened | Cache afterwards |
|---|---|
| The pass made no edit | kept, every result |
| The pass edited the IR | dropped, **every** result |
| The pass failed (stage quarantined) | dropped, every result |
| Nothing ran (`UnknownStage`, `DialectMismatch`, ...) | kept |

A failed pass drops everything because nothing rolls its edits back: it may
have left some of them in place, and its stage is quarantined.

Every result is dropped, including those whose keys name other stages,
because a result can depend on any stage through cross-stage calls. Telling
them apart is a later refinement.

**Revisions.** Dropping is done with a revision counter, as in salsa. The
database bumps it once for each pass that edits the pipeline, and each cached
result records the revision it was computed at. A lookup returns only results
computed at the current revision. Bumping also frees the stale results, so the
cache never holds them. With the whole pipeline as one input this behaves
exactly as clearing the cache. Salsa also keeps one revision, but treats each
input separately, recording the revision it last changed at; making each
function body such an input is how this later becomes selective (see [Toward
query-based compilation](#toward-query-based-compilation)).

**No edit goes unseen.** The database owns the pipeline, so code edits it in
one of three ways, and each keeps the cache honest:

- `run_pass`: dropped as in the table above.
- `pipeline_mut()`: bumps the revision, dropping everything at once, since
  the database cannot see what the caller changes, e.g. parsing more
  functions in. This is coarse: adding a function drops results nothing
  invalidated.
- `into_pipeline()`: gives the pipeline back and discards the cached results
  with the database.

Arena compaction (`Arena::gc`), which `rewrite-engine.md` allows only at pass
boundaries, renumbers ids. It therefore drops every result. Keys hold ids too,
such as `Callee::Specialized`, so compaction must also remap the keys that
rules and requirements hold, or rebuild them.

## Facts during a pass

A pass reads the results computed before it started, for its whole run, even
after its first edit. It never gets a recomputed result: recomputing needs the
pipeline, which the pass does not have. This matches `rewrite-engine.md`:
no analysis reruns during the mutation phase.

So inside a pass, results are a **snapshot** of the IR before the pass. This
holds across everything the pass does: in a pass that inlines calls and then
runs a driver, the driver reads the results from before the inlining, since
nothing drops them until the pass ends. Keeping their use correct is the rule
author's job, as in MLIR. Two properties make that practical:

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
`Arc`s it needs. A rule also declares what it requires:

```rust
// Sound only if `main` is the program's only entry point.
let roots = BTreeMap::from([((stage, main), vec![Top])]);
rules
    .add_statement_rule("fold", {
        let roots = roots.clone();
        move |cache: &AnalysisCache| {
            let facts = cache.get::<ConstPropFacts>(&roots).expect("required");
            ConstFold::new(facts)
        }
    })
    .requires::<ConstPropFacts>(roots);
```

A rule is built knowing the keys it reads. A rule that reads several
analyses, or one analysis for several keys, declares each with its own
`.requires`.

`rules.required()` lists every rule's requirements, each once, for
`Database::run_pass`. As a safety net, before its first sweep the driver also
checks every requirement against the `AnalysisCache` it was given and fails
with `DriverError::MissingAnalysis` naming the rule and the analysis. That
catches a driver run through a plain `Pipeline::run_pass`, or a pass handing
`run_pass` a different list. A rule's `apply` signature does not change.

In M3 only native rules read results. Pattern rules can use the same recipe
approach later, if a client needs it.

The details (the exact recipe signature, and how the driver receives
`AnalysisCache`) are settled in step 2.

## Roots and reads

The interpreter's results have a query-shaped API
([`interpreter/index.md`](interpreter/index.md#reading-results--analyzed-and-analysisresults)):
an analysis runs from given roots, and the result answers reads by context,
value or program point. The interpreter itself memoizes nothing; the
database caches its results.

**The caller gives the roots, through the key.** A root is a callable with its
arguments (constprop) or its demanded outputs (demand), as `analyze(stage,
callee, args)` takes. A root-based analysis's key maps each root to its
arguments; a stage-wide analysis's key is a stage, and its roots are every
function of that stage. Either way the key alone decides what is analyzed: the
analysis picks nothing on its own. A root is a callable, not an arbitrary
statement: one can root at any function, but not at a lone `scf.if`. Analyzing
from `main` also reaches what `main` calls.

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

**One result covers every root in its key.** Since analyzing from `main`
already reaches what `main` calls, a callee needs no result of its own. With
several roots, the result holds one `Analyzed` per root, and a joined read
joins across them.

**What no root reached has no facts.** A rule treats a missing fact as
unknown, the same as for ids created during a pass.

**The roots must cover every entry point.** A rewrite edits a callable's one
body, so a fact justifies it only if the fact holds for every way the
callable can be entered. Joined reads cover only the callers the analysis
visited, so the roots must include every entry point of the program, meaning
every function that code outside the program can call, each with
conservative boundary facts: unknown (`Top`) arguments for constprop, and
every output demanded for demand. For example, if `main` calls `helper(3)`
but outside code may call `helper(4)`, the roots `[main]` let a fold rule
treat `helper`'s argument as 3, which breaks the outside call; the roots
`[main, helper]`, with unknown arguments for `helper`, do not.

By default every function is an entry point: a stage-wide analysis roots
every function of its stage with conservative facts, so it is sound with no
input from the caller. A caller who knows the program's real entry points,
such as only `main`, can list them in a root-based key for more precise
facts. That is sound only if the list is complete.

Two parts rely on the [interpreter changes](#interpreter-changes-for-review).
The result is the interpreter's own `Analyzed`, in its owned form, so the
database can keep it across passes. And a rule reads a joined fact **by
stage and value**: an SSA value belongs to exactly one callable's body at its
stage, so the join over a callable's contexts is a join per value, and a rule
need not first work out which callable its statement sits in. The stage is
needed because SSA ids are allocated per stage: `%5` at `source` and `%5` at
`lowered` are different values, and a cross-stage result holds both.

## First clients: constprop and demand

An analysis's result can be the interpreter's own: `ConstPropFacts::run`
calls `analyze(stage, callee, args)` for each root and returns the owned
`Analyzed` values it gets, with no copy into a rewrite-side type.

Each piece goes in the crate that already holds the matching interpreter
code:

| Piece | Crate |
|---|---|
| `AnalysisKind`, `Analysis`, `AnalysisCache`, `Requirement`, `Database` | `kirin-rewrite` |
| The result type: the owned `Analyzed` | `kirin-interpreter` |
| Lattices and engine aliases | `kirin-constprop`, `kirin-demand`, unchanged |
| The analysis types (`ConstPropFacts`, `DemandFacts`) and their `Analysis<Stage>` impls | the language crate (`toy-lang`; the test languages for the tests) |

The impls go in the language crate because only it knows how to run an
analysis for its stages: the linker, the error type and its frame composition
(`ToyAbstractFrame`), as `toy-lang`'s `analyze_constprop` does today. Each
impl runs the analysis from every root in its key and keeps the resulting
`Analyzed`s. The analysis crates gain nothing, and `kirin-rewrite` depends on
none of them.

## Comparison with MLIR

| | MLIR | Kirin M3 |
|---|---|---|
| An analysis is | a type constructed from `Operation*` | a type naming its key and result (`AnalysisKind`) and how to compute it (`Analysis::run`), as in LLVM |
| Dependencies | constructor takes `AnalysisManager&` | `run` takes `&Database`, as a salsa query takes `&db` |
| Caches results | `AnalysisManager` | `Database` |
| Runs passes and triggers invalidation | `PassManager`, a separate type | `Database::run_pass`, the same type |
| Owns the IR | no | the database owns the pipeline |
| Inputs | the op it is constructed from | a key the caller passes with each request: a stage, or roots |
| When computed | lazily, inside the pass | before the pass (needs the pipeline) |
| Knowing what to compute | not needed (lazy) | rules' requirements + a registry, as in LLVM |
| Reading inside a pass | `getAnalysis`, `getCachedParentAnalysis` | `AnalysisCache::get` only, like a parent analysis |
| Scope | per op, nested | per key |
| After an editing pass | drop unless marked preserved | drop everything (bump the revision) |
| After a pass with no edits | drop unless marked preserved | keep everything |
| Facts after an edit in the pass | pass author's job | pass author's job (snapshot) |
| Patterns get facts | through their constructor | through their recipe |

## Toward query-based compilation

The database has the shape of a query system such as salsa: an analysis is a
function of a key, results are cached per key, an analysis gets the ones it
depends on through `&Database`, and edits happen only between passes. What
it lacks is selective invalidation, keeping the results an edit did not
affect. That needs each function body to be an input recording the revision
it last changed at, as salsa's inputs do, and each result to record what it
read: function bodies, and how its calls resolved, such as the function
table and specializations a linker consulted. IR reads would go through
tracked accessors, as in salsa, so that nothing goes unrecorded; today
`db.pipeline()` is untracked. Adding this does not change how analyses are
asked for; an analysis keeps whole-pipeline invalidation until its reads are
recorded.

## Implementation steps

Each step is reviewed before the next starts.

1. **Core types.** `AnalysisKind`, `Analysis`, `AnalysisCache`, `Requirement`,
   `Database`, `AnalysisError`, `RunPassError`, with cycle detection,
   `Rewriter::edited` in `kirin-ir`, and `Ord` on the IR id types and
   `Callee`, ordered by arena id. Tests in
   `crates/kirin-rewrite/tests/analysis_database.rs` with test analyses
   counting their runs: compute once then reuse; `get_cached` never computes;
   a failure is not cached; distinct keys get distinct results; equal keys
   share one; the same roots added in two orders share one; dependencies
   through the database; cycles; `run_pass` computes every requirement,
   including one analysis for two keys; an unregistered requirement fails
   before the pass and leaves the stage untouched; `get` computes an
   unregistered analysis; a pass with no edits keeps results and the revision;
   a pass with an edit bumps the revision and drops every result, whatever its
   key; a pass that edits and then drains its events still bumps the revision;
   a failed pass drops everything; `pipeline_mut()` drops everything; a pass
   reads the results computed before it.
2. **Rules read results.** Recipes receive `&AnalysisCache`;
   `.requires::<A>(key)`; `RuleSet::required()`; the driver checks
   requirements and fails with `MissingAnalysis`.
3. **Constprop client.** Needs the [interpreter
   changes](#interpreter-changes-for-review). The `ConstPropFacts` analysis
   type and its `Analysis` impl for a test language; a test-only
   constant-folding rule.
4. **Demand client.** The `DemandFacts` analysis type and its `Analysis`
   impl for a test language; a test-only demand-based dead-code rule.
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
- A per-value joined read is added to `AnalysisResults`, keyed by stage and
  value, `(CompileStage, SSAValue)`: the join of the value's fact over every
  context at that stage that has it, so a reader need not know the value's
  callable. Program points are qualified by stage the same way. The stage is
  part of the key because SSA ids are allocated per stage, so two stages can
  hold the same id for different values.

**Why.** The engine borrows the pipeline, so today's `Analyzed` does too.
While it is alive, nothing can edit the pipeline, so the database cannot
keep it across passes. An owned result lets the database cache the
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
which compiles only if the result no longer borrows the pipeline. Another
analyzes across two stages whose SSA ids overlap and checks that the
per-value read returns each stage's own facts.
