# Fixpoint fact store: typed per-context storage and one update path (B2)

Status: **design spike, for review before B3.** The only code change is the
decided default [`Lattice::join_assign`](../../../crates/kirin-ir/src/lattice.rs).
No engine was migrated. The design was checked with a throwaway test-only
prototype of the API below; it is not in the tree, and B3 adds tests against
the real API (§14).

## 1. Problem

The three abstract engines store and update facts three ways:

- **Constprop** (sparse forward) keeps SSA facts in `EnvStore` and a second
  copy of each block's outputs in `BlockSummary.outputs`, and merges by hand
  in `apply_update`.
- **Demand** (sparse backward) keeps one flat driver table keyed
  `(context, SSAValue)`. Building `DemandResult` scans it once per context,
  O(C × F).
- **Liveness** (dense backward) keeps `BlockLiveness` summaries and a
  separate point-fact store.

Target: `EnvStore` owns per-context facts and their lifetime, and every
analysis update goes through one merge → notify → schedule mechanism.

## 2. Decisions this note builds on

1. Reuse `kirin_ir::Lattice`, with a default
   `join_assign(&mut self, &Self) -> bool` that domains may override. Keep the
   value-returning `join`.
2. Widening is deferred. The mechanism uses ordinary joins (§9).
3. Storage is typed within each context, not a map with enum payloads:
   - sparse forward: SSA facts plus a stored return approximation;
   - sparse backward: SSA facts plus a stored parameter-demand approximation;
   - dense backward: program-point facts, block boundaries included.
4. One authoritative representation per query role. Local queries read
   facts. Callable responses and caller subscriptions use the stored
   approximation.
5. Demand's approximation deliberately duplicates its entry-parameter facts.
   A changed parameter fact synchronizes its component before any response
   can be read. Changed approximations notify callers; changed local facts
   notify their own dependents. Nothing writes back from the approximation.
6. `EnvStore::write` stays assignment.
7. Fact identity is separate from work identity, and names both the
   environment and the fact kind.
8. Demand results visit each context and its own facts once: O(C + F).
9. Return-site routing and discovery metadata stay private and are not facts.

## 3. Storage: `EnvStore<K, A, V, S = ()>`

The smallest change that keeps decisions 3 and 6. Each environment record
gains one typed slot next to its facts:

```rust
struct Environment<K, A, V, S> {
    key: Option<K>,
    facts: FactStore<A, V>,
    summary: Option<S>,          // new
}

impl EnvStore<K, A, V, S = ()> {
    pub fn summary(&self, index: EnvIndex) -> Result<Option<&S>, InterpreterError>;
    pub fn summary_mut(&mut self, index: EnvIndex) -> Result<&mut Option<S>, InterpreterError>;
    pub fn contexts(&self) -> impl Iterator<Item = (&K, EnvIndex)>;   // keyed contexts, each once
}
// plus FactStore::get_mut, so join_assign can run in place
```

| Engine | Instantiation |
|---|---|
| Concrete | `EnvStore<Infallible, SSAValue, V>`: spelling unchanged, `S = ()` |
| Constprop | `EnvStore<Key, SSAValue, V, Product<V>>`: return approximation |
| Demand | `EnvStore<Key, SSAValue, Demand, Product<Demand>>`: parameter demand |
| Liveness | `EnvStore<LinkTarget, ProgramPoint, LiveSet>`: `S = ()` |

- The approximation is freed with its environment, and it errors on a dead
  `EnvIndex` like every other access. It lives with the store, not in an
  engine-side map.
- The store still knows nothing about bottom, joins or dependencies.
  `summary` reports absence; the engine decides what absence means (§6, §7).
- Concrete execution never names `S`: its spelling is unchanged.
- `contexts()` exists because results need each context's key. Iterating the
  directory and each environment's own `FactStore` is O(C + F) (decision 8).

## 4. Fact identity and work identity

```rust
enum FactId<A> {
    Local { env: EnvIndex, anchor: A },   // an SSA value or a program point
    Summary { env: EnvIndex },            // the environment's approximation
}
```

- Keyed by `EnvIndex`, not by context key, so there is no second context
  directory.
- Callers subscribe to the whole approximation, not to its components.
- Work identity is an engine type and never derived from a fact:

| Engine | Work | Typical subscriptions |
|---|---|---|
| Constprop | block or graph walk in an environment | facts read across blocks; callee `Summary` |
| Demand | propagate one value in a context | facts read by the rule; callee `Summary` |
| Liveness | block walk in an environment | successors' `BlockEntry` points |

Demand's work has a fact's coordinates (`Propagate { env, value }`) and is
still a different identity. The engine gives each changed local fact its own
propagation work (§7). That is a discovery rule, not a subscription.

## 5. The update mechanism

The helper is split along the borrow boundary every engine already has. Rules
hold only the transfer, reading through `&self` and writing through
`&mut self`. Only the driver holds the dependency index and the worklist.

**`FactLog<A>`, on the transfer next to the store.** Every analysis read and
update goes through it:

```rust
fn read_fact(&self, store, env, anchor) -> Result<V, _>;              // absent = bottom; records the read
fn read_summary(&self, store, env) -> Result<Option<S>, _>;           // records the read
fn join_fact(&mut self, store, env, anchor, &V) -> Result<bool, _>;   // join_assign; records a change
fn join_summary(&mut self, store, env, &Product<V>) -> Result<bool, _>;            // lifted, arity-checked
fn join_summary_at(&mut self, store, env, position, &V) -> Result<bool, _>;        // one component
```

- `join_fact` reads absent as bottom. Joining bottom into an absent fact
  stores nothing and reports no change.
- The `FactId` is built from where the update lands, so a caller cannot
  notify the wrong location.
- Reads record only while a work item is running.

**`FactSolver<A, W>`, on the driver.**

```rust
fn schedule(&mut self, work: W);                                          // coalesces queued work
fn begin(&mut self, log: &FactLog<A>) -> Result<Option<W>, _>;            // pop; start recording
fn commit(&mut self, log: &mut FactLog<A>) -> Vec<FactId<A>>;             // see below
```

`commit` first subscribes the running work to every fact it read, present
or absent. It then schedules the dependents of every fact that changed. It
returns the changes so an engine can derive its own work from them.

**Ordering rule: register reads before notifying changes.** A work item that
reads a fact and then raises it, directly or through an approximation, must
rerun. Notifying first misses this the first time the item runs, because it
is not yet subscribed. In the prototype, the recursive example of §7 with
the two steps swapped left `b` dead: the call site ran once instead of
three times.

Further properties:

- Writes are visible to the running work at once. Only notification waits
  for `commit`.
- Outside work (seeding, discovery with no owner) nothing is recorded as
  read, and `commit` notifies at once.
- `begin` refuses to start while a log is uncommitted, so changes cannot be
  dropped.

**Ownership in production (B3):**

- `FactLog` and `FactId` belong in `core/env`, beside the store.
- The solver half becomes the existing driver's dependency index (keyed by
  `FactId`) and its worklist, gaining coalescing and a `commit` method.
  `drive_frames` and the frame protocol are unchanged. This is not a second
  driver.

## 6. Forward returns

The approximation is `Option<Product<V>>` with lifted semantics:

| State | Stored | Root reads | Nested caller binds |
|---|---|---|---|
| No return reached | `None` | `None` | bottom in each of its result slots, subscribed |
| Zero-result return reached | `Some([])` | `Some([])` | `[]` |
| Returns reached | `Some(join of each site)` | the product | the product, subscribed |

- **First contribution:** absent is below every product, so the first
  `join_summary` reports a change even for `[]`. "Returned nothing" wakes
  callers that are waiting on "never returned".
- **Multiple return sites:** results join per position, e.g.
  `[1, 2] ⊔ [1, 3] = [1, ⊤]`.
- **Arity:** a contribution whose arity differs from the stored product
  fails with `ProductArityMismatch`. It records no change and leaves the
  approximation untouched. A caller whose result count differs fails the
  same way. Today's root (`call_return(..).unwrap_or_default()`,
  `sparse_forward/interp.rs:1583`) collapses `None` into `[]`. Exposing the
  `Option` is the A2/C decision.
- **Reaching a block** with no parameters, or with bottom arguments, changes
  no fact. The first reach is discovery and is recorded in private
  bookkeeping, as constprop's `Owner` presence checks do today.
- **No fake SSA values** anywhere: the return has its own typed slot.

## 7. Demand's parameter-demand approximation

- **Initialization: at context selection, not at discovery.** Selecting a
  context allocates its environment and stores a bottom product of the
  parameter arity, read from the IR. A caller can then read the response and
  subscribe before the discovery scan runs. The scan still runs once,
  deferred, so rule dispatch never re-enters. This changes `AGENTS.md`'s
  "per-context state exists only once discovered": the environment and its
  bottom approximation exist from selection.
- **Synchronization: one mutation path.** Every demand fact change, from a
  rule's `raise_fact` or from routing requested outputs into return
  operands, goes through one private `raise`:
  1. `join_fact`;
  2. if the value is entry parameter `i`, `join_summary_at(i, new fact)`.

  Both happen before control returns, so no response can be read out of
  sync. The approximation has no write-back path. In the prototype,
  removing the synchronization step broke three of the five demand
  scenarios.
- **Notification:**
  - a changed parameter fact notifies its local dependents and gets its own
    propagation work;
  - the approximation change it causes notifies subscribed callers;
  - a changed non-parameter fact notifies no caller.
- **Recursion:** a recursive call site subscribes to its own context's
  approximation, even while it is bottom. Through the ordering rule in §5, it
  reruns whenever its own raises change that approximation.
- **Metadata:** parameters and return-site operands live in private
  `HashMap<EnvIndex, ContextMeta>` bookkeeping, keyed by environment.
  - Return sites must agree on arity, and a requested output's arity must
    match.
  - Requested outputs route straight into return operands, as after #805.
- **Caller subscription:** one `Summary` subscription per caller replaces
  today's per-parameter registrations (`summarize_call`,
  `sparse_backward/interp.rs:623`).

## 8. Recommendation: dispatch demand rules on `SparseBackwardTransfer`

Move the `EnvStore`, the `FactLog` and the context metadata into
`SparseBackwardTransfer`, dispatch rules on it as forward and dense already
dispatch on their transfers, and have the driver's `run_statement` delegate.

Evidence from the prototype: every rule-facing operation of
`SparseBackwardInterp` was local to the transfer.

| Operation | Needs |
|---|---|
| `fact` | store, log, current context |
| `raise_fact` | store, log, metadata (synchronization) |
| `declare_return` | metadata of the current context |
| call response (from the `Call` effect) | store, log |
| `block_params`, `terminator_args` | pipeline (already on the transfer) |

The prototype's rules took only the transfer and never saw the solver. The
driver began work, ran the rule, committed, and scheduled propagation for
changed facts. Two benefits:

- One dispatch shape for all three engines.
- The driver keeps only scheduling. Today `SparseBackwardInterp` is
  implemented on the driver only because `fact` reads its summary map and
  `declare_return` writes its store slot. Both move with the storage.

Consequence for B4: `S: InterpDispatch<SparseBackwardDriver<..>>` becomes
`InterpDispatch<SparseBackwardTransfer<..>>` in the engine and in
`kirin-demand`'s bound. Dialect rules are generic over
`I: SparseBackwardInterp` and do not change.

## 9. Budgets, guards and widening stay distinct

- **Context budgets** bound how many environments exist. They belong to
  context selection: `ConstPropContext::with_budget` and
  `OutputDemandContext` (default budget 64) today, problem D later. B does not
  touch them.
- **Divergence guards** bound a nested local iteration and fail instead of
  approximating: `max_iterations` (`core/frame.rs:473`), used by abstract
  loop frames. They stay in frames.
- **Widening** changes the merge operator at loop heads (`widen_after`,
  `entry_joins`). It is deferred; the mechanism only joins.
  - Constprop's `widen` already delegates to `join`
    (`kirin-constprop/src/value/lattice_impl.rs:80`), so dropping the
    widening counters in B6 should keep its results. B6's guard tests must
    confirm that.
  - Domains with infinite ascending chains need widening reintroduced later,
    as a merge choice inside `FactLog` (for example a widening variant of
    `join_fact` for one fact kind), never by bypassing the log.

## 10. Invariants

1. One authoritative representation per query role. The demand duplicate is
   derived, synchronized on every change, and never written back.
2. `EnvStore::write` assigns; analysis updates go through `FactLog`.
3. Absent local facts read as bottom; an absent approximation is the lifted
   bottom (forward) or impossible after selection (demand).
4. Every read during work subscribes, absent facts included; reads outside
   work never do.
5. `commit` registers reads, then notifies changes. Queued work coalesces;
   running work can reschedule itself.
6. Unchanged updates schedule nothing and materialize no bottom facts.
7. Facts in different environments, or of different kinds, never alias.
   Work identity never implies a subscription.
8. Dependencies persist for the run. Freeing an environment retires its
   facts and approximation. Abstract engines do not free mid-run; an engine
   that does must also retire edges that name the freed `EnvIndex` (B3).

## 11. Alternatives rejected

- **One map with enum anchors and payloads** (`Value | Return` holding
  `V | Product<V>`): every read needs a mismatch error, every entry is the
  size of the largest variant, and a zero-result return needs a marker. The
  layout benchmark (`wip/fact-store-layout-bench`) favored typed storage in
  most operations. The decisive gain there was per-context partitioning,
  which both layouts allow.
- **Fake SSA anchors for returns or parameter demand:** they would name
  values that do not exist in the IR.
- **An engine-side `HashMap<EnvIndex, S>`:** a second place whose lifetime
  `EnvStore::free` does not control.
- **A generic per-environment record (`EnvStore<K, R>`):** every access would
  go through a trait, and the concrete engine's spelling would change.
- **Deriving the forward return from return-site operands** (the MLIR
  approach): it needs reachability facts and return-site discovery in
  forward. That is more than B needs, and the stored approximation is what
  decision 3 chose.
- **A join method on `EnvStore`:** the store would have to know bottom. That
  knowledge stays in `FactLog`.
- **Notifying at update time instead of at commit:** it misses
  self-dependencies whose subscription is still being recorded (§5).
- **Per-parameter caller subscriptions:** the stored approximation is
  exactly what decision 4 makes the single subscription target.

## 12. What the spike did not check

- In-place joins: the prototype cloned the stored fact because `FactStore`
  has no `get_mut` yet.
- Scheduling order: the prototype kept subscriptions in insertion order,
  which made scheduling deterministic. B3 should keep a deterministic order.
- Real IR and dispatch: contexts were string keys and anchors id-only
  handles. The real-engine proof is B4.
- Error recovery: a rule that errors leaves its log uncommitted. Engines are
  dropped on error (`analyze(self)`), so B3 needs no recovery path.

## 13. Open questions for the migrations

- **B4: demand read logging.** Uniform logging subscribes each propagation
  work to the facts its rule reads. This duplicates the intrinsic
  propagation that already covers them, and costs one index entry per read.
  The parameter-position lookup in `raise` adds O(parameters) per changed
  fact. Neither cost was measured; the layout benchmark did not cover
  synchronization. Measure on the 2,000-function chain from the #805 review.
- **B6: constprop rereads.** A block that reads values it defined reruns
  once after they first rise. Today's forward engine excludes values it
  wrote itself. B6 can exclude values defined by the running block, but not
  block parameters: a self-loop's back edge writes its own parameters.
- **B6: edge arguments.** Edge arguments become `join_fact` contributions to
  the successor's parameters, plus first-reach scheduling, replacing
  `OwnerEntry`.
- **A2/C:** the public root response becomes `Option<Product<V>>`.

## 14. Can B3 proceed?

Yes, once this note is reviewed. B3 lands:

- `EnvStore<K, A, V, S = ()>` with `summary`, `summary_mut` and `contexts`;
- `FactStore::get_mut`;
- `FactId` and `FactLog` in `core/env`;
- the fact-keyed dependency index with coalescing and `commit` on the
  existing driver.

It adds focused tests against the real API (§15). No engine uses any of it
until B4.

## 15. Tests for B3

The prototype covered every requirement in the B2 brief. Against the real
API, these few are the ones worth keeping, because each guards a rule that
is easy to get wrong:

1. **Commit order.** A recursive call site that raises its own context's
   approximation reruns until it converges. Swapping `commit`'s two steps
   made this fail in the prototype.
2. **Demand synchronization.** After every change, each approximation
   component equals its parameter's fact; a non-parameter change wakes no
   caller. Removing the synchronization step made this fail.
3. **Forward return states.** Absent versus `[]`, per-position joins of
   several return sites, and arity errors that leave the stored product
   unchanged.
4. **Absent reads subscribe, inspection does not,** and an unchanged update
   schedules nothing.

The `join_assign` default has no dedicated test; B3's tests exercise it
through every update.
