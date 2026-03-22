# Graph & Parsing Refactor — Implementation Plan

**Date:** 2026-03-21
**Pattern:** In-place, 3 waves, parallel agents in isolated worktrees
**Total changes:** 19 accepted findings across 7 crates

---

## Wave 1 — Foundation Fixes (5 parallel agents)

All file-disjoint, no dependencies between them.

### Agent 1: `arena-fix` — P0-1 + P1-2
**Crate:** kirin-ir
**Files:** `builder/stage_info.rs`, `stage/info.rs`
**Changes:**
- Replace `mem::zeroed()` with `Option`-wrapping via `try_map_live_option` (3 sites: lines 222, 262, 272)
- Fix `finalize_unchecked` to error on type-less SSAs instead of zeroing
- Update `StageInfo.ssas` type to `Arena<SSAValue, Option<SSAInfo<L>>>` if needed
- Update all `ssas.get()` call sites to handle `Option`

### Agent 2: `parser-soundness` — P0-2 + P1-1
**Crate:** kirin-chumsky
**Files:** `traits/emit_ir.rs`
**Changes:**
- Replace flat `FxHashMap` with `Vec<FxHashMap>` scope stack
- Add `push_scope()` / `pop_scope()` methods
- Update `lookup_ssa` / `lookup_block` to iterate from top of stack
- Add duplicate-name check in `register_ssa` / `register_block`
- Update `EmitContext::new()` to initialize with one scope
- Update Region/Block emission to push/pop scopes

### Agent 3: `docs-and-validation` — P1-3 + P2-6 + P2-12 + P2-17 + P2-19
**Crates:** kirin-derive-chumsky, kirin-chumsky, kirin-derive-prettyless
**Files:** `format.rs`, `validation.rs`, `field_kind.rs` (validation only), `has_dialect_emit_ir.rs`, `parse_emit.rs` (doc only), `derive-prettyless/lib.rs`
**Changes:**
- Add EBNF grammar + projection table as doc comment in `format.rs`
- Move `ir_path` validation to validation phase (from `field_kind.rs` expects to `validation.rs` check)
- Mark `HasDialectEmitIR` as `#[doc(hidden)]`
- Add ParseEmit decision table to `parse_emit.rs` doc
- Add doc comment to `#[derive(RenderDispatch)]`

### Agent 4: `dedup-and-trait` — P2-8 + P2-9 + P2-14 + P2-15 (RenderBuilder)
**Crates:** kirin-prettyless, kirin-ir, kirin-function
**Files:** `kirin-prettyless/document/ir_render.rs`, `kirin-prettyless/traits.rs`, `kirin-ir/src/` (new trait file), `kirin-function/interpret_impl.rs`
**Changes:**
- Extract `render_digraph_body_inner()` and `render_ungraph_body_inner()` in ir_render.rs
- Add `HasRegionBody` trait to kirin-ir (per user: should live in kirin-ir)
- Refactor FunctionBody/Lambda interpreter impls to use `HasRegionBody` blanket
- Add `#[must_use]` on `RenderBuilder`

### Agent 5: `roundtrip-test` — P2-5
**Crate:** tests/
**Files:** `tests/roundtrip/function.rs` (or new file)
**Changes:**
- Add roundtrip test for split signature projections (`{sig:inputs}` + `{sig:return}`)
- Verify parse → emit → print → compare for reconstructed Signature

---

## Wave 2 — Cross-cutting Changes (3 parallel agents)

**Depends on:** Wave 1 complete and merged.

### Agent 6: `builder-dx` — P2-2 + P2-3 + P2-4 + P2-9 + P2-15 (builders)
**Crate:** kirin-ir (primary), all crates with call sites
**Files:** `builder/{digraph,ungraph,block,region,staged}.rs`, call sites across workspace
**Changes:**
- Rename `new()` → `build()` on all 4 builders + update all call sites
- Add `port_ref(idx)` and `capture_ref(idx)` methods to `BuilderStageInfo`
- Promote `debug_assert!` → `assert!` for builder ordering (4 sites)
- Add `#[must_use]` on all builder types (DiGraphBuilder, UnGraphBuilder, BlockBuilder, RegionBuilder)

### Agent 7: `rename-error` — P2-18
**Crate:** kirin-chumsky (primary), all crates with references
**Files:** `traits/parse_emit.rs`, all `ChumskyError` references
**Changes:**
- Rename `ChumskyError` → `TextParseError`
- Update all import/usage sites across workspace
- Consider deprecated type alias for transition

### Agent 8: `parse-decompose` — P2-16
**Crate:** kirin-chumsky
**Files:** `function_text/parse_text.rs` → split into multiple files
**Changes:**
- Split into `parse_pipeline.rs` (pipeline trait + 2-pass), `parse_statement.rs` (statement trait), `lookup.rs` (resolution helpers)
- Update `function_text/mod.rs` with new module declarations
- Verify all tests pass

---

## Wave 3 — Design Work (2 parallel agents)

**Depends on:** Wave 2 complete and merged.

### Agent 9: `graph-unification` — P2-1
**Crates:** kirin-ir, kirin-prettyless, kirin-derive-toolkit, kirin-derive-chumsky
**Files:** Multiple files across 4 crates
**Changes:**
- Extract `GraphInfo<L, D: EdgeType, Extra>` parameterized struct
- Create `type DiGraphInfo<L> = GraphInfo<L, Directed, Yields>`
- Create `type UnGraphInfo<L> = GraphInfo<L, Undirected, EdgeStatements>`
- Refactor shared builder logic into `GraphBuilderBase<L, D>`
- Optionally merge `FieldCategory::DiGraph`/`::UnGraph` → `::Graph(Directedness)`
- Update printer code to use unified type
- Update derive infrastructure

### Agent 10: `emit-ir-uses-builders` — EmitIR builder reuse
**Crates:** kirin-derive-chumsky, kirin-chumsky
**Files:** `codegen/emit_ir/`, `ast/values.rs`
**Context:**
The derive-generated `EmitIR` code manually creates SSAs and statements via
`ctx.stage.ssa().ty(...).kind(...).new()`, duplicating logic that the
`derive(Dialect)`-generated builders already handle correctly (type setting via
`#[kirin(type = expr)]`, auto-placeholder, field wiring). This duplication is
how the ResultValue type-setting bug (fixed in Wave 1) occurred — the builder
knew the type but the parser reimplemented SSA creation from scratch.

**Changes:**
- Refactor derive-generated `EmitIR` to call the `derive(Dialect)`-generated
  builder methods (e.g., `Add::builder().lhs(a).rhs(b).new(stage)`) instead of
  manually constructing statements and SSAs
- `ResultValue::emit()` delegates to the builder, which already handles
  `#[kirin(type = expr)]` and auto-placeholder
- Ensure forward-reference resolution (graph parsing) still works through the
  builder path
- Update `EmitContext` interface if needed to bridge builder results back to
  SSA name registration

---

## Verification Checkpoints

After each wave:
1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo insta test --workspace` (if snapshots exist)

---

## Agent Count Summary

| Wave | Agents | Parallel? | Estimated Scope |
|------|--------|-----------|-----------------|
| Wave 1 | 5 implementers + 1 verifier | Yes (all parallel) | Quick wins + soundness |
| Wave 2 | 3 implementers + 1 verifier | Yes (all parallel) | Cross-cutting renames |
| Wave 3 | 2 implementers + 1 verifier | Parallel (file-disjoint) | Design work |
| **Total** | **10 implementers + 3 verifiers** | | |
