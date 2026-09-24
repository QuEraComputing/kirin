# Fixpoint framework: separate work, state, and context

Status: **draft** (2026-09-24), branch `dl/fixpoint-owners`.

Builds on [`owner-identity-problem.md`](../../review/owner-identity-problem.md) and
revisits decisions from
[`2026-06-30-fixpoint-convergence`](../2026-06-30-fixpoint-convergence/index.md).
Prerequisite for interprocedural demand analysis (separate plan).

Goal: make the owner-summary fixpoint framework say what it does. Today one
key type (`Owner` / `SummaryKey`) means "runnable work", "stored state", and
"dependency source" at once; the per-engine hooks are named `*Semantics`,
colliding with semantic keys; and the three engines each use a different
subset of the framework. After this plan, the types separate **work**,
**state**, and **context**, and all three engines use the same solver API.

## 1. What the framework is for

A static analysis computes a fact for every value (e.g. "is `x` a constant?").
In straight-line code one pass in order suffices. Loops and recursion make
facts depend on themselves, so the analysis starts from ⊥ and reruns until
nothing changes — a fixpoint.

That is a **system of equations**:

- **Unknowns** (state identities): the facts being solved for — a block's
  entry facts, a value's demand, a function's return.
- **Equations** (work identities): "recompute these unknowns from those" —
  run one block, one value's defining statement, one graph body.
- **Solver rule:** when an unknown changes, rerun every equation that read it.
  Stop when the queue is empty.
- **Context** qualifies both: the same block under two calling contexts is two
  equations writing two sets of unknowns.

Equations may *pull* (read other unknowns, write their own — dense liveness) or
*push* (write other equations' inputs — sparse forward, demand). This is the
side-effecting constraint-system model used by Goblint (Apinis, Seidl,
Vojdani, APLAS 2012).

Everything else — frames, dialect rules, semantic keys — lives *inside* one
equation's run and is unaffected by this plan.

## 2. Vocabulary: current names → proposed names

| Concept | Current name(s) | Problem | Proposed |
|---|---|---|---|
| Dialect rule-set tag | `SemanticKey`, `Interp::Semantics`, `Sem` | fine on its own | **keep** |
| How to run one equation | `OwnerSemantics` (6 type params), implemented by stateless `SparseForwardSemantics` / `SparseBackwardSemantics` / `DenseBackwardSemantics` | name collides with semantic keys; repeats the profile's types | `EquationSystem::start` / `finish` |
| Type bundle for one analysis | `FixpointProfile` + driver params `Store`, `Deps` | one analysis spread over 3–4 places | one trait, `EquationSystem<I>` |
| Equation (work identity) | `Owner` (forward), `P::SummaryKey`, `WorkItem::Analyze` | `Owner::Function` is not runnable; `WorkItem` has one variant | `EquationSystem::Work` |
| Unknown (state identity) | `Owner` again, plus `ValueFactKey` outside the dependency index | two spellings, one outside the index | `EquationSystem::State` |
| Stored facts | `Summary` (+ `FixpointPhase`, `Strategy`, `Change`), `summaries` map in the driver | forward's `merge` is inert; no engine uses phase/strategy/change | analysis-owned `Store`; delete `Summary`; reserve "summary" for **function summaries** |
| "This unknown changed" | `SummaryEffect` (backward, dense), `apply_update` (forward), `Change` | three mechanisms | `solver.changed(&state)` after a store write |
| Context | `CallContext::Key` (forward), `BodyScope` + `Scoped` (backward), global `BackwardAnalysisState::scope` | three spellings; the global duplicates the key | a component of both `Work` and `State`: `Scoped<Context, Item>` |

## 3. Current state (evidence)

How each engine uses the framework today:

| Framework piece | Dense backward | Sparse backward | Sparse forward |
|---|---|---|---|
| Owner | block | SSA value | `Function` (never run), block, graph |
| `Summary::merge` | real join | real join | inert (always "no change") |
| `complete_owner` returns | `Update` | `Many` | always `None` |
| Entry | `solve_many` | own seeding pass + `merge_summary` + `drain_worklist` | `apply_update` + `drain_worklist` |
| Dependencies | `BackwardSummaryDeps` | `OwnerSummaryDeps` | `ForwardDeps` + a reader map outside the trait |
| Widening | — | — | `WideningStrategy` in the policy `P` |

Sources: [`fixpoint/`](../../../crates/kirin-interpreter/src/fixpoint/),
[`sparse_forward/interp.rs`](../../../crates/kirin-interpreter/src/engines/sparse_forward/interp.rs),
[`sparse_backward/interp.rs`](../../../crates/kirin-interpreter/src/engines/sparse_backward/interp.rs),
[`dense_backward/interp.rs`](../../../crates/kirin-interpreter/src/engines/dense_backward/interp.rs).

**Unused surface** (no use outside `fixpoint/` and its tests, checked
2026-09-24): `FixpointPhase`, `run_narrowing`, `set_phase`, `phase`,
`Summary::Strategy` and `Summary::Change` (every impl sets `()`),
`push_summary_effect`, the `frame_stack()` accessor, `clear_frame_stack`, `solve` (tests
only), `SimpleFixpointInterpreter` (re-export only). `WorkItem` and
`SummaryDependency` have one variant each. `ForwardSummaryDeps` and
`BackwardSummaryDeps` are the same code under two names.

**Earlier decisions this plan revisits** (from the 2026-06-30 plan):

- *Forward merges through its own `apply_update`; `ForwardSummary::merge`
  stays inert.* Pragmatic during migration, but it left the framework's
  "merge → dependencies → schedule" path unused by the one engine that
  handles calls.
- *`Function` owners are storage-only.* Correct observation, but encoded by
  rejecting them at runtime ("function owners are storage-only and never
  executed") instead of in the types.
- *"Interval/constprop use the phases."* They do not: widening lives in
  `WideningStrategy`, and nothing narrows.

**Smaller issues found:**

- Sparse backward reads the scope from a global in `fact()` but from the
  owner key in `complete_owner`. They agree only because each `analyze`
  covers one body.
- `schedule` does not skip owners that are already queued.
- Dense `analyze` rebuilds the driver on every call; sparse backward keeps
  results across calls.

## 4. Target design

### 4.1 Solver API

One trait per analysis, and a solver that owns only the queue, the
dependency index, and the current work item:

```rust
/// One analysis's equation system. Replaces `FixpointProfile`,
/// `OwnerSemantics`, and the driver's `Store`/`Deps` parameters.
pub trait EquationSystem<I: Interp>: Sized {
    /// Equation identity: a unit of work the solver can run.
    type Work: Clone + Eq + Hash;
    /// Unknown identity: stored information whose change reruns its readers.
    type State: Clone + Eq + Hash;
    /// Analysis-owned fact storage (`FactStore`, function-summary maps, …).
    type Store;
    type Frame;
    type Completion;

    /// Build the root frame that runs `work`.
    fn start(
        solver: &mut StandardFixpointInterpreter<I, Self>,
        work: &Self::Work,
    ) -> Result<Self::Frame, I::Error>;

    /// Write `work`'s results into the store and call `solver.changed(..)`
    /// for every state that rose.
    fn finish(
        solver: &mut StandardFixpointInterpreter<I, Self>,
        work: Self::Work,
        done: Self::Completion,
    ) -> Result<(), I::Error>;
}

impl<I: Interp, P: EquationSystem<I>> StandardFixpointInterpreter<I, P> {
    /// Queue `work` once; a request for already-queued work is dropped.
    pub fn schedule(&mut self, work: P::Work);
    /// The running equation read `state`: rerun it when `state` changes.
    pub fn read(&mut self, state: P::State);
    /// `state` rose: schedule every equation that read it.
    pub fn changed(&mut self, state: &P::State);
    /// The equation being run, if any.
    pub fn current(&self) -> Option<&P::Work>;
    /// Run queued equations until the queue is empty.
    pub fn solve(&mut self) -> Result<(), I::Error>;
}
```

There is no `bottom_summary`: an absent fact is bottom in the store. Merging
is the store's job (`FactStore::join_with` already joins and reports
`Change`); forward keeps its policy-driven merges (`WideningStrategy`,
visit counts) in its store-writing helpers, which now end in `changed(..)`
instead of scheduling by hand.

A standalone model of this shape was compiled and tested on 2026-09-24
(scratch, not committed): a forward-shaped client with `Work ≠ State` and a
generic context, and a backward-shaped client with `Work = State` taking its
scope from the work key. Both converged; a duplicate `schedule` ran once.
The model did not include `Interp` delegation or the real frame protocol,
which this plan leaves unchanged.

### 4.2 Per-engine mapping

| | Sparse forward | Sparse backward demand | Dense backward liveness |
|---|---|---|---|
| `Work` | `ForwardWork<K>`: `Block { ctx, block }`, `Graph { ctx, graph }` | `Scoped<BodyScope, SSAValue>` | `Scoped<BodyScope, Block>` |
| `State` | `ForwardState<K>`: `FunctionReturn(K)`, `BlockEntry(ForwardWork<K>)`, `Value(Scoped<K, SSAValue>)`; `FunctionEntry(K)` if not folded into the entry block's `BlockEntry` (decide in step 4) | same as `Work` | same as `Work` |
| `Store` | function summaries by `K`, block summaries by work, `EnvStore` for values | `FactStore<Scoped<..>, V>` | `FactStore` for live-in/live-out and point facts |
| Reads | own entry, callee returns, values read from other blocks | own demand | successors' live-in |
| Writes, then `changed` | successors' entries, callee entry, own return, own values | operands' demand | own live-in/live-out |

`Owner::Function` disappears: a function record is **state**, never work, so
the type cannot express "schedule a function".

## 5. Steps

Each step is one commit and leaves `cargo nextest run --workspace` green.

1. **Solver hygiene.** De-duplicate the queue. Delete the unused surface in §3
   (pending decision D1 for phases). Replace single-variant `WorkItem` and
   `SummaryDependency` with plain keys. Merge `ForwardSummaryDeps` and
   `BackwardSummaryDeps` into one explicit-edge index. *Risk:* de-duplication
   changes visit counts, and forward widens after `widen_after` visits — run
   the constprop and interval tests.
2. **Sparse backward: scope from the work key.** `fact()` reads
   `current().scope`; the seeding pass in `analyze` sets the context
   explicitly; delete `BackwardAnalysisState::scope`. Add a unit test that
   solves two bodies in one run. *Unblocks interprocedural demand.*
3. **State → work dependency index.** Add `read` / `changed` backed by a
   `State → {Work}` map. Backward and dense use `State = Work`: dense's
   `absorb_edges` calls `read(successor)`; demand's self-dependency becomes
   "each run reads its own value". `SummaryEffect` is no longer needed by
   either.
4. **Forward: split work from state.** Introduce `ForwardWork<K>` and
   `ForwardState<K>`. Move function records into the store. `apply_update`
   keeps its `P`-driven merges but ends in `changed(..)`; value-reader edges
   become `State::Value` dependencies. Delete `summaries_mut` / `summary_mut`,
   the `as_block` / `as_function` downcasts, and the three "storage-only"
   runtime errors.
5. **Fold the traits.** Replace `FixpointProfile` + `OwnerSemantics` + the
   `Store`/`Deps` parameters with `EquationSystem<I>`; the driver becomes
   `StandardFixpointInterpreter<I, P>`. Rename the implementors
   `SparseForwardEquations` / `SparseBackwardEquations` /
   `DenseBackwardEquations` (the old `*Profile` markers and `*Semantics` unit
   structs merge). Delete `Summary` and `SummaryEffect`. Resolves the TODO in
   `SparseForwardInterpreter::analyze`.
6. **Context naming.** Spell every key as `Scoped<Context, Item>`; treat
   `BodyScope` as the unrefined context and `CallContext`'s key as a refined
   one. Consider renaming `CallContext::Key` to `Context`.
7. **Docs.** Update [`docs/design/interpreter/index.md`](../../design/interpreter/index.md)
   (Engines, Abstract frames, Abstract policies, Status), the Interpreter
   conventions in `CLAUDE.md` (mentions of `Owner`, `OwnerSemantics`,
   `StandardFixpointInterpreter`), and the `fixpoint/` module docs.

Order rationale: deletions first shrink what later steps touch; step 2 is
small and unblocks demand; steps 3–4 are the real type fix; renaming waits
until step 5 because renaming `OwnerSemantics` earlier would be wasted when
it is folded away.

## 6. Decisions needed

- **D1. Narrowing.** Delete `FixpointPhase` / `run_narrowing` now (no client;
  `fixpoint/tests/phase.rs` is the only user), or keep them for interval?
  *Recommendation:* delete; re-add with interval as the first client and a
  test showing the precision gain.
- **D2. Who owns fact storage.** *Recommendation:* the analysis (`Store`); the
  solver owns only queue, dependencies, and current work. Alternative: a
  generic fact table inside the solver, which forward could not use without
  moving its policy into it.
- **D3. Push inputs through dependencies or direct scheduling.** A block's
  entry can be a `State` the block reads (uniform) or a special case that
  schedules directly (today's behaviour). *Recommendation:* uniform; seeding
  still calls `schedule` explicitly because a never-run block has registered
  no reads.
- **D4. Rename `StandardFixpointInterpreter`?** Optional and high-churn;
  defer unless step 5 makes the old name misleading.
- **D5. Demand context** (body only vs body + call site). Out of scope here,
  but steps 2–3 must not preclude either choice.

## 7. Non-goals

- No change to `SemanticKey`, `Interpretable`, `InterpDispatch`, or dialect
  rules.
- No change to the frame protocol (`Frame`, `drive_frames`) or to frames'
  traversal logic.
- No change to what `CallContext` / `WideningStrategy` compute.
- Interprocedural demand itself.

## 8. Tests

After every step: `cargo nextest run --workspace` and
`cargo test --doc --workspace`. Guards that must stay green:

- Forward convergence: `constprop_loop_carried_cross_block_rise_is_top`,
  `constprop_direct_dominated_cross_block_use` (toy-lang);
  `abstract_digraph_owner_joins_two_call_sites`,
  `abstract_digraph_self_recursion_converges` (`tests/body_kinds.rs`).
- Context sensitivity: `constprop_context_budget_overflow_falls_back_to_top`;
  `cargo run -p toy-lang -- run example/toy-lang/programs/factorial.kirin --stage source --function factorial --constprop 5`
  prints `Const(120)`.
- Backward: `scf_for_loop_carried_demand_converges`,
  `dense_loop_carried_fixpoint`, and the `kirin-liveness` suite.

New tests:

- Step 1: an equation scheduled twice while queued runs once.
- Step 2: two bodies solved in one run keep their facts separate.
- Step 3: dense reruns a predecessor when a successor's live-in rises, via
  `read` rather than an explicit edge.

## 9. References

- [`docs/review/owner-identity-problem.md`](../../review/owner-identity-problem.md)
- [`docs/plans/2026-06-30-fixpoint-convergence/index.md`](../2026-06-30-fixpoint-convergence/index.md)
- K. Apinis, H. Seidl, V. Vojdani, "Side-Effecting Constraint Systems: A Swiss
  Army Knife for Program Analysis", APLAS 2012.
