# Fixpoint framework convergence — restore the mature owner-summary engine on `dl/liveness` without duplicating `Interp`

Status: **locked plan** (rev 2026-07-01). Supersedes the 2026-06-30 "rebase onto `rust`" framing (see §0).

Goal: bring the mature owner-summary fixpoint framework from the `rust` branch onto
`dl/liveness`, **adapted** to the current interpreter model. `Interp` stays the single
source of `Value` / `Error` / `Effect` / `Kind`. The fixpoint layer only describes
owner-summary convergence (owner key, summary, frame, completion) and wraps an
`Interp` by delegation.

## 0. What changed since the 2026-06-30 revision

The first revision of this doc recommended converging by **rebasing the `dl/liveness`
dataflow additions onto `rust`**, and gating the `SparseForwardInterpreter` migration to
a later, separate milestone ("do not start until the backbone + liveness client are
green"). Two decisions are now **reversed**:

1. **Vehicle: port onto `dl/liveness`, do _not_ rebase onto `rust`.** The `rust` branch
   carries a large, unrelated reorganization (renames, `Location`/`Position`, `standard/`
   frames, `Env<V>`, profile bundling). Rebasing would drag all of it in. Instead we port
   only the fixpoint subsystem and adapt it to the interpreter model that already exists on
   `dl/liveness`.
2. **`SparseForwardInterpreter` migration is in scope for this plan** (still the last
   milestone, M3 — but part of this effort, not deferred indefinitely).

A third, new architectural decision drives the port: the **wrapper-over-`I`** layering
(§1). `rust` bundles `Value`/`Error`/`Stage`/`Frame`/`Completion` into a `FixpointProfile:
InterpreterProfile` "type family." That collides with our model where `Interp` already owns
`Value`/`Error`/`Effect`/`Kind`. We keep `Interp` as the single source and shrink the
profile to owner-summary types only.

What still holds from the first revision (the durable analysis):

- The two `solve` loops live at **different levels** and nest — `dl/liveness::solve` is the
  intra-owner transfer loop; the mature `solve` is the inter-owner convergence loop with
  intra-owner traversal delegated to a frame stack. They are not competitors.
- The `dl/liveness` dataflow vocabulary (`LatticeAnchor`, sparse/dense stores, structural
  extraction) survives as the **shape of a `Summary`/`Store`**, hosted inside owner-semantics
  clients, not as a standalone kernel (§5).
- The reason to adopt the mature backbone rather than extend `FixpointInterp` is the
  `FrameEffect` (traversal) vs `SummaryEffect` (convergence) split, which removes the
  `transfer_work_item -> Interp::Effect` coupling that blocked forward unification.

## 1. Final layering (wrapper-over-`I`)

```
I: Interp
  owns Value / Error / Effect / Kind

StandardFixpointInterpreter<I, P, Store, Deps>
  owns summaries, owner worklist, dependency graph, phase/strategy
  implements Interp by delegating to I

P: FixpointProfile<I>
  owns SummaryKey / Summary / Frame / Completion only
```

The profile does **not** re-declare `Interp`'s associated types:

```rust
pub trait FixpointProfile<I: Interp> {
    type SummaryKey: Clone + Eq + Hash;
    type Summary: Summary;
    type Frame;
    type Completion;
}
```

The driver wraps one `Interp` and adds the owner-summary machinery:

```rust
pub struct StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    inner: I,
    summaries: HashMap<P::SummaryKey, P::Summary>,
    deps: Deps,
    worklist: VecDeque<WorkItem<P::SummaryKey>>,
    frame_stack: Vec<P::Frame>,
    current_owner: Option<P::SummaryKey>,
    pending_effects: Vec<SummaryEffect<P::SummaryKey, P::Summary>>,
    phase: FixpointPhase,
    strategy: <P::Summary as Summary>::Strategy,
    store: Store,
}
```

The driver **is** an `Interp`, by delegation to `inner`:

```rust
type Value = I::Value;
type Error = I::Error;
type Effect = I::Effect;
type Kind = I::Kind;
// stage()/statement()/index() delegate to inner
```

Additional delegation impls, added only where a client needs them:

- `Env for StandardFixpointInterpreter<I, ...>` when `I: Env` (so forward clients keep the
  blanket `SparseForwardInterp` impl and its `read`/`write` helpers "for free").
- `ForwardFrameDriver` when `I: ForwardFrameDriver`.
- analysis-specific frame-driver traits only where needed.

`OwnerSemantics` is ported as-is (it already threads `interp: &mut I` through its methods);
its error parameter collapses to `I::Error` at the driver, and its `K`/`S`/`F`/`C`
parameters bind to `P::SummaryKey`/`P::Summary`/`P::Frame`/`P::Completion`.

**`SummaryKey` is not a `LatticeAnchor`.** `SummaryKey` identifies the owner / dataflow
equation (a function+context, a block). `LatticeAnchor` identifies fact *locations* inside
summaries and stores (an `SSAValue`, a `DenseAnchor`). Do not require `SummaryKey:
LatticeAnchor`.

## 2. API changes — `kirin-interpreter::fixpoint`

Add a `fixpoint` module (already stubbed at `crates/kirin-interpreter/src/fixpoint/`) with
the mature framework, adapted to the wrapper design:

- `FixpointPhase` (`Join` / `Widen` / `Narrow`), `Summary` (`merge(phase, candidate,
  strategy) -> Option<Change>`), `OwnerSemantics` (`bottom_summary` / `entry_frame` /
  `complete_owner`), `SummaryEffect` (`None` / `Update{owner,candidate}` / `Many`),
  `WorkItem`.
- `SummaryDependencyIndex`, `SummaryDependency`, and the concrete indices
  `OwnerSummaryDeps` / `ForwardSummaryDeps` / `BackwardSummaryDeps`.
- `StandardFixpointInterpreter` (above) and `SimpleFixpointInterpreter`.
- `FixpointProfile<I>` (owner-summary types only; §1).

## 3. Implementation changes

Port the mature fixpoint files from `rust`
(`abstract_interp/fixpoint/{traits,deps,driver,solver,runner,delegates}.rs` + `summary.rs`),
adapting each to the wrapper-over-`I` design:

- Driver schedules `WorkItem::Analyze(owner)`.
- `OwnerSemantics::entry_frame` builds a frame for one owner.
- `run_frame` drives `FrameEffect` (Continue/Push/Done/Complete), returning a `Completion`.
- `OwnerSemantics::complete_owner` returns a `SummaryEffect`.
- `Summary::merge` performs join / widen / narrow per `FixpointPhase`.
- `SummaryDependencyIndex` maps "A changed → reanalyze B".

Remove the public tiny analysis solver after migration:

- delete or de-export `FixpointInterp` and `solve<I: FixpointInterp>` (currently in
  `solver.rs`, exported from `lib.rs`).
- **keep `DenseCursor`/`CursorPosition`**, moved out of `solver.rs` (they are a
  reconstruction helper, not a solver).

### 3.1 Migrate `kirin-liveness`

- Use the shared fixpoint driver, not its own `solve`.
- Dense liveness uses **block owners**:
  - `SummaryKey = Block`
  - `Summary = live-in / live-out sets`
  - `Dependency = successor block changed → predecessor block` (`BackwardSummaryDeps`).
- Derive the sparse demanded set from the final liveness summaries.
- Preserve `LivenessResult::{demanded, live_in, live_out, live_before, live_after}`.
- Preserve the edge-arg correctness rule: terminator `root_uses` are unconditional roots; a
  successor edge arg is demanded only when its matching successor block param is demanded; if
  any result is demanded, operands are demanded.

### 3.2 Migrate `SparseForwardInterpreter` (two owner kinds)

Locked constraints (user, 2026-07-01):

- **Two owner kinds**, not just functions:
  ```
  Owner =
    Function(<CallContext<V>>::Key)
    Block { function: <CallContext<V>>::Key, block: Block }
  ```
  `Owner`/`SummaryKey` is **not** a `LatticeAnchor`. CFG block-entry convergence must become
  owner-summary convergence; `AbstractCFGFrame`'s `pending`/`queued`/`block_in` may be
  *temporarily* retained during a short mechanical transition, but M3 is not complete until it
  is gone.
- **Preserve the public API** verbatim: `SparseForwardInterpreter`, `CallContext`,
  `ContextInsensitive`, `WideningStrategy`, `analyze` / `analyze_by_name`, `return_summary`.
  Internally it becomes
  `StandardFixpointInterpreter<SparseForwardCore, SparseForwardProfile, …, ForwardSummaryDeps<Owner>>`
  where `SparseForwardCore: Interp` is the summary-free transfer + env.
- **Atomic call summarization** on `SparseForwardEffect::Call`: resolve callee → compute callee
  `Function` owner → register `callee → current (Function|Block) owner` dependency → merge/update
  callee entry summary → read callee return summary (or write bottom) → **explicitly register the
  self-dependency** for same-key recursion.
- **Summaries modelled separately:**
  - `FunctionSummary`: stage, body, entry args, return product/accumulator.
  - `BlockSummary`: block-entry product, visit/widen count, **plus the block's output facts**
    (the abstract values it defines that are read elsewhere) — `{entry, visits}` alone is
    **unsound** for direct cross-block uses (see §3.2.1).
- **Block owner transfer walks one block once:** bind the block-entry product, step statements
  until branch/jump/return/push/call, and emit `SummaryEffect` updates —
  successor `Block` owners for `Jump`/`Branch`, the current `Function` owner return for `Return`,
  the callee `Function` owner for `Call`. No local `pending`/`queued`/`block_in` remains in
  `AbstractCFGFrame`.
- **SCF loops** may stay traversal frames for this pass **only if** their convergence goes
  through the driver's shared `WideningStrategy`/`FixpointPhase` path. No second public solver.
  Any residual loop guard is documented as frame-local traversal.

### 3.2.1 Two design realities that shape the staging

- **Env lifetime + value deps.** Kirin treats *direct dominated cross-block SSA uses* as
  first-class: a value defined in a dominating block may be used in a dominated block **without**
  being passed as a block parameter. Evidence (Kirin 1.0 `main`): `CompactifyRegion` explicitly
  accounts for "SSAs from previous blocks (non-phi)", and `SortBlocks` / `Region.clone` rely on
  dominator/RPO ordering for statement-result uses across blocks; also visible in the liveness
  `edge_live_out` pass-through and `else`-uses-`%live`. Two consequences for M3b:
  1. All `Block` owners of one function **share that function's env**; the env lifetime spans the
     whole Function-owner convergence (allocate when the `Function` owner is created, not per block).
  2. `BlockSummary{entry, visits}` alone is **unsound**: a block owner must also be rescheduled when
     a directly-read external SSA fact rises. So block owners carry **value dependencies** — either
     def-use/value deps (a `defining-block → reading-block` owner edge, made observable by putting a
     block's output facts in its summary so a rise changes the summary and reschedules readers) or
     summary-visible env deps. A **regression** must cover a successor block that directly uses a
     predecessor-defined value that is *not* a block argument.
- **`AbstractFrameDriver` placement.** `summarize_call`/`contribute_return` need both the transfer
  (`SparseForwardCore`) and the owner summaries (the driver), so that capability is implemented on
  the `StandardFixpointInterpreter` wrapper, delegating the transfer parts to `inner`.

### 3.2.2 Staged execution (each stage green)

- **M3a — interprocedural layer → Function owners.** Introduce `Owner`, `FunctionSummary`,
  `BlockSummary`, `SparseForwardProfile`, and `SparseForwardCore`; move the interprocedural
  `while`-loop + `FnInfo`/`summaries`/`worklist`/`queued` + `summarize_call` summary bookkeeping +
  `return_summary` onto `StandardFixpointInterpreter` with `ForwardSummaryDeps<Owner>` (Function
  owners + `callee → caller` deps + self-deps). **`AbstractCFGFrame` retained** (constraint-1
  transition). Public API unchanged.
- **M3b — CFG → Block owners.** Replace `AbstractCFGFrame`'s block worklist with `Block` owners
  and `BlockSummary`; block walk emits `SummaryEffect` per §3.2; function env shared across its
  block owners; **value/def-use deps so a block owner reschedules when a directly-read external
  fact rises** (§3.2.1). Delete `pending`/`queued`/`block_in`. Regression: successor block directly
  uses a predecessor-defined non-argument value.
- **M3c — SCF + tests + docs.** Route any SCF loop convergence through the shared merge/phase;
  add the M3 tests (§6); update the interpreter design doc.

### 3.2.4 M3b mechanism (locked 2026-07-01) — soundness of direct cross-block uses

Owners: `Function(K)` + `Block { function: K, block: Block }`. One unified driver summary
type (`ForwardSummary`, since the driver's map is keyed by a single `Owner`):

- `Function`: entry args, return accumulator, **the shared env index**, entry visit count.
- `Block`: block-entry (param) product, **output facts** (the abstract values it writes), visit
  count.

**Shared env (constraint 2).** The function's env is allocated when its `Function` owner is
created and stored in the `Function` summary; every `Block` owner of that function binds/reads/
writes that one env; it is freed when the function's analysis completes. Values flow across blocks
through this env (transport); summaries drive *scheduling* (change detection).

**Value deps for direct cross-block uses (constraints 3, 4).** `SparseForwardCore` logs env
reads and writes during a block-owner walk (`read_log` / `write_log`, cleared at `entry_frame`).
On completion the driver, centrally:
- captures the block's **outputs** = `{ v: env[v] for v in write_log }` into the `Block` summary;
- registers **value-reader deps** `v -> this block owner` for each `v` in `read_log` (a
  value-granular index alongside `ForwardSummaryDeps`);
- when an output `v` rises, reschedules `value_readers[v]`.
So a directly-read dominating fact's rise is observable through the summary/dep machinery — CFG
successor-edge deps alone are *not* relied upon (constraint 3).

**Centralization (constraint 5).** All summary mutation + scheduling flow through **one** driver
method (merge candidate via the analysis policy `P` from `SparseForwardCore`, then reschedule via
`on_summary_changed`/`value_readers`). No scattered `summaries_mut()`/`schedule()` at call sites;
frames/`summarize_call` only *hand candidates* to that method. `ForwardSummary::merge` stays inert
(the joins are `P`-driven), i.e. the same pattern as M3a.

**Owner scheduling edges.** function entry → entry block; predecessor block → successor block
(entry update on `Jump`/`Branch`); block self-dep (rerun on own entry change); callee → caller
(from `summarize_call`, unchanged); plus the value-reader edges above.

**Block-owner walk.** `entry_frame` binds the block's params into the shared env and returns a
single-block CFG frame (walks one block, handling `Next`/`Call`/`Push` like `AbstractBlockFrame`;
at `Jump`/`Branch` hands successor-entry candidates to the central method; at `Return` contributes
to the `Function` return). Exactly one pass per analysis — no intra-owner block worklist.

**Two mechanism decisions (recommended):**
- *P centralization:* a custom central `apply_update` method with `P` kept only in
  `SparseForwardCore` (avoids duplicating `P` into the driver `strategy` and keeping `widen_after`
  in sync). `ConstPropContext: Clone` so the backbone-`Summary::merge`-with-`P`-clone path is also
  viable, but the custom method is simpler.
- *Def-use source:* dynamic read/write logging in `SparseForwardCore` (above), **not** static
  `extract_region` — the forward engine dispatches through `StageQuery`, which does not expose the
  per-stage dialect bounds (`HasArguments`/`HasResults`/`HasSuccessors`) `extract_region` needs.

**Staging (constraint 1).** Build `Block` owners + value deps, switch the engine to them, verify
green (incl. the loop-carried cross-block-rise stress test), **then** delete `AbstractCFGFrame`'s
`pending`/`queued`/`block_in`.

### 3.2.5 M3b implementation refinements (worked out 2026-07-01; switch not yet landed)

Key simplification found while implementing: **`Function` owners are storage-only — never
scheduled/analyzed.** Only `Block` owners run frames. This removes the need for a `Function`-owner
frame (the awkward "trivial no-op frame" problem). Consequences:
- `analyze(K)`: resolve → seed `Function(K)` summary (entry args, `entry_block`, meta) → seed the
  **entry `Block` owner** (block-entry = fn args) → schedule the entry block → drain → read
  `Function(K).ret` → free envs.
- `Function`-owner summary changes still fire owner-deps via `apply_update` (callee→caller), even
  though `Function` owners are never popped.
- When a `Function` entry rises (from `summarize_call`), re-seed + schedule its **entry `Block`
  owner** (not the `Function` owner).

Central method dispatch — `apply_update(ForwardUpdate)` with variants:
`FunctionEntry{args}` (join w/ widen; on rise → schedule callee entry block),
`FunctionReturn{values}` (join, no widen; on rise → reschedule callers via owner-deps),
`BlockEntry{args}` (join w/ widen; on rise → schedule this block owner),
`BlockOutputs{map}` (per-value replace + change-detect; on rise → reschedule `value_readers`).
The block frame cannot call `apply_update` (it's driver-concrete, not on the `AbstractFrameDriver`
trait), so it **records edges in the completion** and `complete_owner` calls `apply_update`.
Returns go via the existing `ret_acc` (`contribute_return`), flushed to `Function(K).ret` at the
block owner's `complete_owner`.

Frames-layer changes (implemented + reverted cleanly this session; re-apply first next time):
`AbstractCompletion::CFGBlock { edges: Vec<Edge<V>> }`; `AbstractBlockFrame` gains a
`BlockMode { StructuredBody, CFGBlock }` + `new_cfg_block`; in `CFGBlock` mode `Jump`/`Branch`
complete with `CFGBlock{edges}`, `Return` contributes + completes `CFGBlock{edges:[]}`, `Yield`
errors; resume with `Finished(None)` (nested return) completes `CFGBlock{edges:[]}`. Add `CFGBlock`
arms to the other frames' resume matches. During the switch `AbstractCFGFrame`/`AbstractFunctionFrame`
become unused (dead-code warnings) until deleted in the final step — build/tests stay green; clippy
`-D warnings` goes green only after deletion.

**Status:** design + frames-layer fully worked out and green-checkpointed; the ~500-line interp
rewrite (unified summary, `ForwardStore`, logging, `apply_update`, block-owner semantics,
`analyze`/`summarize_call` rewrite, `Store`-type change) is a single atomic edit, deferred to a
dedicated pass to avoid leaving the engine half-switched.

### 3.3 CFG block convergence as owner summaries

Superseded by §3.2 (two owner kinds) — retained heading for cross-references.

## 4. How the `dl/liveness` vocabulary survives

The kept vocabulary maps onto owner summaries without loss (unchanged from rev 1, still
authoritative):

- `LatticeAnchor` + `SparseStore`/dense stores become the *shape of a `Summary`/`Store`*.
  A `SparseStore<Fact>` keyed by `LatticeAnchor` is one `Summary` whose `merge` is per-anchor
  `join_with` (phase-insensitive for finite lattices). Dense block/point stores are other
  `Summary`/`Store` shapes.
- Sparse/dense × forward/backward markers become a taxonomy of summary shapes and dependency
  directions (sparse ↔ `SparseStore` summary; backward ↔ `BackwardSummaryDeps`; forward ↔
  `ForwardSummaryDeps`).
- `extract_region` / `StmtStructure` / `StmtFacts` / `DenseCursor` stay as the structural
  transfer used *inside* an owner analysis.
- `SummaryDependencyIndex` replaces the ad-hoc `dependents` map.

`LatticeAnchor`, `SparseStore`, `DenseBlockStore`, and `RegionStructure` remain first-class
fact vocabulary inside summaries.

## 5. Milestones

All on `dl/liveness`; each lands green before the next.

- **M1 — port the backbone.** `fixpoint` module: `FixpointPhase` / `Summary` /
  `OwnerSemantics` / `SummaryEffect` / `WorkItem`, `SummaryDependencyIndex`
  (+ owner/forward/backward), `FixpointProfile<I>`, `StandardFixpointInterpreter` (wrapper +
  `Interp`/`Env`/frame-driver delegation), `SimpleFixpointInterpreter`. Port the mature
  fixpoint unit tests. Additive — nothing else breaks yet.
- **M2 — liveness client.** Migrate `kirin-liveness` onto the shared driver with block
  owners (§3.1); derive the sparse demanded set from summaries. Delete the standalone
  `FixpointInterp`/`solve`; keep `DenseCursor`.
- **M3 — `SparseForwardInterpreter`.** Split into `SparseForwardCore` + wrapper; function/
  context owners; move CFG block convergence into the framework (§3.2, §3.3). Public API
  source-compatible.

## 6. Test plan

Port the mature fixpoint unit tests first:

- owner reanalysis until stable
- forward dependency scheduling
- backward dependency scheduling
- self-dependency A → A
- widening / narrowing phase behavior

Update liveness tests:

- terminator operands become demanded
- a demanded result marks operands demanded
- a dead result leaves operands dead
- branch edge args depend on successor block params
- `live_before` / `live_after` reconstruction still works

Preserve constprop / interprocedural tests:

- factorial const input remains precise; unknown input returns Top
- fibonacci const input remains precise; unknown input returns Top
- custom abstract frame tests still observe traversal

Add M3 tests before declaring completion:

- recursive factorial `Const(5)` precise; `Top → Top`
- recursive fibonacci `Const` precise; `Top → Top`
- branching CFG joins block entries through owner summaries
- same-key recursion registers `Owner → Owner`

Run: `cargo test -p kirin-interpreter fixpoint`, then `cargo test -p kirin-liveness`, then
recursive constprop / toy-lang tests, then the full workspace after the targeted suites pass.
Also `cargo clippy -p kirin-interpreter -p kirin-liveness --all-targets -- -D warnings` and
`cargo fmt`.

## 7. Assumptions and non-goals

- Implement on current `dl/liveness`; do **not** rebase wholesale onto `rust`.
- `Interp` is the only source of `Value`, `Error`, `Effect`, and `Kind`.
- `FixpointProfile<I>` must not re-declare those `Interp` associated types.
- Analyses use the shared owner-summary fixpoint framework; no analysis keeps a separate
  public solver.
- `LatticeAnchor`, `SparseStore`, `DenseBlockStore`, and `RegionStructure` remain first-class
  fact vocabulary inside summaries.

## 8. Risk notes

- `FixpointInterp::transfer_work_item -> Interp::Effect` was the coupling that blocked forward
  unification (forward `Interp::Effect` is pinned to `SparseForwardEffect`). The mature split
  of `FrameEffect` (traversal) vs `SummaryEffect` (convergence) removes it — the reason to
  adopt the backbone rather than extend `FixpointInterp`.
- Liveness is phase-insensitive (finite lattice): its `Summary::merge` ignores
  `Widen`/`Narrow`. Interval/constprop use the phases. `FixpointPhase` stays a driver concept;
  sparse-backward clients simply don't exercise widening.
- The wrapper picks up the blanket `SparseForwardInterp` impl only if it impls `Env` and
  delegates `Effect = I::Effect`; verify this composes for the M3 forward wrapper before
  reworking `AbstractCFGFrame`.
```

## 9. Status (2026-07-02): superseded in part by the two-liveness unification

M1–M3 landed as planned. The **M2 liveness client has since been superseded**:
liveness no longer uses the structural `extract_region`/`StmtFacts` path (both
deleted) or a fake `Interp`. It now ships as two real framework clients in
`kirin-liveness`, each dispatching dialect `Interpretable<I, Kind>` rules:

- **Strong liveness / demand** (`SparseBackward`, `SparseBackwardInterpreter`):
  summary owners are scope-qualified SSA values
  (`Scoped<(CompileStage, Region), SSAValue>`); the driver's default
  self-dependent index is the demand worklist; scf needs no frames.
- **Classic per-point liveness** (`DenseBackward`, `DenseBackwardInterpreter`):
  block owners with backward block walks and `BackwardSummaryDeps`
  (self-discovered from terminator `Edges`); scf owns dense frames; per-point
  sets reconstructed on demand.

Region topology (blocks incl. nested bodies, feeders) moved into `StageQuery`
actions — enumeration only, semantics in rules. `SparseForwardCore` was renamed
`SparseForwardTransfer` (and the backward inner engines follow the `*Transfer`
naming). §4's "vocabulary survives" mapping now reads: `LatticeAnchor`,
`Scoped`, `ProgramPoint`, `DenseAnchor`, and the `store.rs` stores are the
live fact vocabulary; `extract_region`/`StmtStructure`/`StmtFacts` are gone.
See `docs/design/interpreter/index.md` for the implemented design.
