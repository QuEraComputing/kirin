# Implementation Report

## Summary

All 6 plans from the 2026-03-01 codebase design review have been implemented and approved by specialized reviewers.

- [x] Plan 1: IR Core — approved (`review/01-ir-core`)
- [x] Plan 2: Interpreter Dispatch — approved (`review/02-interpreter`)
- [x] Plan 3: Parser Two-Pass & Ergonomics — approved (`review/03-parser`)
- [x] Plan 4: Derive Infrastructure — approved (`review/04-derive`)
- [x] Plan 5: Pretty Printer — approved (`review/05-printer`)
- [x] Plan 6: Dialect Cleanup — approved (`review/06-dialects`)

### Team

| Agent | Role | Plans |
|-------|------|-------|
| `lead` | Coordinator | All |
| `ir-impl` | Implementer | Plan 1 |
| `interp-impl` | Implementer | Plan 2 |
| `parser-impl` | Implementer | Plan 3 |
| `derive-impl` | Implementer | Plan 4 |
| `printer-impl` | Implementer | Plan 5 |
| `dialect-impl` | Implementer | Plan 6 |
| `review-core` | Reviewer | Plans 1 + 2 |
| `review-frontend` | Reviewer | Plans 3 + 4 |
| `review-surface` | Reviewer | Plans 5 + 6 |

---

## 2026-03-02 Follow-up

- Added a `StageInfo` block-identity remap path used by two-pass `Region::emit`, updating `node.ptr`, statement parents, and block-argument ownership when swapping real block payloads into stubs.
- `Pipeline::function()` now rejects duplicate abstract function names before allocation, so `function_by_name` is deterministic by construction.

---

## What Was Implemented

### Plan 1: IR Core Improvements (7/9 items)
- `Arena::gc()` safety documentation
- `InternTable` switched to `FxHashMap`
- Redundant `.map(|x| x)` removed from `Arena::iter()`
- `Clone` bound removed from `DenseHint` Index impls
- `Detach::detach` return type simplified to `()`
- `Pipeline::function_by_name()` added with FxHashMap index
- `FxHashSet<Use>` → `SmallVec<[Use; 2]>` for SSA use tracking

### Plan 2: Interpreter Dispatch (all 8 items)
- `call_handler` panic replaced with error return
- `run_nested_calls` / `run_nested_calls_cached` unified
- `push_call_frame_with_stage` / `push_call_frame_with_stage_cached` unified
- `resolve_dispatch_for_stage` / `lookup_dispatch_cached` deduplicated
- `HashSet<Statement>` → `FxHashSet`
- `resolve_stage` helper added to `StageAccess`
- `build_pattern` deduplicated in kirin-derive-interpreter
- `#[callable]` / `#[wraps]` interaction documented

### Plan 3: Parser Ergonomics (4/9 items)
- Two-pass `Region::emit` fixing forward block reference panic (P0)
- `FxHashMap` in `EmitContext` and `parse_text.rs`
- `input_requires_ir_type` unified in kirin-chumsky-format
- Dead `_ir_path` parameter removed from `BoundsBuilder`
- 3 regression tests for forward/backward/mixed block references

### Plan 4: Derive Infrastructure (3/8 items + cascading fixes)
- `constant => pure` property lattice validation with compile-error emission and 3 tests
- `kirin-derive-dialect` merged into `kirin-derive-core` (generators module)
- `callable` hint added to `error_unknown_attribute`
- Deprecated `InputBuilder`/`InputContext` re-exports removed
- Cascading fix: added `#[kirin(pure)]` to existing `#[kirin(constant)]` callers

### Plan 5: Pretty Printer
- `Config.line_numbers` dead code removed
- Dead alignment code (`result_width`, `max_result_width`) removed
- `RenderBuilder` added and `PrettyPrintExt` simplified around builder-style rendering
- `PrettyPrintName` / `PrettyPrintType` folded into `PrettyPrint` default methods
- `PrintExt` / `PipelinePrintExt` collapsed to builder-first surfaces (`render().to_string()`)
- Roundtrip property documented on `PrettyPrint` trait

### Plan 6: Dialect Cleanup (all phases including Phase 3)
- Div/Rem panic fixed with `CheckedDiv`/`CheckedRem` traits returning `InterpreterError`
- `Return` removed from `kirin-cf`; `kirin-function::Return` is canonical (~15 files updated)
- Module-level docs for kirin-cf, kirin-scf, kirin-function
- E0275 limitation documented on `Lambda` type and in AGENTS.md
- Standardized kirin-scf imports
- Phase 3: Adopted `resolve_stage` helper and `Pipeline::lookup_symbol` in dialect interpret impls (+32/-59 lines)
- Div/rem by-zero tests added (2 tests with IR fixtures)

---

## Decisions Needing Your Input

### High Impact

- **PhantomData removal from BlockInfo/RegionInfo (Plan 1)**: Skipped — medium risk with cascading signature changes across >20 files. If you want this done, it should be a dedicated follow-up with careful prototyping.

- **HasParser lifetime collapse `<'tokens, 'src>` → `<'src>` (Plan 3)**: Skipped — 62 files contain `HasParser<'tokens, 'src>`, well above the 30-file safety threshold. This is the highest-risk item across all 6 plans. Should remain on backlog as a dedicated effort, possibly with an RFC.

### Medium Impact

- **PhantomData auto-injection in derive (Plan 4 Phase 2)**: Skipped due to file reversion issues during implementation. The detection logic exists at `builder/helpers.rs:91` but the auto-default behavior was not added. Worth retrying in a clean session.

- **Field iteration deduplication (Plan 4 Phase 4)**: Same — skipped due to file reversion issues. `all_fields`/`field_pattern` still duplicated between `field/iter/statement.rs` and `property/statement.rs`.

- **`#[kirin(fn)]` → `#[kirin(builder)]` rename (Plan 4)**: Deferred — too many files, high risk for a P2-P3 item.

- **`tokens.to_vec()` elimination (Plan 3)**: Reverted — chumsky's `Stream::from_iter` ownership semantics require either owned data or a `'src`-lived slice reference. Fixing requires cascading signature changes.

### Low Impact (FYI only)

- **`TestSSAValue` gating behind `#[cfg(test)]` (Plan 1)**: Skipped.

- **ParseDialect helper trait, RecursiveAST, AST type rename (Plan 3 Phase 4)**: Deferred — pure ergonomic improvements, no correctness impact.

- **`derive(Interpretable)` for StructuredControlFlow (Plan 6)**: Deferred — only saves ~20 lines, not worth the proc-macro dependency cost.

- **Two-pass emit identity remap (Plan 3 follow-up)**: Addressed in follow-up work by adding a `StageInfo` remap helper that updates `node.ptr`, statement parent links, and block-argument `SSAKind::BlockArgument` ownership when swapping real block payloads into stubs.

---

## Reviewer Observations

- **review-frontend**: Plan 4's initial commit claimed lattice validation was implemented but only fixed callers. The validation logic itself was missing. Caught and fixed in a follow-up commit. Commit `8552e0c95` bundles snapshot updates with unrelated code changes — minor history hygiene concern.

- **review-surface**: Plan 5 had a build failure in `kirin-interpreter` tests (printer-impl accidentally touched interpreter test imports) and duplicate imports in kirin-derive-interpreter. Both fixed in follow-up commits.

- **review-frontend**: Plan 3's arena delete after stub-swap was analyzed for traversal safety (`Arena::iter()` and `Region::blocks()` behavior). Follow-up implementation additionally remaps statement parent and block-argument ownership to preserve block identity invariants beyond traversal.

- **2026-03-02 follow-up**: `Pipeline::function()` now rejects duplicate abstract function names up front, removing `function_by_name` last-write-wins ambiguity.

- **Pre-existing build error**: `kirin-cf::interpret_impl.rs` has an SSAValue import error present on the base commit `da7f3b9f5`. This is not introduced by any plan — it predates this work.

---

## Deferred Items

| Item | Plan | Priority | Reason |
|------|------|----------|--------|
| PhantomData removal from BlockInfo/RegionInfo | 1 | P2 | >20 file cascade, needs dedicated effort |
| HasParser lifetime collapse | 3 | P1 | 62 files, needs RFC + dedicated effort |
| ParseDialect helper trait | 3 | P2 | Pure ergonomics, no correctness impact |
| RecursiveAST + AST type rename | 3 | P2 | Pure ergonomics |
| PhantomData auto-injection in derive | 4 | P1 | File reversion issues, retry in clean session |
| Field iteration deduplication | 4 | P1 | File reversion issues |
| `#[kirin(fn)]` → `#[kirin(builder)]` | 4 | P2-P3 | Too many files |
| `tokens.to_vec()` elimination | 3 | P1 | Lifetime cascade through Stream::from_iter |
| TestSSAValue cfg gating | 1 | P3 | Low priority |
| derive(Interpretable) for StructuredControlFlow | 6 | P2 | Low value vs dependency cost |

---

## Branches

Each plan's work is on its own branch, ready for merge:

| Branch | Commits | Key commit |
|--------|---------|------------|
| `review/01-ir-core` | 2 | `f6b089eca` |
| `review/02-interpreter` | 3 | `be249f360` |
| `review/03-parser` | 1 | `eaa8f4153` |
| `review/04-derive` | 3 (+1 stray) | `b53b4b165` |
| `review/05-printer` | 3 (+2 cross-branch) | `ff62f8623` |
| `review/06-dialects` | 4 (+5 cross-branch) | `8d7035d04`, `e38cdd93e` |

**Note**: Some branches contain cross-branch commits from agents sharing the main repo worktree. These may need cherry-picking or rebasing before merge to avoid pulling in unrelated changes. Branches `review/05-printer` and `review/06-dialects` are the most affected.
