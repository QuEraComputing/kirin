# U1: Core IR (kirin-ir) — Review Report
**Lines:** ~9,177 | **Files:** 64
**Perspectives:** Formalism, Code Quality, Ergonomics, Soundness, Dialect Author, Compiler Engineer
**Date:** 2026-03-22

---
## High Priority (P0-P1)

### 1. [P1] [confirmed] Statement::detach does not clear BlockInfo::terminator cache — detach.rs:13
**Perspective:** Soundness Adversary

`Statement::detach()` correctly updates the linked list (prev/next pointers, head/tail, length) but never checks whether the detached statement is the block's cached `terminator`. After detaching a terminator statement, `BlockInfo::terminator` still points to the detached (orphaned) statement ID.

**Attack construction:** `let term = block.terminator(&stage).unwrap(); term.detach(&mut stage);` -- now `block.terminator(&stage)` returns `Some(term)` pointing to an orphaned statement, while `block.last_statement(&stage)` may return the terminator *or* the linked list tail inconsistently. Any subsequent iteration or analysis trusting the terminator cache will observe stale data.

**Reachability:** Any pass or interpreter that detaches a terminator and then queries the block.

**Suggested action:** In `Statement::detach`, when `parent == Some(StatementParent::Block(block))`, check if `parent_info.terminator == Some(*self)` and clear it to `None`.

### 2. [P1] [confirmed] Detach for Statement: linked list length underflow in release builds — detach.rs:53-57
**Perspective:** Soundness Adversary

The length decrement `*parent_info.get_len_mut() -= 1` is guarded by a `debug_assert!(parent_info.get_len() > 0)`. In release builds, `debug_assert!` is stripped, and if the invariant is violated (e.g., by double-detach or stale parent pointer), the subtraction wraps around `usize::MAX` due to unsigned underflow, silently corrupting the linked list length. All `ExactSizeIterator` impls (`StatementIter`, `BlockIter`) trust this length.

**Attack construction:** Call `stmt.detach(&mut stage)` twice on the same statement. First call succeeds, clears prev/next/parent. Second call: `get_info_mut` returns `Some(info)` (the statement arena slot still exists, the deleted flag is not set by detach), but prev/next/parent are `None`, so it falls through to the `parent` branch which is `None` -- actually safe in this specific case because parent is `None`. However, if a stale `StatementParent::Block` is stored (e.g., after GC remap failure), the length will underflow.

**Reachability:** Lower than finding 1, but the pattern of `debug_assert` guarding arithmetic on `usize` is fragile.

**Suggested action:** Replace `debug_assert` + raw decrement with `self.len = self.len.checked_sub(1).expect("...")` or at minimum `self.len = self.len.saturating_sub(1)` with an `assert` in debug mode.

### 3. [P1] [confirmed] Arena GC invalidates all external IDs with no compiler-enforceable safety — arena/gc.rs:27
**Perspective:** Soundness Adversary

`Arena::gc()` compacts the arena and returns an `IdMap`, but all previously obtained IDs become stale. There is no generation counter, no lifetime tie, and no runtime detection. The doc comment acknowledges this: "After calling `gc()`, **all previously obtained IDs become stale**." Since IDs are `Copy` and stored throughout the IR (linked lists, parent pointers, SSA references, graph node weights), a single `gc()` call silently invalidates the entire IR structure.

**Attack construction:** `let stmt = stage.statement().definition(...).new(); stage.statements.gc(); let info = stmt.expect_info(&stage);` -- returns wrong data or panics with OOB access.

**Reachability:** `gc()` is `pub` on `Arena`. Any code with `&mut Arena` can call it.

**Suggested action:** Either (a) make `gc()` private/`pub(crate)` and only expose it through a safe whole-stage compaction API that remaps all references, or (b) add a generation counter to `Arena` and `Id` so stale access is detected at runtime.

### 4. [P1] [likely] DenseHint::insert_or_combine silently drops value when ID is out of range — arena/hint/dense.rs:46-59
**Perspective:** Soundness Adversary

`insert_or_combine` calls `self.data.get_mut(id.into().raw())`. If the index is beyond the current vec length, `get_mut` returns `None` and the value is silently dropped. In contrast, `insert()` on the same type dynamically resizes with `resize_with`. This inconsistency means callers using `insert_or_combine` may lose data without any indication.

**Suggested action:** Add the same resize logic from `insert()` to `insert_or_combine`, or return a `bool`/`Result` indicating whether the operation succeeded.

---
## Medium Priority (P2)

### 5. [P2] [confirmed] Detach impl_detach! macro duplicates Statement::detach logic — detach.rs:62-113
**Perspective:** Code Quality

The `impl_detach!` macro (used for `Block`) contains 50 lines of logic that is nearly identical to the explicit `Statement::detach` impl. The only semantic difference is that `Statement::detach` pattern-matches `StatementParent::Block(block)` while the macro version handles a generic parent. This is brittle duplication -- a fix to one (like the terminator cache issue in finding 1) must be manually replicated.

**Suggested action:** Extract the shared linked-list-detach logic into a generic helper function parameterized over the node type, or combine into a single trait default impl using the `LinkedListElem`/`ParentInfo`/`LinkedListInfo` query traits already defined in `query/info.rs`.

### 6. [P2] [confirmed] SparseHint Index impl requires Clone unnecessarily — arena/hint/sparse.rs:52-55
**Perspective:** Code Quality

`impl<T, I> Index<I> for SparseHint<I, T> where T: Clone` -- the `Clone` bound on `T` is not needed for `Index` (which returns `&T`). It is similarly unnecessary for `IndexMut`. `DenseHint`'s `Index`/`IndexMut` impls do not have this extra bound. This inconsistency restricts usage: you cannot index a `SparseHint<I, NonCloneType>`.

**Suggested action:** Remove the `T: Clone` bound from both `Index` and `IndexMut` impls on `SparseHint`.

### 7. [P2] [confirmed] #[allow(clippy::unit_cmp)] in ExactSemantics and LatticeSemantics — signature/semantics.rs:61,97
**Perspective:** Code Quality

`#[allow(clippy::unit_cmp)]` is used because `C` defaults to `()` and `call.constraints() == cand.constraints()` triggers the lint when `C = ()`. The comparison is semantically correct for non-unit `C`, but the allow suppresses a valid warning for the default case.

**Root cause:** The constraint comparison is always performed even when `C = ()`.

**Suggested action:** Specialize the `()` case: add a blanket impl or a `where C: PartialEq` bound that naturally handles both cases, or use a trait method `constraints_match()` that defaults to `true` for `()`.

### 8. [P2] [confirmed] #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)] on all builder .new() methods — builder/block.rs:93, region.rs:31, digraph.rs:84, ungraph.rs:77
**Perspective:** Code Quality / Ergonomics

Four builder types use `#[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]` because their `new()` method consumes `self` and returns the built node ID rather than `Self`. This is a deliberate builder-pattern choice that conflicts with Rust naming conventions.

**Suggested action:** Rename to `build()` or `finish()` to eliminate the lint suppressions. The `bon`-based builders already use `finish_fn = new` -- these hand-written builders could follow `finish_fn = build` instead for consistency.

### 9. [P2] [confirmed] No #[must_use] on any public type or method — entire crate
**Perspective:** Code Quality

The crate has zero `#[must_use]` annotations. Key types and methods that should have it:
- `Arena::alloc()`, `Arena::alloc_with_id()` (returns the ID)
- `Arena::delete()` (returns bool indicating success)
- `Arena::gc()` (returns IdMap that must be used)
- `Pipeline::function()`, `Pipeline::staged_function()`, `Pipeline::define_function()` (return Result)
- `BuilderStageInfo::finalize()` (returns Result)
- `StageInfo::with_builder()` (returns R)
- All ID types (`SSAValue`, `ResultValue`, `Block`, `Region`, `Statement`, etc.)

Ignoring the return value of `finalize()` or `gc()` would silently discard critical information.

**Suggested action:** Add `#[must_use]` to arena allocation methods, finalize, GC, and Result-returning builder methods. Consider `#[must_use]` on ID types.

### 10. [P2] [confirmed] GraphInfo does not derive PartialEq/Eq/Hash despite DiGraphExtra and UnGraphExtra doing so — node/graph.rs:39
**Perspective:** Formalism

`GraphInfo` derives only `Clone, Debug` but not `PartialEq, Eq, Hash`. This is because `petgraph::Graph` does not implement these traits. However, `DiGraphExtra` and `UnGraphExtra` both derive `PartialEq, Eq, Hash`, creating an asymmetry. Downstream code cannot compare two `DiGraphInfo` or `UnGraphInfo` values for equality, which blocks snapshot testing and assertion patterns for graph-bearing IR.

**Suggested action:** Document the limitation explicitly. If equality is needed, provide a semantic equality method that compares the graph structure (node/edge sets) rather than the petgraph internals.

### 11. [P2] [confirmed] O(n^2) specialization dispatch in all_matching — node/function/staged.rs:102-137
**Perspective:** Compiler Engineer

`StagedFunctionInfo::all_matching()` first collects all applicable specializations (O(n)), then for each applicable candidate, checks if any other candidate dominates it (O(n^2)). With many specializations (e.g., 50+ type-specialized versions of a polymorphic function), this becomes quadratic.

**Suggested action:** For the common case of `ExactSemantics`, dominance is trivially `Equal`, so the O(n^2) phase is unnecessary. Consider specializing the dispatch path or using a sorted/indexed structure.

### 12. [P2] [confirmed] Substantial BFS/graph-building duplication between UnGraphBuilder::new and BuilderStageInfo::attach_nodes_to_ungraph — builder/ungraph.rs vs builder/stage_info.rs:389-533
**Perspective:** Code Quality

`UnGraphBuilder::new()` (255 lines) and `BuilderStageInfo::attach_nodes_to_ungraph()` (145 lines) contain nearly identical BFS traversal, edge-to-node mapping, graph reordering, and parent assignment logic. The graph_common module was introduced to deduplicate port allocation but the BFS/graph construction was not extracted.

**Lines saved estimate:** ~120 lines by extracting into a shared helper.

**Suggested action:** Extract the BFS canonical ordering and graph construction into `graph_common.rs`.

---
## Low Priority (P3)

### 13. [P3] [confirmed] SSAInfo::default() creates a sentinel pointing to Statement(Id(0)), index 0 — node/ssa.rs:71-84
**Perspective:** Formalism

`SSAInfo::default()` constructs a value with `kind: SSAKind::Result(Statement(Id(0)), 0)`, which points to whatever statement occupies slot 0. This is only available when `L::Type: Default`, but if used, it creates an SSAInfo that appears to reference a real statement. The `Default` impl is presumably for arena tombstone purposes, but it could mislead analysis code.

**Suggested action:** Consider removing `Default` for `SSAInfo` if no longer needed (the finalized SSA arena uses `Option<SSAInfo>` for tombstones now). If kept, document it is a sentinel value.

### 14. [P3] [confirmed] finalize_unchecked silently swallows unresolved SSAs with a dummy SSAKind — builder/stage_info.rs:251-286
**Perspective:** Soundness Adversary

`finalize_unchecked()` replaces unresolved `BuilderSSAKind` with `SSAKind::Result(Statement(Id(0)), 0)`, a sentinel that points to statement slot 0. This is used by `StageInfo::with_builder()` for round-tripping. If a builder callback leaves SSAs unresolved (a bug), this silently creates valid-looking but incorrect IR rather than surfacing the error.

**Suggested action:** Log a warning or track the count of patched SSAs for diagnostic purposes.

### 15. [P3] [confirmed] BlockInfo fields are pub but builder is pub(crate) — node/block.rs:54-61
**Perspective:** Dialect Author

`BlockInfo`'s fields (`parent`, `name`, `node`, `arguments`, `statements`, `terminator`) are `pub`, allowing direct mutation that bypasses the builder's linked-list invariant maintenance. While the builder constructor is `pub(crate)`, the fields themselves are fully public.

**Suggested action:** Make fields `pub(crate)` and provide getter methods, consistent with `StatementInfo` which already uses `pub(crate)` fields.

### 16. [P3] [confirmed] Dialect trait has 16 supertrait bounds using HRTB — language.rs:117-142
**Perspective:** Compiler Engineer

The `Dialect` trait requires 16 supertraits, many using `for<'a>` HRTB. While the `#[derive(Dialect)]` macro generates all impls, the trait bounds affect compile time for any generic code bounded by `L: Dialect`. Each bound must be resolved for every monomorphization. With 50 dialects, this creates 50 * 16 = 800 trait impl resolutions per generic function call site.

**Suggested action:** This is an inherent cost of the design. Monitor compile times as dialect count grows. If problematic, consider splitting into a "core Dialect" (type + terminator + arguments + results) and "extended Dialect" (graphs, successors, regions) with a blanket impl.

### 17. [P3] [confirmed] Concept budget: 5 ID types for SSA values — node/ssa.rs
**Perspective:** Ergonomics / Dialect Author

Dialect authors encounter 5 SSA-related ID types: `SSAValue`, `ResultValue`, `BlockArgument`, `Port`, `DeletedSSAValue`. While `From` conversions exist between them, the distinction between `SSAValue` and `ResultValue` is a common source of confusion -- when to use which, and why `ResultValue` fields in dialect structs are different from `SSAValue` arguments.

| Concept | Types | When to use |
|---------|-------|-------------|
| SSA value (generic) | `SSAValue` | Statement arguments, general references |
| Statement result | `ResultValue` | Dialect struct result fields |
| Block argument | `BlockArgument` | Block parameter declarations |
| Graph port | `Port` | Graph boundary ports |
| Deleted placeholder | `DeletedSSAValue` | After deletion, for tracking |

**Suggested action:** Add a "which SSA type to use" decision tree to the prelude docs or AGENTS.md.

### 18. [P3] [confirmed] Pipeline::staged_function returns wrong error variant for missing stage — pipeline.rs:315
**Perspective:** Soundness Adversary

When the stage index is out of bounds (`stages.get_mut(Id::from(stage).raw())` returns `None`), the error is `PipelineError::UnknownFunction(func)`, which misidentifies the problem. The function exists; the stage does not.

**Suggested action:** Add a `PipelineError::UnknownStage(CompileStage)` variant and use it here.

### 19. [P3] [confirmed] TODO comment: use Cow for names — language.rs:2
**Perspective:** Code Quality

`// TODO: use Cow<'a, str> for name to avoid allocations in some cases` has been present since early development. Symbol interning via `InternTable` already avoids repeated allocations for names, making this TODO potentially obsolete.

**Suggested action:** Either implement the optimization or remove the TODO if interning makes it unnecessary.

---
## Strengths

1. **Clean arena-based design.** The `Arena<I, T>` + `Identifier` + `GetInfo<L>` pattern provides a consistent, type-safe way to access IR nodes. The `identifier!` macro eliminates boilerplate while maintaining type-level ID distinctions.

2. **Well-separated builder and finalized representations.** The `BuilderStageInfo`/`StageInfo` split with `BuilderSSAInfo`/`SSAInfo` ensures that build-time placeholders (`Unresolved`, `Test`) cannot leak into finalized IR. The `finalize()` method validates this invariant.

3. **Thoughtful error types.** `StagedFunctionError` and `SpecializeError` preserve enough context (conflicting IDs, signatures, backedges) for callers to either propagate or consume via `redefine_*` methods. This "error as recovery token" pattern is elegant.

4. **Graph unification.** The `GraphInfo<L, D, Extra>` generic type cleanly shares code between directed and undirected graphs while preserving type-level direction distinction via `petgraph::EdgeType`.

5. **Lattice formalism.** The `Lattice` / `HasBottom` / `HasTop` / `FiniteLattice` / `TypeLattice` hierarchy is clean and well-documented with algebraic laws. The `CompileTimeValue` blanket impl avoids redundant bounds.

6. **Diagnostic attributes.** `#[diagnostic::on_unimplemented]` on `Dialect`, `HasStageInfo`, and `StageMeta` provides actionable error messages when derive macros are missing -- a significant DX improvement.

7. **Comprehensive test coverage.** Pipeline, arena, intern table, stage dispatch, signature semantics, and SSA conversion all have unit tests covering both happy paths and error conditions.

8. **Three-level function hierarchy.** `Function -> StagedFunction -> SpecializedFunction` with invalidation support is a well-considered design that mirrors real compiler pipeline needs (staged compilation, specialization, redefinition).

---
## Filtered Findings

The following patterns were noted but not flagged per the Design Context rules:

- `BlockInfo::terminator` as cached pointer (Design Context item 4) -- the caching itself is intentional; only the detach bug (finding 1) is flagged.
- No unsafe code found anywhere -- consistent with the project policy.
- `#[wraps]` / `#[callable]` separation from `#[kirin(...)]` -- intentional (Design Context item 2).
- Darling re-export pattern -- not applicable to kirin-ir (no darling dependency).
- Single-lifetime parser traits, `ParseDispatch`, auto-placeholder -- not in kirin-ir scope.
- `L` on method vs trait for interpreter -- not in kirin-ir scope.
