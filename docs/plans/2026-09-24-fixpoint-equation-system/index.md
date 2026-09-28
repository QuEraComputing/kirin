# Unified analysis refactor: frame boundaries and authoritative facts

Status: **consolidated architecture plan; implementation not started**
(updated 2026-09-28), branch `dl/fixpoint-owners`.

Builds on [`owner-identity-problem.md`](../../review/owner-identity-problem.md)
and revisits [`2026-06-30-fixpoint-convergence`](../2026-06-30-fixpoint-convergence/index.md).
Prerequisite for interprocedural demand analysis (separate plan).

This is the single parent plan for the fixpoint and analysis-boundary findings.
It incorporates the former analysis-boundaries review. The agreed first change,
[shared abstract call-boundary preparation](abstract-call-boundary.md), has its
own small implementation plan. Completing that subplan does not complete this
larger refactor. Type-bundle consolidation and the concrete dependency API remain
deferred; do not treat the overall architecture as an approved final Rust API.

Goal: remove the owner/summary machinery that overlaps the existing environment,
fact-storage, and frame abstractions. Context selects an environment; facts live
at lattice anchors in that environment; executable work runs frames; changes to
facts schedule dependent work. All three analyses use this protocol.

## 1. Architectural constraints: preserve the stacked PRs

These decisions are constraints, not alternatives to reconsider in this refactor:

| PR | Contract to preserve |
|---|---|
| [#723](https://github.com/QuEraComputing/kirin/pull/723) | Member frames return their own continuation state; private stack compositions own conversions. Reuse `Frame` / `drive_frames`; do not restore construction-trait cycles. |
| [#724](https://github.com/QuEraComputing/kirin/pull/724) | All engines enter callable roots through linking and discovery, followed by engine-specific boundary initialization. Honor the resolved stage. |
| [#729](https://github.com/QuEraComputing/kirin/pull/729) | `LinkTarget` selects stage/specialization; body discovery reads authoritative IR via `HasCallableBody` (currently `LinkTarget::body`). Do not restore interpreter-dispatched or argument-dependent discovery, or a redundant target/body authority. |
| [#732](https://github.com/QuEraComputing/kirin/pull/732) | Preserve `K → EnvStore::get_or_allocate(K) → EnvIndex → LatticeAnchor → fact`. `EnvStore` owns context allocation and per-environment `FactStore`s. `Env` stays anchor-generic; `SSABinding` stays SSA-specific; activation lifetime stays on sibling `CallServices`. |

**All authoritative analysis facts use `EnvStore`/`FactStore`.** No parallel
function-summary map, block-summary map, or solver-owned fact table. A function
result is a boundary fact in the existing storage abstraction. "Function summary"
may describe the function's boundary facts, but does not name another container.

The framework solves mutually dependent computations: run work, update facts,
and repeat affected work until stable. A fact location identifies information;
a work item identifies a computation. The dependency index connects them without
introducing another state-identity or context-sharing mechanism.

## 2. Vocabulary: current names → proposed responsibilities

| Concept | Current names / problem | Proposed |
|---|---|---|
| Dialect rule-set tag | `SemanticKey`, `Interp::Semantics`, `Sem` | Keep unchanged. |
| Executable computation | `Owner`, `SummaryKey`, `WorkItem::Analyze` conflate storage and work | Runnable work with its environment and IR location; queue it directly without a second one-variant `WorkItem` wrapper. |
| How to run work | `OwnerSemantics`, `SparseForwardSemantics`, `SparseBackwardSemantics`, `DenseBackwardSemantics` | Frame-owned preparation and completion via `step_into` / `resume_into`; remove the solver lifecycle hooks. |
| Type bundle | `FixpointProfile`, separate adapter and `Store`/`Deps` parameters | Deferred. Keep the existing type bundle where needed during migration; no new lifecycle trait or independent fact store. |
| Authoritative information | Generic `Summary` plus summary maps, sometimes duplicating environment facts | Existing `EnvStore` / `FactStore`; add boundary anchors where needed. Delete `Summary` and `SummaryEffect`. |
| Dependency source | `Owner`, `ValueFactKey`, proposed `ForwardState` | Existing environment + lattice-anchor location; exact dependency API deferred. No independent identity or storage. |
| Fact update | Summary effects versus manual forward updates | Engine helper merges against authoritative facts, writes, and notifies dependents on change. |
| Context sharing | `CallContext::Key` and body scopes | Keep policy-selected `K` and `EnvStore`'s existing mapping. Work carries the selected environment; the solver does not allocate contexts. |

`*Semantics` currently adapts work to frames: forward starts a block/graph and
processes its edges/returns; demand starts the rules that propagate one value's
demand; dense backward starts a block walk and processes its boundary facts.
These responsibilities move into frames, not a renamed work-execution adapter.
A work-boundary parent can prepare and push a walker, then process its completion;
a leaf frame can do its work directly. No wrapper is required solely for symmetry.
Initial frames are constructed outside the stack using constructors and
composition-owned conversions, preserving #723. `FrameEffect` is unchanged.

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

### 3.1 Abstract root and nested calls repeat call preparation — fix first

`SparseForwardInterpreter::analyze` resolves/discovers the callable, selects a
context key, contributes function inputs, drains the worklist, and reads returns.
`AbstractCallFrame::step_into` delegates to `summarize_call`, which separately
resolves/discovers, selects context, contributes inputs, records the caller, and
reads/binds current returns. The frame is largely a wrapper around an engine-owned
call protocol. Root and nested entry can drift as either path changes.

### 3.2 Work execution has a second lifecycle outside frames — follow-up

The solver calls `OwnerSemantics::entry_frame`, runs frames, and then calls
`complete_owner`. Those preparation/completion responsibilities fit frame methods.
Move them into a work-boundary frame when a child walker needs surrounding
processing; a leaf demand frame may incorporate them directly. Do not add a
wrapper frame solely for naming symmetry. Remove the solver lifecycle hooks when
their behavior has moved. Type-bundle consolidation remains deferred.

### 3.3 Body-kind selection is repeated, but selects different things — follow-up

The exhaustive `Body` match is in `CallFrame::step_into`, dispatching to the
`CallBodyTraversal` methods. `seed_entry_block` has a similar match, but produces
scheduled block/graph work rather than a concrete child walker. A concrete CFG
entry selects a CFG walker; abstract entry schedules its entry block. Neither
should inherit the other's traversal or capability requirements.

Share pure structural dispatch only if it removes meaningful duplication without
adding a larger visitor/factory hierarchy. A small repeated match over a closed
IR enum is not by itself a duplicated lifecycle. Keep `CallBodyTraversal` concrete
unless a separate review demonstrates a compatible common interface. Do not
make abstract frames require concrete activation lifetime services.

### 3.4 Fact updates and summary storage overlap existing environments — follow-up

`apply_update` mixes analysis merging, summary storage, and notifications.
All authoritative facts must ultimately live in `EnvStore`/`FactStore`, including
boundary results. Engine access/update operations merge under analysis policy,
write through the established environment path, and notify readers on change.
Low-level `EnvStore::write` remains assignment. Not every transient interpreter
write is a lattice join, and no second public read/write service is needed.

Boundary updates alone are insufficient if solver-visible SSA facts change too:
every fact change consumed by other work needs correct notification. Exact anchor
representation and dependency changes belong to the later phases of this plan,
not the first call-boundary subplan.


## 4. Target design

### 4.1 Authoritative storage and boundary facts

Retain the existing layering:

```text
analysis policy selects K
    → EnvStore maps K to EnvIndex
    → FactStore holds facts at LatticeAnchor locations
    → engine access/update helpers interpret reads, merges, and writes
    → dependency notifications schedule executable work
```

`EnvStore::write` remains assignment, not implicit joining. Missing facts remain
distinct from invalid environments; engines decide when absence means bottom.
Analysis helpers apply `Lattice` / `WideningStrategy` before committing a fact.
`FactStore::join_with` may be used where it fits. Storage itself knows nothing
about scheduling, dependencies, or widening policy.

Before migration, specify and test the boundary-anchor representation:

- SSA facts have one authoritative location. Delete persistent block-output
  copies used as a second source of values; compare with authoritative facts at
  update time. Read/write logs may hold locations and transient execution data.
- Block inputs and function inputs use the appropriate parameter/boundary
  locations. Multiple incoming edges or calls merge at the same locations.
- Function return positions have explicit boundary locations: contributions from
  multiple return statements merge there, and callers read those same facts.
  Preserve multi-result arity, empty products, and the distinction between no
  completed return and a completed zero-result return where required. Represent
  semantic reachability/completion information as facts, not an untracked flag.
- Dense block entry/exit facts use the same authoritative locations exposed as
  point facts, rather than a second live-in/live-out summary table.
- Sparse demand facts live at SSA anchors in the selected environment; its old
  summary table and global body-scope field disappear.

`LatticeAnchor` is the existing extension point. Do not replace it with a
`StateIdentity` hierarchy. The exact anchor/payload representation must respect
homogeneous `FactStore<A, V>` typing and existing dialect contracts:
`SparseForwardInterp` and `SSABinding` currently pin `Env::Anchor = SSAValue`.
Do not simply substitute a mixed anchor enum and break those bounds. The storage-integration
step must demonstrate how SSA access and additional boundary
anchors address the authoritative environment without a second context directory
or duplicate fact values. A typed storage projection/extension, if needed, must
preserve the established SSA-facing API and safe Rust. Resolve this integration
before freezing the solver's Rust signature.

Non-fact bookkeeping (widening counters, work discovery, IR handles, dependency
sets) may live on the engine/solver. It must not become a renamed summary store.
Transient frame-local working values remain valid; externally consumed converged
facts must come from the authoritative storage.

### 4.2 One frame lifecycle, two boundary responsibilities

Concrete already has the target construction pattern: `call()` constructs
`CallRequest::root`, converts it into the configured stack item, and runs
`drive_frames`. `CallFrame` owns preparation and completion for both root and
nested calls. Leave it unchanged.

**Abstract call boundary:** root and nested requests use `AbstractCallFrame` for
common resolution/discovery, context/environment selection, and input contribution.
Nested calls bind the current return approximation and finish with `Done`. Root
preparation finishes with a selected-instance handle; the runner drains the
worklist and reads its result. The detailed first change is in
[abstract-call-boundary.md](abstract-call-boundary.md). A nested abstract call
must not recursively drain the worklist or descend through concrete call frames.

**Work boundary:** scheduled work is converted into an initial frame. Its
`step_into` prepares the computation and may push a walker; `resume_into` handles
the child's completion through existing environment operations. `resume_done_into`
handles payload-free child completion. A leaf may complete directly. Root frames
must produce `Complete`; `Done` requires a parent. The solver has no separate
`start_work`/`complete_work` lifecycle hooks after migration.

```text
public entry → root request → initial frame → drive_frames

analysis additionally:
    queued work → initial frame → drive_frames → fact changes → affected work
```

Frame construction is a narrow constructor/conversion concern, not another
execution protocol. Keep the current type bundle as needed; deciding whether to
consolidate it into `EquationSystem` is deferred.

The solver owns queue/discovery/dependency bookkeeping and active work; the engine
owns the existing environment storage. The driver may retain its reusable stack
buffer. No arbitrary analysis `Store` or `State` type or solver summary database
is introduced.

A dependency refers to an existing `(EnvIndex, anchor)` location within the
analysis's authoritative store; indices are store-local. Detailed dependency API
selection is deferred. Any tagged reference must identify real fact locations,
without a new context mapping or fact payload. Facts remain partitioned by
environment; dependency bookkeeping is not another fact store.

### 4.3 Lifecycle and notification invariants

- **Discovery is explicit throughout solving.** Initial roots, newly reached
  successors, new callees, and newly demanded values must have their work
  discovered before relying on subscriptions. Discovery schedules a first run
  even if no fact rose (important for zero-argument entries and effect roots).
  Unreachable forward work must not be discovered merely because it exists in IR.
- **Reads subscribe even when the fact is absent/bottom.** Thus a later rise can
  wake the reader. Demand work subscribes to its own demand before evaluation;
  discovery handles the first run before that subscription exists.
- **All solver-visible updates use engine helpers** that merge, compare, commit,
  and notify together. Avoid exposing unrestricted mutation through the analysis
  facade. Keep raw container APIs usable for concrete execution and initialization.
- **Queue deduplication permits self-rescheduling.** Remove a work item from the
  queued set before executing it. A change during execution/completion can queue
  it again. Being discovered or currently running does not suppress such work.
- **Dependencies are retained conservatively for an analysis run.** Extra old
  edges may cause extra work; silently dropping live edges can miss updates.
  Clearing/freeing environments must also retire their dependent work and edges,
  or reset the analysis as a unit. Preserve each engine's existing public reuse
  behavior unless separately documented and tested.
- **Real reads must reach the solver.** `env_read` and demand `fact` take `&self`;
  forward/dense dialect dispatch may target the inner transfer. Preserve those
  interfaces using safe read logs/interior bookkeeping or an equivalent explicit
  bridge. Drain logs and install dependencies before completion notifications;
  any earlier changes need immediate registration or replay so none are missed.
- **Active work and logs have a defined lifetime.** Seeding outside the work loop
  supplies its environment explicitly. Keep active work through completion;
  clear active state and transient logs on success and error. Do not substitute
  a global body scope for per-work environment selection.

### 4.4 Per-engine mapping

| | Sparse forward | Sparse backward demand | Dense backward liveness |
|---|---|---|---|
| Work | Block or graph + environment | SSA value + environment | Block + environment |
| Facts read | Entry parameters, callee return boundaries, SSA operands | Demand facts consulted by the rule | Successor live-in boundaries |
| Facts updated | Successor/callee inputs, return boundaries, SSA results | Operand demand | Block boundaries and program-point facts |
| First discovery | Root, reached successor, callee | Root demands and newly demanded values | Blocks in the analyzed body |
| Storage | Existing environment/fact pipeline with required boundary anchors | Same pipeline, SSA anchors | Same pipeline, program-point anchors |

Function records are never executable work. Context selection occurs before
work discovery through the existing `K → EnvIndex` mechanism.

### 4.5 Context and callable identity

Forward already implements resolved-callable plus context keying:
`ContextInsensitive::Key = LinkTarget`, and constprop uses `(LinkTarget, CallCtx)`.
Preserve this behavior and its context-budget fallback.

`BodyScope = (stage, body)` can remain a public identifier for current standalone
backward analyses. Internally, select an environment and carry it with the work;
do not read an independent global scope. Body identity is not automatically
callable identity, and `BodyScope` need not be renamed or deleted just for symmetry.
Future interprocedural demand may choose a key including the resolved callable
and demanded-result pattern, a call site, or a shared context with joined demand.
That policy is outside this plan; preserve the seam without selecting it here.

## 5. Implementation steps

Each step leaves the workspace tests green. Transitional adapters must be marked
for deletion; the final architecture must not retain dual fact authorities.

1. **Unify abstract root and nested call preparation.** Implement the agreed
   [AbstractCallFrame subplan](abstract-call-boundary.md). Keep concrete unchanged.
   Existing summary-backed operations and solver hooks may remain temporarily;
   this phase adds no storage and does not require the remaining migrations.
2. **Prove storage and access integration.** Define boundary anchors/payloads and
   their SSA/dense access paths under §4.1. Add a committed test using the real
   `Env`, dispatch, and frame protocol that reads a function boundary fact from
   the same authoritative environment it updates. Cover context isolation,
   multi-result/zero-result returns, and the read-notification bridge. Do not
   treat a standalone queue model as proof of this integration.
3. **Add fact-location dependencies and executable work.** Resolve the deferred
   dependency API against the storage integration first. Then implement discovery,
   queue deduplication, read registration, and notification with the lifecycle in
   §4.3. Keep the old clients working through temporary adapters if necessary.
   Test first discovery during solving and self-rescheduling, not only duplicate
   queue insertion. Forward widening visit counts may change: check precision
   and termination with constprop/interval tests.
4. **Migrate sparse backward.** Move demand into environment facts; pass the
   environment during root seeding and derive it from work during propagation.
   Remove `BackwardAnalysisState::scope` and demand summaries. Newly demanded
   values are discovered explicitly. Test two bodies/environments in one solve.
5. **Migrate dense backward.** Store live-in/live-out and point facts through the
   authoritative environment pipeline. Subscribe to successor entry locations,
   remove block-summary duplication, and preserve current root/reset behavior.
6. **Migrate forward.** Replace `Owner` with executable block/graph work. Move
   function boundary results and remaining boundary inputs into authoritative
   facts. Route all relevant updates through merge-and-notify helpers. Remove
   `ForwardSummary`, `FunctionSummary`/`BlockSummary` fact containers, output-value
   snapshots, downcasts, and storage-only runtime errors. Keep metadata/counters
   separately without duplicate facts or context directories.
7. **Remove obsolete framework layers.** Move remaining `*Semantics` preparation
   and completion into work-boundary frames (or existing leaf frames) and remove
   solver lifecycle hooks. Delete `Summary`, `SummaryEffect`,
   summary maps/mutators, old dependency indexes, and redundant `Store`/`Deps`
   parameters where made obsolete by storage migration. Defer discretionary profile
   consolidation; retain `SemanticKey`, frame composition, and capability bounds.
   Delete unused phase/narrowing APIs after confirming the client audit; retain
   working engine widening policy. A future narrowing API needs a real client
   and meaningful round/work limits.
8. **Document the final model.** Update the interpreter design, `AGENTS.md` and
   applicable `CLAUDE.md` guidance, and module docs. Amend the owner review to
   explain that dependency sources are existing environment/anchor locations.
   Public compatibility and exports must be reviewed before removing APIs; local
   non-use alone does not prove that downstream users do not exist.

## 6. Settled decisions and remaining implementation questions

**Settled:**

- `EnvStore`/`FactStore` is the sole authoritative storage for analysis facts.
- No independent analysis `Store`, `State` identity, or summary database.
- Abstract root and nested calls share one boundary frame; the separate subplan
  is the first fix, not the whole refactor.
- Work preparation/completion belongs in frames. No change to `FrameEffect` or
  replacement `start_work`/`complete_work` hook protocol.
- Context sharing uses the existing policy and `EnvStore` mapping.
- Work contains only runnable computations. Discovery and change notification
  are distinct operations; both are required.
- The stacked PR contracts in §1 remain intact.

**Resolve in the storage-integration step:** exact boundary-anchor/payload representation under the
existing SSA-pinned access contracts; the safe read-registration bridge across
inner-transfer dispatch; representation of completed empty returns/reachability.
These are implementation choices within the settled architecture, not permission
to restore side summary maps.

Type-bundle consolidation and the exact dependency API are deferred; resolve them
only when their later migration requires it. Pure body-kind dispatch sharing is
optional and must preserve concrete versus scheduled abstract traversal.
Renaming `StandardFixpointInterpreter` is optional and deferred. Interprocedural
demand context policy remains a separate design task. No new global glossary
or context hierarchy is required just to rename these implementation types.

## 7. Non-goals

- Changing semantic keys, dialect rule meanings, or public SSA rule vocabulary.
- Changing frame composition, frame protocol, or dialect-owned traversal.
- Reintroducing callable discovery into interpreter dispatch.
- Replacing `EnvStore`/`FactStore`, weakening `LatticeAnchor`, moving lifecycle onto
  `Env`, or making low-level storage choose context or convergence policy.
- Changing what existing `CallContext` / `WideningStrategy` policies compute.
- Implementing interprocedural demand or a new narrowing algorithm.

## 8. Validation

After each implementation step: `cargo nextest run --workspace` and
`cargo test --doc --workspace`; run `cargo build --workspace` and `cargo fmt --all`
as required. Shared test helpers belong in `kirin-test-utils`.

Existing regression guards:

- Forward: `constprop_loop_carried_cross_block_rise_is_top`,
  `constprop_direct_dominated_cross_block_use` (toy-lang),
  `abstract_digraph_owner_joins_two_call_sites`,
  `abstract_digraph_self_recursion_converges` (`tests/body_kinds.rs`).
- Contexts: `constprop_context_budget_overflow_falls_back_to_top` and factorial
  constprop (`cargo run -p toy-lang -- run example/toy-lang/programs/factorial.kirin
  --stage source --function factorial --constprop 5`) still gives `Const(120)`.
- Backward: `scf_for_loop_carried_demand_converges`,
  `dense_loop_carried_fixpoint`, and the `kirin-liveness` suite.
- Existing frame-capability, callable-discovery, cross-stage/linker, and
  environment lifecycle tests from the stacked PRs.

The first subplan's [validation cases](abstract-call-boundary.md#2-validation)
cover root/nested parity, recursion, body kinds, and preparation/result timing.

New focused guards for the full refactor:

- Multiple returns join at authoritative boundary facts; callers read exactly
  those facts. No second summary/output map supplies analysis answers.
- Zero-input and zero-result callees are discovered and completion is represented
  correctly. A bottom-valued boundary does not suppress required first execution.
- Equal context keys share an environment; different keys isolate the same
  anchors. Backward bodies coexist in one solve without scope leakage.
- Duplicate enqueues run once; a running computation can schedule itself again.
- A changed fact wakes every registered reader, including readers of absent facts;
  an unchanged update schedules no extra work.
- Newly discovered demand chains, CFG successors, and callees run without prior
  subscriptions. Unreachable forward blocks are not accidentally activated.
- Recursive return updates and cross-block SSA rises reach callers/readers through
  the real dispatch/read-log path. Dense predecessor reanalysis follows successor
  entry-fact changes.
- Error cleanup leaves no active work or stale transient logs; analysis reset or
  environment retirement cannot leave executable work targeting dead environments.

## 9. References

- [First subplan: AbstractCallFrame](abstract-call-boundary.md)

- [`owner-identity-problem.md`](../../review/owner-identity-problem.md)
- [`2026-06-30-fixpoint-convergence`](../2026-06-30-fixpoint-convergence/index.md)
- Stacked PRs [#723](https://github.com/QuEraComputing/kirin/pull/723),
  [#724](https://github.com/QuEraComputing/kirin/pull/724),
  [#729](https://github.com/QuEraComputing/kirin/pull/729),
  [#732](https://github.com/QuEraComputing/kirin/pull/732).
