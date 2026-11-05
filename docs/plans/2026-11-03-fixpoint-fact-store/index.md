# Fixpoint fact store: per-context facts and one update path (B2–B6)

Status: **B2 design, revised at B3 review; B3–B5 landed; B6 landed
(uncommitted).** B3 adds
[`Lattice::join_assign`](../../../crates/kirin-ir/src/lattice.rs) plus
`EnvStore::join`, `EnvStore::contexts` and `FactStore::get_mut`
([store.rs](../../../crates/kirin-interpreter/src/core/env/store.rs)). B4
moves demand onto them (§15), B5 liveness (§16), B6 constprop (§17). B2 checked an earlier version of this design
with a throwaway test-only prototype, which is not in the tree.

## 1. Problem

The three abstract engines store and update facts three ways:

- **Constprop** (sparse forward) keeps SSA facts in `EnvStore`, a second copy
  of each block's outputs in `BlockSummary.outputs`, and function returns in
  `FunctionSummary`. It merges by hand in `apply_update`.
- **Demand** (sparse backward) keeps one flat driver table keyed
  `(context, SSAValue)`. Building `DemandResult` scans it once per context,
  O(C × F).
- **Liveness** (dense backward) keeps `BlockLiveness` summaries and a
  separate point-fact store.

Target: every fact lives in one place, its environment's `FactStore`, and
every analysis update goes through one merge path that reports whether the
fact rose.

## 2. Decisions

1. Reuse `kirin_ir::Lattice`, with a default
   `join_assign(&mut self, &Self) -> bool` that domains may override.
2. Widening is deferred; updates use ordinary joins (§9).
3. **Everything is an anchored fact.** Each environment's `FactStore<A, V>`
   holds every fact of its context, with one value type `V`. There is no
   separate summary slot and no enum payload. A callable's response is stored
   at anchors too (§6, §7).
4. **One authoritative representation per query role.** Callers read the
   callee's anchors. Nothing is duplicated, so nothing needs synchronizing.
5. `EnvStore::write` stays assignment; analysis updates use `EnvStore::join`.
6. Fact identity is `(EnvIndex, anchor)`. It is separate from work identity,
   which each engine chooses.
7. Demand results visit each context and its own facts once: O(C + F).
8. Return-site routing, return arity and discovery state are private
   metadata, not facts.

B2 had proposed a typed per-environment summary slot (`EnvStore<K, A, V, S>`)
and a demand parameter summary kept in sync with the entry parameters. The B3
review replaced both with anchors (§11).

## 3. Storage

```rust
impl<K, A, V> EnvStore<K, A, V> {
    pub fn join(&mut self, env: EnvIndex, anchor: A, incoming: &V) -> Result<bool, _>
    where V: Lattice + HasBottom + Clone;                                // new
    pub fn contexts(&self) -> impl Iterator<Item = (&K, EnvIndex)>;       // new
    // unchanged: alloc, get_or_allocate, context_env, read, write, free, environment
}
```

- `join` reads absent as bottom. Joining bottom into an absent anchor stores
  nothing and reports no change. Otherwise it calls `join_assign` in place
  (through `FactStore::get_mut`).
- `contexts()` lists each keyed context once. Iterating it, and each
  environment's own facts, is O(C + F) (decision 7).
- Concrete execution is untouched: `EnvStore<Infallible, SSAValue, V>`, using
  `write` only.

| Engine | Store | Anchors |
|---|---|---|
| Concrete | `EnvStore<Infallible, SSAValue, V>` | SSA values |
| Constprop | `EnvStore<Key, ForwardAnchor, V>` | `Value(SSAValue)`, `Return(i)` |
| Demand | `EnvStore<Key, SSAValue, Demand>` | SSA values |
| Liveness | `EnvStore<LinkTarget, ProgramPoint, LiveSet>` | program points |

## 4. Fact identity and work identity

A dependency names a fact by `(EnvIndex, anchor)`. It is keyed by `EnvIndex`,
not the context key, so there is no second context directory. Work identity
is an engine type and never derived from a fact:

| Engine | Work | Typical subscriptions |
|---|---|---|
| Constprop | block or graph walk in an environment | values read across blocks; callee `Return(i)` |
| Demand | propagate one value in a context | facts read by the rule; callee entry parameters |
| Liveness | block walk in an environment | successors' `BlockEntry` points |

Demand's work has a fact's coordinates (`Propagate { env, value }`) and is
still a different identity. The engine gives each changed fact its own
propagation work, which is a discovery rule, not a subscription.

## 5. Updates, reads and scheduling

- **Merge:** `EnvStore::join`, returning whether the fact rose.
- **Notify and schedule:** the engine and its existing driver. The engine
  records what the running work read, absent facts included, and which
  `join`s returned `true`. It then registers and schedules through the
  driver's dependency index and worklist. Forward already keeps a read log
  (`read_log` in `sparse_forward/interp.rs`). Whether the bookkeeping becomes
  a shared type is decided with the first client (B4).

Each migration keeps these rules:

- Reads during work subscribe, absent facts included. Result queries after
  the solve never subscribe.
- **Register reads before notifying changes.** A work item that reads a fact
  and then raises it, directly or through a callee it calls recursively, must
  rerun. Notifying first misses this the first time the item runs, because it
  is not yet subscribed. In the B2 prototype, swapping the two steps left a
  recursive call site's argument dead: the call site ran once instead of
  three times.
- Writes are visible to the running work at once; only notification waits
  until the work completes.
- An unchanged update schedules nothing.

## 6. Constprop returns: one anchor per return slot

A multi-value return is not one IR value. `Return` holds
`values: Vec<SSAValue>` (`kirin-function/src/ret.rs`) and `Call` holds
`results: Vec<ResultValue>` (`kirin-function/src/call/mod.rs`). So the return
is stored per slot, at an engine-private anchor:

```rust
enum ForwardAnchor { Value(SSAValue), Return(usize) }   // implements LatticeAnchor

// `ret %a, %b` in env: each slot joins like any fact
for (i, fact) in values.iter().enumerate() {
    store.join(env, ForwardAnchor::Return(i), fact)?;
}

// nested caller with n results: an absent slot reads as ⊥, like any fact
let response: Product<V> = (0..n)
    .map(|i| store.read(callee, ForwardAnchor::Return(i)).map(|f| f.unwrap_or_else(V::bottom)))
    .collect::<Result<_, _>>()?;
```

- **`Env::Anchor` stays `SSAValue`.** The engine maps `v` to `Value(v)`, so
  dialect rules and `SSABinding` are unchanged.
- **Bottom per slot is free.** A slot no `ret` has raised reads as ⊥, so
  "start at `[⊥; n]`" needs no `n` at context creation.
- **Return arity is private metadata.** Each context keeps a
  `return_arity: Option<usize>`, set by the first reached `ret`. It does
  three jobs:
  - a later `ret` with a different arity is an error;
  - the root reads `Return(0..arity)`;
  - `None` means no return was reached, so the root can still tell "never
    returned" from "returned nothing". Today's root collapses the two
    (`call_return(..).unwrap_or_default()`); exposing the difference is an
    A2/C decision.
- **Joins per slot:** `ret 1, 2` then `ret 1, 3` leaves `Return(0) = 1` and
  `Return(1) = ⊤`.
- **Reaching a block** with no parameters, or with bottom arguments, changes
  no fact. First reach is discovery, recorded in private bookkeeping, as
  constprop's `Owner` presence checks do today.

Not one fact per function: `HasProductValue` can pack a product into one
value, but only domains that opt in support it (`Demand` does not). It would
also conflate `ret %t`, one tuple-valued result, with `ret %a, %b`, two
results.

## 7. Demand: callers read the callee's entry parameters

A callee's response is the demand on its entry parameters, which are already
SSA anchors in its environment. A caller reads them directly and subscribes
to each:

```rust
// caller of f(x, y)
for (argument, param) in [x, y].into_iter().zip(entry_params(f)) {
    let demand = store.read(f_env, param)?.unwrap_or(Demand::Dead);
    raise(argument, demand);                     // the caller subscribes to (f_env, param)
}
```

- **No duplicate.** There is no parameter summary to keep in sync, and no
  write-back to forbid.
- **Subscriptions:** one per parameter, as today's per-parameter
  registrations (`summarize_call`, `sparse_backward/interp.rs:623`).
  Subscribing to a parameter that is still bottom is what makes recursion
  converge.
- **Environment at selection.** A caller can only subscribe to
  `(f_env, param)` if the environment exists. So selecting a context
  allocates its environment at once, while the discovery scan stays deferred
  and runs once, so rule dispatch never re-enters. This changes `AGENTS.md`'s
  "per-context state exists only once discovered".
- **Metadata:** parameters and return-site operands stay in private
  per-context bookkeeping keyed by `EnvIndex`. Return sites must agree on
  arity, and so must a requested output. Requested outputs route straight
  into return operands, as after #805.

## 8. Recommendation: dispatch demand rules on `SparseBackwardTransfer`

Move the `EnvStore`, its read bookkeeping and the context metadata into
`SparseBackwardTransfer`. Dispatch rules on it, as forward and liveness
already dispatch on their transfers, and have the driver's `run_statement`
delegate.

| Rule-facing operation | Needs |
|---|---|
| `fact` | store, read bookkeeping, current context |
| `raise_fact` | store, change bookkeeping |
| `declare_return` | the current context's metadata |
| call response (from the `Call` effect) | store, read bookkeeping |
| `block_params`, `terminator_args` | pipeline, already on the transfer |

`SparseBackwardInterp` is implemented on the driver today only because `fact`
reads the driver's summary map and `declare_return` writes its store slot.
Both move with the storage, so the driver keeps only scheduling. The bound
`InterpDispatch<SparseBackwardDriver<..>>` becomes
`InterpDispatch<SparseBackwardTransfer<..>>` in the engine and in
`kirin-demand`. Dialect rules are generic over `I: SparseBackwardInterp` and
do not change.

## 9. Budgets, guards and widening stay distinct

- **Context budgets** bound how many environments exist. They belong to
  context selection: `ConstPropContext::with_budget` and
  `OutputDemandContext` (default budget 64) today, problem D later.
- **Divergence guards** bound a nested local iteration and fail instead of
  approximating: `max_iterations` (`core/frame.rs:473`), used by abstract loop
  frames.
- **Widening** changes the merge at loop heads (`widen_after`,
  `entry_joins`). It is deferred. Constprop's `widen` already delegates to
  `join` (`kirin-constprop/src/value/lattice_impl.rs:80`), so B6 should keep
  its results; its guard tests must confirm that. Domains with infinite
  ascending chains need widening reintroduced later, as a merge choice beside
  `join`, never by bypassing change reporting.

## 10. Invariants

1. Every fact has one home: its environment's `FactStore`, at one anchor.
2. `write` assigns; analysis updates go through `join`.
3. Absent reads as bottom, including return slots.
4. Reads during work subscribe, absent facts included; reads outside work
   never do.
5. Reads register before changes notify. Running work can reschedule itself.
6. Unchanged updates schedule nothing and store no bottom facts.
7. Facts in different environments, or at different anchors, never alias.
   Work identity never implies a subscription.
8. Dependencies persist for the run. Abstract engines do not free
   environments mid-run; an engine that does must also retire the edges that
   name the freed `EnvIndex`.

## 11. Alternatives rejected

- **A typed per-environment summary slot** (`EnvStore<K, A, V, S = ()>`, with
  `summary`/`join_summary`): it was B2's proposal. It adds a second kind of
  fact with its own merge, its own "absent" rule and its own dependency kind,
  while anchors already cover returns with the existing rules.
- **A demand parameter summary synchronized from the entry parameters:** it
  duplicates facts that callers can read directly.
- **One fact per function via `HasProductValue`:** only some domains opt in,
  and it conflates a tuple-valued result with several results (§6).
- **An enum payload in one map** (`V | Product<V>`): every read needs a
  mismatch error.
- **Fake SSA values for returns:** they would name values that do not exist
  in the IR. `Return(i)` is a distinct anchor variant, not an SSA value.
- **A shared access-log type** (`FactLog`/`FactId`): the engines' existing
  read bookkeeping covers it. Reconsider at B4 if the three engines'
  bookkeeping turns out identical.
- **Notifying at update time** instead of when the work completes: it misses
  self-dependencies whose subscription is still being recorded (§5).

## 12. Open questions for the migrations

- **B6: constprop rereads.** A block that reads values it defined reruns once
  after they first rise. Exclude values defined by the running block, but not
  block parameters: a self-loop's back edge writes its own parameters.
- **B6: edge arguments** become `join` contributions to the successor's
  parameters, plus first-reach scheduling, replacing `OwnerEntry`.
- **B6: anchor enum cost.** Each read of an SSA fact hashes a
  `ForwardAnchor` instead of an `SSAValue`. Measure it.
- **A2/C:** whether the public root response exposes "never returned".

## 13. Tests for the migrations

B3 adds no tests; the first client (B4) tests the mechanism through a real
engine. These rules need coverage there:

1. **Commit order:** a work item that reads a fact and then raises it reruns;
   an unchanged rerun schedules nothing.
2. **Recording:** absent reads subscribe during work; result queries record
   nothing; unchanged joins schedule nothing.
3. **Constprop returns (B6):** per-slot joins over several `ret`s, arity
   errors, and `return_arity = None` for a callee that never returns.
4. **Demand responses:** a caller subscribed to a still-bottom parameter
   reruns when it rises, including through recursion.

## 14. Work and dependency keys: arena handles only (agreed 2026-11-04)

Every engine keys work and dependencies by `EnvIndex` plus an existing IR
handle. `Owner` and `Scoped` go away; the policy key `K` appears only where a
context is selected (`get_or_allocate(K)`).

| Engine | Work | Dependency (fact) key |
|---|---|---|
| Demand | `(EnvIndex, Statement)` | `(EnvIndex, SSAValue)` |
| Liveness | `(EnvIndex, Body)`, a block as `Body::Block` | `(EnvIndex, ProgramPoint)` |
| Constprop | `(EnvIndex, Body)`, a CFG block as `Body::Block`, a graph as `Body::DiGraph` | `(EnvIndex, SSAValue)`, `(EnvIndex, Return(i))` |

Work names what reruns; a fact key names what changed. Demand allocates a
callee's environment at context selection so callers can subscribe to its
parameters; the discovery scan stays deferred. Order: B4 demand, B5 liveness
(including its public `Scoped<LinkTarget, ProgramPoint>` signatures), B6
constprop (`Owner<K>`, `value_readers`), then delete `Owner` and `Scoped`.

## 15. B4: demand migrated

- **Storage:** `SparseBackwardTransfer` holds
  `env: EnvStore<P::Key, SSAValue, V>` and the per-context metadata
  `contexts: HashMap<EnvIndex, CallableContext>`. `DemandSummary`, the
  `DemandContexts` store slot and the placeholder `EnvIndex` are gone;
  `Interp::index()` is the dispatched context's environment.
- **Work and dependencies:** work is `(EnvIndex, Statement)`. When
  `EnvStore::join` reports a rise of `(env, value)`, the driver reruns the
  statements that translate it, from SSA topology, plus the callers
  registered on it. The one registered dependency is callee parameter →
  caller's call statement, in `DemandCallers = HashMap<(EnvIndex, SSAValue),
  Vec<(EnvIndex, Statement)>>`, the driver's store slot.
- **Driver loop:** unchanged. The profile uses `Summary = ()` and
  `Deps = ()`, both inert implementations added to `fixpoint/` until B7.
  `DemandCallers` sits in the store slot rather than `Deps` only because
  `SummaryDependencyIndex` keys trigger and dependent alike; B7 moves every
  engine's fact-keyed index into `Deps` once that trait is gone.
  Each scheduled work item still gets an empty summary entry, which the
  existing loop requires.
- **Reads:** no read log. Reads inside a context are covered by topology;
  the only cross-context read, the callee's parameters, is registered in
  `summarize_call` during the work, before its changes are applied.
- **Dispatch:** rules dispatch on `SparseBackwardTransfer`, which now
  implements `SparseBackwardInterp`. Selection, discovery, activation and
  parameter reads also live on the transfer; the driver keeps only
  `summarize_call` (registration) and `raise` (join + schedule). Selecting a
  context allocates its environment; discovery stays deferred and runs once.
- **Public API:** unchanged except removals: `DemandSummary` is gone, and
  `SparseBackwardProfile`/`DemandFrame` lose their context-key parameter.
  `kirin-demand`'s `analyze_demand` bound names `SparseBackwardTransfer`.
- **Scaling** (release, call chain, best of 3; probe in `/tmp`, not kept):

  | Functions | Analysis before → after | `DemandResult` before → after |
  |---:|---:|---:|
  | 500 | 1.13 → 0.68 ms | 0.42 → 0.09 ms |
  | 1,000 | 2.27 → 1.37 ms | 1.76 → 0.17 ms |
  | 2,000 | 4.93 → 2.68 ms | 6.75 → 0.33 ms |
  | 4,000 | 12.42 → 6.92 ms | 26.88 → 0.74 ms |

  Result construction now doubles with the chain instead of quadrupling.
- **Tests:** none added; the 1,666 workspace tests pass unchanged, including
  `kirin-demand`'s recursive and boundary suites.

## 16. B5: liveness migrated

- **Storage:** `DenseBackwardTransfer` holds
  `env: EnvStore<LinkTarget, ProgramPoint, V>` with one environment per
  `analyze` (`root: Option<(LinkTarget, EnvIndex)>`). The `BlockLiveness`
  summaries, the driver's point `FactStore` and `EnvIndex::placeholder` are
  gone; `Interp::index()` is the analyzed callable's environment.
- **Every recorded point joins** (decided 2026-11-04): statement points,
  nested structured-block boundaries and the walked block's own boundaries,
  so `BlockEntry(b)` always equals `Before(first statement of b)`. A
  recorded bottom point stays present (`record` writes it when the join
  stores nothing), so results still tell "nothing live" (`Some(∅)`) from
  "never reached" (`None`), as before.
- **Work and dependencies:** work is `(EnvIndex, Body::Block(block))`.
  `absorb_edges` registers the walking block on each successor's
  `(EnvIndex, BlockEntry)`; a rise of that entry reruns the readers. These
  live in `DenseReaders`, the driver's store slot, with `Deps = ()` (B7 moves
  them). Predecessors no longer rerun when only a successor's live-out rises.
- **Public API:** `point_facts(ProgramPoint)` and
  `facts() -> FactStore<ProgramPoint, V>` on the engine and
  `DenseLivenessResult`; `BlockLiveness` is no longer exported;
  `DenseBackwardFrameEngine::record_point` returns `Result`.
- **Dispatch, as in the sparse engines:** `DenseBackwardTransfer::run_statement`
  dispatches rules on the transfer. `DenseBackwardFrameEngine` now extends
  the shared `StatementDispatch` and `BlockQueries` instead of redeclaring
  `run_statement` and `block_params`; it keeps only the backward-walk
  queries (`last_statement`, `previous_statement`, `terminator_args`) and the
  dense point state. Dense frames carry their environment `index`:
  `DenseBlockFrame::cfg_owner/structured_body` and kirin-scf's
  `DenseScfIfFrame::new`/`DenseScfForFrame::new` take it.
- **Tests:** none added; 1,666 workspace tests pass after updating the call
  sites in `kirin-liveness/tests/cfg.rs` and toy-lang to bare points.

## 17. B6: constprop migrated

- **Storage:** `SparseForwardTransfer` holds
  `env: EnvStore<P::Key, ForwardAnchor, V>` with
  `ForwardAnchor { Value(SSAValue), Return(usize) }` (`Env::Anchor` stays
  `SSAValue`; the engine maps `v` to `Value(v)`), per-context metadata
  `contexts: HashMap<EnvIndex, ForwardContext>` (key, stage, entry unit,
  `return_arity`), and per-unit `entry_joins` visit counters, whose presence
  also marks a unit as reached. `Owner`, `ForwardSummary`,
  `FunctionSummary`, `BlockSummary`, `ForwardUpdate`, `apply_update`,
  `seed_entry_block` and `ForwardDeps` are gone; `Scoped` and
  `ScopedSparseStore` are deleted, since no engine uses them any more.
- **Updates:**
  - an abstract `env_write` joins (`EnvStore::join`) and logs what rose;
  - call and edge arguments merge into the entry unit's parameters through
    the analysis policy (`analysis_merge`: join, then widen by visits — widening
    is left as is, decided 2026-11-04) and are written; the unit is scheduled
    on first reach or when a parameter rose, and readers of every risen
    parameter are woken;
  - a reached `ret` joins into `Return(i)` per slot and wakes that slot's
    callers; `return_arity` is set by the first return and checked after.
- **Work and dependencies:** work is `(EnvIndex, Body)` (`Body::Block`,
  `Body::DiGraph`). `ForwardReaders = HashMap<(EnvIndex, ForwardAnchor),
  Vec<(EnvIndex, Body)>>` in the driver's store slot holds both dependency
  kinds: blocks that read a value they did not write (registered at walk
  completion, before risen values are woken) and callers of each return
  slot (registered in `summarize_call`). `Deps = ()`; B7 moves the map.
- **Widening timing:** a function's entry and its entry block used to keep
  separate visit counters over two stored copies of the arguments; with one
  copy (the entry block's parameters) there is one counter. This changes when
  widening would start only for a domain whose `widen` differs from `join`;
  none ships.
- **Public API:** `Owner` is replaced by `ForwardAnchor` in the exports;
  `return_summary` returns an owned `Option<Product<V>>`; `with_linker` and
  `with_policy` now reset the analysis state (as `with_analysis` already
  did). `analyze` still returns `Product<V>`, with "never returned" read as
  `[]`, symmetric with demand.
- **Tests:** none added; 1,666 workspace tests pass, including the guard
  tests (`constprop_loop_carried_cross_block_rise_is_top`,
  `constprop_direct_dominated_cross_block_use`,
  `constprop_context_budget_overflow_falls_back_to_top`,
  `abstract_digraph_owner_joins_two_call_sites`,
  `abstract_digraph_self_recursion_converges`), and factorial still gives
  `Const(120)`.

